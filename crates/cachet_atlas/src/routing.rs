// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use cachet_storage::{AtlasPageId, RectAtlasSet};

use crate::{ArtifactRequest, AtlasClass};

/// One atlas-side routing rule from an [`AtlasClass`] to a compatible page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasPageAssignment {
    class: AtlasClass,
    page: AtlasPageId,
}

impl AtlasPageAssignment {
    /// Creates one atlas page assignment.
    #[must_use]
    pub const fn new(class: AtlasClass, page: AtlasPageId) -> Self {
        Self { class, page }
    }

    /// Returns the class described by this assignment.
    #[must_use]
    pub const fn class(self) -> AtlasClass {
        self.class
    }

    /// Returns the compatible page for this assignment.
    #[must_use]
    pub const fn page(self) -> AtlasPageId {
        self.page
    }
}

/// Atlas-side routing rules for choosing compatible storage pages.
///
/// This type keeps workload policy in `cachet_atlas`: classes describe
/// compatibility, while storage pages remain storage-owned physical targets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AtlasPageRouter {
    assignments: Vec<AtlasPageAssignment>,
}

impl AtlasPageRouter {
    /// Creates an empty atlas page router.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            assignments: Vec::new(),
        }
    }

    /// Registers one compatible page for an atlas class.
    ///
    /// Returns whether this call inserted a new routing rule.
    pub fn register_page(&mut self, class: AtlasClass, page: AtlasPageId) -> bool {
        if self
            .assignments
            .iter()
            .any(|assignment| assignment.class == class && assignment.page == page)
        {
            return false;
        }
        self.assignments.push(AtlasPageAssignment::new(class, page));
        true
    }

    /// Returns the compatible pages registered for one atlas class.
    pub fn pages_for(&self, class: AtlasClass) -> impl Iterator<Item = AtlasPageId> + Clone + '_ {
        self.assignments
            .iter()
            .filter(move |assignment| assignment.class == class)
            .map(|assignment| assignment.page)
    }

    /// Returns compatible pages for an artifact request in preferred order.
    ///
    /// The current policy prefers the least-allocated compatible page that can
    /// represent the requested size. Ties fall back to fewer tracked free
    /// regions and then page id for determinism.
    pub fn candidate_pages<K>(
        &self,
        request: &ArtifactRequest<K>,
        pages: &RectAtlasSet,
        out: &mut Vec<AtlasPageId>,
    ) {
        out.clear();
        out.extend(self.pages_for(request.class()).filter(|page| {
            pages.page_stats(*page).is_some_and(|stats| {
                request.size().width() <= stats.width() && request.size().height() <= stats.height()
            })
        }));
        out.sort_unstable_by_key(|page| {
            let stats = pages
                .page_stats(*page)
                .expect("candidate pages come from registered storage pages");
            (stats.allocated(), stats.free_regions(), *page)
        });
    }

    /// Chooses the best currently compatible page for an artifact request.
    #[must_use]
    pub fn best_page_for<K>(
        &self,
        request: &ArtifactRequest<K>,
        pages: &RectAtlasSet,
    ) -> Option<AtlasPageId> {
        self.pages_for(request.class())
            .filter(|page| {
                pages.page_stats(*page).is_some_and(|stats| {
                    request.size().width() <= stats.width()
                        && request.size().height() <= stats.height()
                })
            })
            .min_by_key(|page| {
                let stats = pages
                    .page_stats(*page)
                    .expect("candidate pages come from registered storage pages");
                (stats.allocated(), stats.free_regions(), *page)
            })
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use cachet_residency::Priority;

    use super::*;
    use crate::ArtifactSize;

    #[test]
    fn router_prefers_the_least_allocated_compatible_page() {
        let mut pages = RectAtlasSet::new();
        let page0 = pages.add_page(16, 16).expect("page id fits");
        let page1 = pages.add_page(16, 16).expect("page id fits");

        let _ = pages.allocate_in(page0, 4, 4).expect("page 0 has room");

        let mut router = AtlasPageRouter::new();
        assert!(router.register_page(AtlasClass::new(7), page0));
        assert!(router.register_page(AtlasClass::new(7), page1));

        let request = ArtifactRequest::new(
            "glyph:A",
            AtlasClass::new(7),
            ArtifactSize::new(4, 4),
            Priority::new(0, 10),
            0,
        );

        assert_eq!(router.best_page_for(&request, &pages), Some(page1));
    }

    #[test]
    fn router_does_not_treat_class_as_page_identity() {
        let mut pages = RectAtlasSet::new();
        let page3 = pages.add_page(16, 16).expect("page id fits");
        let page4 = pages.add_page(16, 16).expect("page id fits");

        let mut router = AtlasPageRouter::new();
        assert!(router.register_page(AtlasClass::new(1), page3));
        assert!(router.register_page(AtlasClass::new(1), page4));

        let compatible: Vec<_> = router.pages_for(AtlasClass::new(1)).collect();
        assert_eq!(compatible, vec![page3, page4]);
        assert_ne!(compatible[0].get(), 1);
    }

    #[test]
    fn candidate_pages_reuses_caller_output_buffer() {
        let mut pages = RectAtlasSet::new();
        let page0 = pages.add_page(16, 16).expect("page id fits");
        let page1 = pages.add_page(16, 16).expect("page id fits");
        let page2 = pages.add_page(2, 2).expect("page id fits");

        let _ = pages.allocate_in(page0, 4, 4).expect("page 0 has room");

        let mut router = AtlasPageRouter::new();
        assert!(router.register_page(AtlasClass::new(7), page0));
        assert!(router.register_page(AtlasClass::new(7), page1));
        assert!(router.register_page(AtlasClass::new(7), page2));

        let request = ArtifactRequest::new(
            "glyph:A",
            AtlasClass::new(7),
            ArtifactSize::new(4, 4),
            Priority::new(0, 10),
            0,
        );

        let mut out = Vec::with_capacity(2);
        let original_capacity = out.capacity();

        router.candidate_pages(&request, &pages, &mut out);
        assert_eq!(out, vec![page1, page0]);
        assert_eq!(out.capacity(), original_capacity);

        router.candidate_pages(&request, &pages, &mut out);
        assert_eq!(out, vec![page1, page0]);
        assert_eq!(out.capacity(), original_capacity);
    }
}

// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::{cmp::Reverse, fmt, hash::Hash};

use cachet_residency::{
    AdmissionError, Budget, Epoch, ProcessedRequest, ResidencyBindings, ResidencyHandle,
    ResidencyTracker, ResidentEntry,
};
use cachet_storage::{
    AllocationError, AtlasPageId, PagedAtlasSlot, RectAtlasSet, RectAtlasSetStats, RectAtlasStats,
};

use crate::{ArtifactRequest, AtlasClass, AtlasPageRouter, ResolvedArtifact};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AtlasBinding {
    class: AtlasClass,
    slot: PagedAtlasSlot,
}

/// Atlas-side allocation failure reported per processed request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasAllocationError {
    /// No compatible page was registered for the request's class and size.
    NoCompatiblePage,
    /// A compatible page existed, but allocation still failed.
    Storage(AllocationError),
}

impl fmt::Display for AtlasAllocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCompatiblePage => {
                f.write_str("no compatible atlas page is registered for this request")
            }
            Self::Storage(error) => write!(f, "atlas storage allocation failed: {error}"),
        }
    }
}

impl core::error::Error for AtlasAllocationError {}

/// Queue-time validation failure for [`AtlasCache::queue`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasQueueError {
    /// The logical key was already queued or resident with different atlas
    /// metadata.
    ///
    /// Atlas keys are expected to have stable atlas metadata. If a caller
    /// needs a different class or size, that distinction should be reflected
    /// in the logical key rather than queued as contradictory requests for the
    /// same key.
    ConflictingMetadata,
}

impl fmt::Display for AtlasQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConflictingMetadata => f.write_str(
                "atlas keys must not be queued or reused with conflicting atlas metadata",
            ),
        }
    }
}

impl core::error::Error for AtlasQueueError {}

/// One atlas-facing outcome reported by [`AtlasCache::process_queued`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AtlasProcessedRequest<K> {
    /// The queued request resolved to live atlas placement.
    Resolved {
        /// The atlas-facing request that was processed.
        request: ArtifactRequest<K>,
        /// The residency handle assigned to the live resident.
        handle: ResidencyHandle,
        /// The resolved atlas placement for the request.
        artifact: ResolvedArtifact<K>,
        /// The total used cost after this admission completed.
        used_cost_after: u32,
        /// Whether this admission crossed the configured soft limit.
        crossed_soft_limit: bool,
        /// Any stale resident displaced by a newer generation of the same key.
        ///
        /// This is a historical placement record. Its rect and page remain
        /// meaningful for diagnostics, but its `PagedAtlasSlot` allocation id
        /// has already been freed and must not be treated as a live storage
        /// handle.
        displaced: Option<ResolvedArtifact<K>>,
    },
    /// The queued request reused an already-resident artifact.
    AlreadyResident {
        /// The atlas-facing request that was processed.
        request: ArtifactRequest<K>,
        /// The existing residency handle for the live resident.
        handle: ResidencyHandle,
        /// The already-resolved atlas placement for the request.
        ///
        /// Reusing an already-resident artifact also updates recency in the
        /// underlying residency tracker.
        artifact: ResolvedArtifact<K>,
    },
    /// The queued request was rejected by residency policy.
    Rejected {
        /// The atlas-facing request that was processed.
        request: ArtifactRequest<K>,
        /// The caller-defined cost used during processing.
        cost: u32,
        /// Any stale resident displaced by a newer generation of the same key.
        ///
        /// This is a historical placement record. Its rect and page remain
        /// meaningful for diagnostics, but its `PagedAtlasSlot` allocation id
        /// has already been freed and must not be treated as a live storage
        /// handle.
        displaced: Option<ResolvedArtifact<K>>,
    },
    /// The queued request admitted logically but could not be placed
    /// physically, so its new resident was rolled back.
    AllocationRejected {
        /// The atlas-facing request that was processed.
        request: ArtifactRequest<K>,
        /// The caller-defined cost used during processing.
        cost: u32,
        /// Why physical placement failed.
        error: AtlasAllocationError,
        /// Any stale resident displaced by a newer generation of the same key.
        displaced: Option<ResolvedArtifact<K>>,
    },
}

impl<K> AtlasProcessedRequest<K> {
    /// Returns the processed atlas-facing request.
    #[must_use]
    pub const fn request(&self) -> &ArtifactRequest<K> {
        match self {
            Self::Resolved { request, .. }
            | Self::AlreadyResident { request, .. }
            | Self::Rejected { request, .. }
            | Self::AllocationRejected { request, .. } => request,
        }
    }
}

/// Atlas-facing report returned by [`AtlasCache::process_queued`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasProcessingReport<K> {
    processed: Vec<AtlasProcessedRequest<K>>,
    evicted: Vec<ResolvedArtifact<K>>,
}

impl<K> AtlasProcessingReport<K> {
    /// Creates an atlas processing report.
    #[must_use]
    pub const fn new(
        processed: Vec<AtlasProcessedRequest<K>>,
        evicted: Vec<ResolvedArtifact<K>>,
    ) -> Self {
        Self { processed, evicted }
    }

    /// Returns one outcome per processed queued atlas request.
    #[must_use]
    pub fn processed(&self) -> &[AtlasProcessedRequest<K>] {
        &self.processed
    }

    /// Returns the resolved artifacts evicted to make room during processing.
    ///
    /// These are historical placement records. Their rects and pages remain
    /// useful for diagnostics, but their `PagedAtlasSlot` allocation ids have
    /// already been freed and must not be treated as live storage handles.
    #[must_use]
    pub fn evicted(&self) -> &[ResolvedArtifact<K>] {
        &self.evicted
    }
}

/// Summary diagnostics returned by [`AtlasCache::stats`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasCacheStats {
    pending: u32,
    residents: u32,
    pages: RectAtlasSetStats,
}

impl AtlasCacheStats {
    /// Creates atlas cache statistics from raw values.
    #[must_use]
    pub const fn new(pending: u32, residents: u32, pages: RectAtlasSetStats) -> Self {
        Self {
            pending,
            residents,
            pages,
        }
    }

    /// Returns the number of queued atlas requests not yet processed.
    #[must_use]
    pub const fn pending(self) -> u32 {
        self.pending
    }

    /// Returns the number of live atlas residents tracked by the controller.
    #[must_use]
    pub const fn residents(self) -> u32 {
        self.residents
    }

    /// Returns aggregate storage-page statistics for the atlas cache.
    #[must_use]
    pub const fn pages(self) -> RectAtlasSetStats {
        self.pages
    }
}

/// Small atlas composition layer over residency, storage, and page routing.
///
/// This controller keeps atlas-specific glue out of callers: it queues
/// [`ArtifactRequest`] values, delegates residency policy to
/// `cachet_residency`, delegates physical placement to `cachet_storage`, and
/// reports atlas-facing outcomes back to the caller.
#[derive(Clone, Debug)]
pub struct AtlasCache<K> {
    tracker: ResidencyTracker<K>,
    pages: RectAtlasSet,
    router: AtlasPageRouter,
    bindings: ResidencyBindings<AtlasBinding>,
    pending: Vec<ArtifactRequest<K>>,
}

impl<K> AtlasCache<K> {
    /// Creates an atlas cache from a budget, page set, and atlas router.
    #[must_use]
    pub fn new(budget: Budget, pages: RectAtlasSet, router: AtlasPageRouter) -> Self {
        Self {
            tracker: ResidencyTracker::new(budget),
            pages,
            router,
            bindings: ResidencyBindings::new(),
            pending: Vec::new(),
        }
    }

    /// Starts a new epoch in the underlying residency tracker.
    pub fn begin_epoch(&mut self, epoch: Epoch) {
        self.tracker.begin_epoch(epoch);
    }

    /// Returns summary diagnostics for the atlas cache.
    #[must_use]
    pub fn stats(&self) -> AtlasCacheStats {
        AtlasCacheStats::new(
            self.pending.len() as u32,
            self.tracker.residents().count() as u32,
            self.pages.stats(),
        )
    }

    /// Returns page-local statistics for one storage page.
    #[must_use]
    pub fn page_stats(&self, page: AtlasPageId) -> Option<RectAtlasStats> {
        self.pages.page_stats(page)
    }

    /// Returns the underlying residency tracker for inspection.
    #[must_use]
    pub const fn tracker(&self) -> &ResidencyTracker<K> {
        &self.tracker
    }

    /// Returns the storage-side atlas pages for inspection.
    #[must_use]
    pub const fn pages(&self) -> &RectAtlasSet {
        &self.pages
    }
}

impl<K> AtlasCache<K>
where
    K: Eq + Hash,
{
    /// Queues one atlas-facing request for later processing.
    ///
    /// Atlas keys are expected to have stable atlas metadata. If the same key
    /// is already queued or currently resident with a different
    /// [`AtlasClass`] or
    /// [`ArtifactSize`](crate::ArtifactSize), this method rejects the request
    /// with [`AtlasQueueError::ConflictingMetadata`].
    pub fn queue(&mut self, request: ArtifactRequest<K>) -> Result<(), AtlasQueueError> {
        self.ensure_metadata_is_consistent(&request)?;
        self.pending.push(request);
        Ok(())
    }

    /// Marks one live resident as used by handle.
    ///
    /// This is the explicit touch path for callers that already hold a
    /// `ResidencyHandle` between epochs and want to keep recency bookkeeping
    /// current without re-queuing the request.
    pub fn mark_used_handle(&mut self, handle: ResidencyHandle) -> bool {
        self.tracker.mark_used(handle)
    }
    fn ensure_metadata_is_consistent(
        &self,
        request: &ArtifactRequest<K>,
    ) -> Result<(), AtlasQueueError> {
        if self
            .pending
            .iter()
            .any(|queued| self.conflicts_with_request(queued, request))
        {
            return Err(AtlasQueueError::ConflictingMetadata);
        }

        let Some(handle) = self
            .tracker
            .resident_handle_for_key(request.request().key())
        else {
            return Ok(());
        };
        let Some(binding) = self.bindings.get(handle) else {
            debug_assert!(
                false,
                "live atlas residents must have a bound atlas slot before queue-time validation"
            );
            return Ok(());
        };

        if binding.class != request.class()
            || binding.slot.rect().width() != request.size().width()
            || binding.slot.rect().height() != request.size().height()
        {
            return Err(AtlasQueueError::ConflictingMetadata);
        }

        Ok(())
    }

    fn conflicts_with_request(
        &self,
        left: &ArtifactRequest<K>,
        right: &ArtifactRequest<K>,
    ) -> bool {
        left.request().key() == right.request().key()
            && (left.class() != right.class() || left.size() != right.size())
    }
}

impl<K> AtlasCache<K>
where
    K: Clone + Eq + Hash,
{
    /// Returns the resolved artifact currently bound to a logical key.
    #[must_use]
    pub fn resolved_by_key(&self, key: &K) -> Option<ResolvedArtifact<K>> {
        let resident = self.tracker.resident_by_key(key)?;
        self.resolved_from_binding(resident.key().clone(), resident.handle())
    }

    /// Marks one live resident as used by logical key.
    ///
    /// This is the explicit touch path for callers that think in atlas keys
    /// rather than handles. Re-queueing an already-resident request also marks
    /// usage, but this method makes the direct intent visible.
    pub fn mark_used(&mut self, key: &K) -> bool {
        let Some(handle) = self.tracker.resident_handle_for_key(key) else {
            return false;
        };
        self.mark_used_handle(handle)
    }

    /// Processes all queued atlas requests through residency and storage.
    ///
    /// The caller provides a cost model in the same spirit as
    /// [`ResidencyTracker::process_requests`](cachet_residency::ResidencyTracker::process_requests),
    /// but receives atlas-facing outcomes and resolved artifacts back.
    pub fn process_queued<F>(
        &mut self,
        mut cost_for: F,
    ) -> Result<AtlasProcessingReport<K>, AdmissionError>
    where
        F: FnMut(&ArtifactRequest<K>) -> u32,
    {
        let mut pending = core::mem::take(&mut self.pending);
        pending.sort_by_key(|request| Reverse(request.request().priority()));
        for request in &pending {
            self.tracker.request(request.request().clone());
        }

        let mut pending_for_cost = pending.iter();
        let report = self.tracker.process_requests(|request| {
            let pending_request = pending_for_cost
                .next()
                .expect("queued atlas requests must round-trip through residency");
            debug_assert!(
                pending_request.request().key() == request.key()
                    && pending_request.request().priority() == request.priority()
                    && pending_request.request().generation() == request.generation()
            );
            cost_for(pending_request)
        })?;

        let displaced: Vec<_> = report
            .processed()
            .iter()
            .map(|processed| {
                processed
                    .displaced()
                    .map(|resident| self.take_resolved(resident))
            })
            .collect();
        let evicted: Vec<_> = report
            .evicted()
            .iter()
            .map(|resident| self.take_resolved(resident))
            .collect();

        let mut processed_out = Vec::with_capacity(report.processed().len());
        for ((processed, displaced), atlas_request) in report
            .processed()
            .iter()
            .zip(displaced.into_iter())
            .zip(pending.into_iter())
        {
            debug_assert!(
                atlas_request.request().key() == processed.request().key()
                    && atlas_request.request().priority() == processed.request().priority()
                    && atlas_request.request().generation() == processed.request().generation()
            );

            match processed {
                ProcessedRequest::AlreadyResident { handle, .. } => {
                    let artifact = self
                        .resolved_from_binding(atlas_request.request().key().clone(), *handle)
                        .expect("already-resident atlas request must have a bound slot");
                    processed_out.push(AtlasProcessedRequest::AlreadyResident {
                        request: atlas_request,
                        handle: *handle,
                        artifact,
                    });
                }
                ProcessedRequest::Rejected { cost, .. } => {
                    processed_out.push(AtlasProcessedRequest::Rejected {
                        request: atlas_request,
                        cost: *cost,
                        displaced,
                    });
                }
                ProcessedRequest::Admitted {
                    handle,
                    used_cost_after,
                    crossed_soft_limit,
                    ..
                } => match self.allocate_for_request(&atlas_request) {
                    Ok(slot) => {
                        let _ = self.bindings.bind(
                            *handle,
                            AtlasBinding {
                                class: atlas_request.class(),
                                slot,
                            },
                        );
                        let artifact = ResolvedArtifact::new(
                            atlas_request.request().key().clone(),
                            atlas_request.class(),
                            slot,
                        );
                        processed_out.push(AtlasProcessedRequest::Resolved {
                            request: atlas_request,
                            handle: *handle,
                            artifact,
                            used_cost_after: *used_cost_after,
                            crossed_soft_limit: *crossed_soft_limit,
                            displaced,
                        });
                    }
                    Err(error) => {
                        let removed = self
                            .tracker
                            .remove(*handle)
                            .expect("newly admitted resident must be removable on rollback");
                        processed_out.push(AtlasProcessedRequest::AllocationRejected {
                            request: atlas_request,
                            cost: removed.cost(),
                            error,
                            displaced,
                        });
                    }
                },
            }
        }

        Ok(AtlasProcessingReport::new(processed_out, evicted))
    }

    fn allocate_for_request(
        &mut self,
        request: &ArtifactRequest<K>,
    ) -> Result<PagedAtlasSlot, AtlasAllocationError> {
        let mut last_error = None;
        for page in self.router.candidate_pages_for(request, &self.pages) {
            match self
                .pages
                .allocate_in(page, request.size().width(), request.size().height())
            {
                Ok(slot) => return Ok(slot),
                Err(AllocationError::Exhausted) => {
                    last_error = Some(AtlasAllocationError::Storage(AllocationError::Exhausted));
                }
                Err(error) => return Err(AtlasAllocationError::Storage(error)),
            }
        }
        Err(last_error.unwrap_or(AtlasAllocationError::NoCompatiblePage))
    }

    fn take_resolved(&mut self, resident: &ResidentEntry<K>) -> ResolvedArtifact<K> {
        let binding = self
            .bindings
            .unbind(resident.handle())
            .expect("live atlas residents must have a bound atlas slot");
        let _ = self
            .pages
            .free(binding.slot)
            .expect("bound atlas slots must still be freeable");
        ResolvedArtifact::new(resident.key().clone(), binding.class, binding.slot)
    }

    fn resolved_from_binding(
        &self,
        key: K,
        handle: ResidencyHandle,
    ) -> Option<ResolvedArtifact<K>> {
        let binding = self.bindings.get(handle)?;
        Some(ResolvedArtifact::new(key, binding.class, binding.slot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cachet_residency::{Budget, Priority};
    use cachet_storage::RectAtlasSetStats;

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    struct GlyphKey(&'static str);

    #[test]
    fn atlas_cache_routes_across_pages_and_reuses_evicted_space() {
        let mut pages = RectAtlasSet::new();
        let page0 = pages.add_page(6, 4).expect("page id fits");
        let page1 = pages.add_page(6, 4).expect("page id fits");

        let mut router = AtlasPageRouter::new();
        assert!(router.register_page(AtlasClass::new(1), page0));
        assert!(router.register_page(AtlasClass::new(1), page1));

        let mut cache = AtlasCache::new(Budget::new(2, 2), pages, router);
        cache.begin_epoch(Epoch::new(1));
        cache
            .queue(glyph_request("A", 80))
            .expect("metadata should be consistent");
        cache
            .queue(glyph_request("B", 70))
            .expect("metadata should be consistent");

        let first = cache
            .process_queued(|_| 1)
            .expect("initial requests should process");

        assert_eq!(first.evicted().len(), 0);
        let first_resolved = first
            .processed()
            .iter()
            .filter_map(|processed| match processed {
                AtlasProcessedRequest::Resolved { artifact, .. } => Some(*artifact),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(first_resolved.len(), 2);
        assert_ne!(first_resolved[0].page(), first_resolved[1].page());

        cache.begin_epoch(Epoch::new(2));
        cache
            .queue(glyph_request("C", 90))
            .expect("metadata should be consistent");
        let second = cache
            .process_queued(|_| 1)
            .expect("follow-up requests should process");

        assert_eq!(second.evicted().len(), 1);
        assert_eq!(second.evicted()[0].key(), &GlyphKey("B"));

        let resolved_c = second
            .processed()
            .iter()
            .find_map(|processed| match processed {
                AtlasProcessedRequest::Resolved { artifact, .. }
                    if artifact.key() == &GlyphKey("C") =>
                {
                    Some(*artifact)
                }
                _ => None,
            })
            .expect("glyph C should resolve");
        assert_eq!(resolved_c.page(), page1);
        assert_eq!(resolved_c.rect(), second.evicted()[0].rect());

        let stats = cache.pages().stats();
        assert_eq!(stats, RectAtlasSetStats::new(2, 2, 0));
        assert!(cache.resolved_by_key(&GlyphKey("A")).is_some());
        assert!(cache.resolved_by_key(&GlyphKey("B")).is_none());
        assert!(cache.resolved_by_key(&GlyphKey("C")).is_some());
    }

    #[test]
    fn atlas_cache_exposes_explicit_touch_paths() {
        let mut pages = RectAtlasSet::new();
        let page = pages.add_page(6, 4).expect("page id fits");

        let mut router = AtlasPageRouter::new();
        assert!(router.register_page(AtlasClass::new(1), page));

        let mut cache = AtlasCache::new(Budget::new(1, 1), pages, router);
        cache.begin_epoch(Epoch::new(1));
        cache
            .queue(glyph_request("A", 80))
            .expect("metadata should be consistent");
        let report = cache.process_queued(|_| 1).expect("request should resolve");

        let handle = report
            .processed()
            .iter()
            .find_map(|processed| match processed {
                AtlasProcessedRequest::Resolved { handle, .. } => Some(*handle),
                _ => None,
            })
            .expect("glyph A should resolve");

        cache.begin_epoch(Epoch::new(2));
        assert!(cache.mark_used(&GlyphKey("A")));
        assert!(cache.mark_used_handle(handle));
        assert_eq!(
            cache
                .tracker()
                .resident(handle)
                .expect("glyph A should still be resident")
                .last_touched()
                .get(),
            2
        );
    }

    #[test]
    fn atlas_cache_rejects_conflicting_pending_metadata_for_the_same_key() {
        let mut pages = RectAtlasSet::new();
        let page = pages.add_page(6, 4).expect("page id fits");

        let mut router = AtlasPageRouter::new();
        assert!(router.register_page(AtlasClass::new(1), page));
        assert!(router.register_page(AtlasClass::new(2), page));

        let mut cache = AtlasCache::new(Budget::new(2, 2), pages, router);
        cache
            .queue(glyph_request("A", 80))
            .expect("first request should queue");

        let error = cache
            .queue(ArtifactRequest::new(
                GlyphKey("A"),
                AtlasClass::new(2),
                crate::ArtifactSize::new(6, 4),
                Priority::new(0, 10),
                0,
            ))
            .expect_err("same key must not queue with conflicting atlas metadata");

        assert_eq!(error, AtlasQueueError::ConflictingMetadata);
    }

    #[test]
    fn atlas_cache_rejects_conflicting_metadata_for_a_live_resident() {
        let mut pages = RectAtlasSet::new();
        let page = pages.add_page(6, 4).expect("page id fits");

        let mut router = AtlasPageRouter::new();
        assert!(router.register_page(AtlasClass::new(1), page));
        assert!(router.register_page(AtlasClass::new(2), page));

        let mut cache = AtlasCache::new(Budget::new(1, 1), pages, router);
        cache.begin_epoch(Epoch::new(1));
        cache
            .queue(glyph_request("A", 80))
            .expect("first request should queue");
        let _ = cache.process_queued(|_| 1).expect("request should resolve");

        let error = cache
            .queue(ArtifactRequest::new(
                GlyphKey("A"),
                AtlasClass::new(2),
                crate::ArtifactSize::new(6, 4),
                Priority::new(0, 80),
                0,
            ))
            .expect_err("live residents define stable atlas metadata for their keys");

        assert_eq!(error, AtlasQueueError::ConflictingMetadata);
    }

    fn glyph_request(name: &'static str, priority: u32) -> ArtifactRequest<GlyphKey> {
        ArtifactRequest::new(
            GlyphKey(name),
            AtlasClass::new(1),
            crate::ArtifactSize::new(6, 4),
            Priority::new(0, priority),
            0,
        )
    }
}

// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Cross-crate integration tests for the Cachet facade crate.

use cachet::{atlas, residency, storage};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct GlyphKey(&'static str);

#[test]
fn atlas_cache_reclaims_space_across_multiple_pages() {
    let mut pages = storage::RectAtlasSet::new();
    let page0 = pages.add_page(6, 4).expect("page id fits");
    let page1 = pages.add_page(6, 4).expect("page id fits");

    let mut router = atlas::AtlasPageRouter::new();
    assert!(router.register_page(atlas::AtlasClass::new(1), page0));
    assert!(router.register_page(atlas::AtlasClass::new(1), page1));

    let mut cache = atlas::AtlasCache::new(residency::Budget::new(2, 2), pages, router);
    cache.begin_epoch(residency::Epoch::new(1));
    cache
        .queue(glyph_request("A", 80))
        .expect("metadata should be consistent");
    cache
        .queue(glyph_request("B", 70))
        .expect("metadata should be consistent");

    let first = cache
        .process_queued(|_| 1)
        .expect("first wave should resolve");

    let first_resolved = first
        .processed()
        .iter()
        .filter_map(|processed| match processed {
            atlas::AtlasProcessedRequest::Resolved { artifact, .. } => Some(*artifact),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(first_resolved.len(), 2);
    assert_ne!(first_resolved[0].page(), first_resolved[1].page());

    cache.begin_epoch(residency::Epoch::new(2));
    cache
        .queue(glyph_request("C", 90))
        .expect("metadata should be consistent");
    let second = cache
        .process_queued(|_| 1)
        .expect("second wave should resolve");

    assert_eq!(second.evicted().len(), 1);
    assert_eq!(second.evicted()[0].key(), &GlyphKey("B"));

    let resolved_c = second
        .processed()
        .iter()
        .find_map(|processed| match processed {
            atlas::AtlasProcessedRequest::Resolved { artifact, .. }
                if artifact.key() == &GlyphKey("C") =>
            {
                Some(*artifact)
            }
            _ => None,
        })
        .expect("glyph C should resolve");
    assert_eq!(resolved_c.page(), page1);
    assert_eq!(resolved_c.rect(), second.evicted()[0].rect());

    assert!(cache.resolved_by_key(&GlyphKey("A")).is_some());
    assert!(cache.resolved_by_key(&GlyphKey("B")).is_none());
    assert!(cache.resolved_by_key(&GlyphKey("C")).is_some());
    assert_eq!(cache.pages().stats().allocated(), 2);
}

fn glyph_request(name: &'static str, priority: u32) -> atlas::ArtifactRequest<GlyphKey> {
    atlas::ArtifactRequest::new(
        GlyphKey(name),
        atlas::AtlasClass::new(1),
        atlas::ArtifactSize::new(6, 4),
        residency::Priority::new(0, priority),
        0,
    )
}

// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Cross-crate integration tests for the Cachet facade crate.

use cachet::{atlas, residency, storage};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GlyphKey(&'static str);

#[test]
fn atlas_flow_reclaims_space_after_eviction() {
    let mut tracker = residency::ResidencyTracker::new(residency::Budget::new(2, 2));
    let mut bindings = residency::ResidencyBindings::new();
    let mut atlas_page = storage::RectAtlas::new(16, 16);

    tracker.begin_epoch(residency::Epoch::new(1));
    tracker.request(glyph_request("A", 80).request().clone());
    tracker.request(glyph_request("B", 70).request().clone());

    let first_report = tracker
        .process_requests(|_| 1)
        .expect("first wave should admit");
    let mut resolved = resolve_admitted(&first_report, &mut bindings, &mut atlas_page);

    assert_eq!(resolved.len(), 2);
    assert_eq!(bindings.len(), 2);
    assert_eq!(resolved[0].class().get(), 1);

    tracker.begin_epoch(residency::Epoch::new(2));
    let b_handle = tracker
        .resident_handle_for_key(&GlyphKey("B"))
        .expect("glyph B should be resident");
    assert!(tracker.mark_used(b_handle));

    tracker.request(glyph_request("C", 90).request().clone());
    let second_report = tracker
        .process_requests(|_| 1)
        .expect("second wave should admit after eviction");

    assert_eq!(second_report.evicted().len(), 1);
    let evicted = &second_report.evicted()[0];
    assert_eq!(evicted.key(), &GlyphKey("B"));

    let freed_allocation = bindings
        .unbind(evicted.handle())
        .expect("evicted glyph should have a bound allocation");
    let freed_rect = atlas_page
        .free(freed_allocation)
        .expect("live atlas allocation can be freed");
    resolved.retain(|artifact| artifact.key() != &GlyphKey("B"));

    let mut reused = resolve_admitted(&second_report, &mut bindings, &mut atlas_page);
    assert_eq!(reused.len(), 1);
    assert_eq!(reused[0].key(), &GlyphKey("C"));
    assert_eq!(reused[0].rect(), freed_rect);

    resolved.append(&mut reused);
    assert_eq!(resolved.len(), 2);
    assert!(tracker.resident_by_key(&GlyphKey("A")).is_some());
    assert!(tracker.resident_by_key(&GlyphKey("B")).is_none());
    assert!(tracker.resident_by_key(&GlyphKey("C")).is_some());
}

fn resolve_admitted(
    report: &residency::RequestProcessingReport<GlyphKey>,
    bindings: &mut residency::ResidencyBindings<storage::AllocationId>,
    atlas_page: &mut storage::RectAtlas,
) -> Vec<atlas::ResolvedArtifact<GlyphKey>> {
    let mut resolved = Vec::new();
    for processed in report.processed() {
        if let residency::ProcessedRequest::Admitted {
            request, handle, ..
        } = processed
        {
            let slot = atlas_page
                .allocate(6, 4)
                .expect("integration atlas page should have room");
            let _ = bindings.bind(*handle, slot.allocation());
            resolved.push(atlas::ResolvedArtifact::new(
                request.key().clone(),
                atlas::AtlasClass::new(1),
                slot,
            ));
        }
    }
    resolved
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

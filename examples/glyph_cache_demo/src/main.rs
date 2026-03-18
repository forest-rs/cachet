// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runnable atlas-oriented glyph cache story for Cachet.
//!
//! This example shows the normal atlas flow:
//!
//! 1. describe glyphs with `cachet::atlas`
//! 2. track their logical residency with `cachet::residency`
//! 3. assign rects with `cachet::storage`
//! 4. expose resolved atlas metadata back to the caller
//!
//! Run it with:
//!
//! ```text
//! cargo run -p glyph_cache_demo
//! ```
//!
//! What to look for:
//!
//! - the glyph request vocabulary stays in `cachet::atlas`
//! - the residency kernel knows nothing about glyphs or atlas UVs
//! - the rect atlas owns physical placement, not policy

use cachet::{atlas, residency, storage};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GlyphKey {
    font: &'static str,
    glyph: char,
}

fn main() {
    println!("glyph cache demo");
    println!("================");
    println!();

    let glyphs = demo_glyphs();
    let mut tracker = residency::ResidencyTracker::new(residency::Budget::new(8, 8));
    let mut pages = storage::RectAtlasSet::new();
    let page = pages.add_page(64, 64).expect("demo page id should fit");
    let mut router = atlas::AtlasPageRouter::new();
    assert!(router.register_page(atlas::AtlasClass::new(1), page));
    let mut resolved: Vec<(
        residency::ResidencyHandle,
        atlas::ResolvedArtifact<GlyphKey>,
    )> = Vec::new();

    println!("phase 1: start a frame-like epoch and describe the logical work");
    tracker.begin_epoch(residency::Epoch::new(1));

    for (index, key) in glyphs.into_iter().enumerate() {
        let request = build_glyph_request(key, index);
        println!(
            "  request glyph '{}' from font '{}' as class {} at {}x{} pixels",
            request.request().key().glyph,
            request.request().key().font,
            request.class().get(),
            request.size().width(),
            request.size().height()
        );
        tracker.request(request.request().clone());

        println!("phase 2: admit the glyph into bounded logical residency");
        let handle = tracker
            .admit(request.request(), 1)
            .expect("demo budget should accept each glyph");
        println!("  admitted as resident handle {}", handle.get());

        println!("phase 3: choose a compatible page and assign a physical rect");
        let page = router
            .best_page_for(&request, &pages)
            .expect("demo router should find one compatible page");
        let slot = pages
            .allocate_in(page, request.size().width(), request.size().height())
            .expect("demo atlas should have room");
        let artifact =
            atlas::ResolvedArtifact::new(request.request().key().clone(), request.class(), slot);
        println!(
            "  packed on page {} at ({}, {}) with extent {}x{}",
            artifact.page().get(),
            artifact.rect().x(),
            artifact.rect().y(),
            artifact.rect().width(),
            artifact.rect().height()
        );
        println!();
        resolved.push((handle, artifact));
    }

    println!("phase 4: hand resolved metadata back to the caller");
    for (handle, artifact) in &resolved {
        println!(
            "  glyph '{}' -> handle {} -> page {} -> atlas rect ({}, {}) {}x{}",
            artifact.key().glyph,
            handle.get(),
            artifact.page().get(),
            artifact.rect().x(),
            artifact.rect().y(),
            artifact.rect().width(),
            artifact.rect().height()
        );
    }

    let summary = tracker.end_epoch();
    println!();
    println!("epoch summary");
    println!(
        "  epoch {}: {} requests, {} admissions, used cost {}",
        summary.epoch().get(),
        summary.requests(),
        summary.admissions(),
        summary.used_cost()
    );
}

fn demo_glyphs() -> [GlyphKey; 3] {
    [
        GlyphKey {
            font: "demo-sans",
            glyph: 'A',
        },
        GlyphKey {
            font: "demo-sans",
            glyph: 'B',
        },
        GlyphKey {
            font: "demo-sans",
            glyph: 'C',
        },
    ]
}

fn build_glyph_request(key: GlyphKey, index: usize) -> atlas::ArtifactRequest<GlyphKey> {
    atlas::ArtifactRequest::new(
        key,
        atlas::AtlasClass::new(1),
        atlas::ArtifactSize::new(12, 16),
        residency::Priority::new(0, 100 - index as u32),
        0,
    )
}

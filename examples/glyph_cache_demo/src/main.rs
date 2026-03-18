// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runnable atlas-oriented glyph cache story for Cachet.
//!
//! This example shows the normal atlas flow:
//!
//! 1. describe a bounded hot set of glyphs with `cachet::atlas`
//! 2. process them through `cachet::atlas::AtlasCache`
//! 3. watch one glyph spill to a second page, then get evicted and reused
//! 4. inspect the resolved atlas metadata and page stats returned by the
//!    controller
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
//! - the atlas cache controller composes routing and storage without erasing
//!   their boundary
//! - page stats make the spill and reuse story visible

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

    let mut pages = storage::RectAtlasSet::new();
    let page0 = pages.add_page(12, 16).expect("demo page id should fit");
    let page1 = pages.add_page(12, 16).expect("demo page id should fit");
    let mut router = atlas::AtlasPageRouter::new();
    assert!(router.register_page(atlas::AtlasClass::new(1), page0));
    assert!(router.register_page(atlas::AtlasClass::new(1), page1));
    let mut cache = atlas::AtlasCache::new(residency::Budget::new(2, 2), pages, router);

    println!("phase 1: fill a two-glyph hot set across two small pages");
    cache.begin_epoch(residency::Epoch::new(1));

    for (index, key) in initial_glyphs().into_iter().enumerate() {
        let request = build_glyph_request(key, index);
        println!(
            "  request glyph '{}' from font '{}' as class {} at {}x{} pixels",
            request.request().key().glyph,
            request.request().key().font,
            request.class().get(),
            request.size().width(),
            request.size().height()
        );
        cache
            .queue(request)
            .expect("glyph keys should use stable atlas metadata");
    }

    let first = cache
        .process_queued(|_| 1)
        .expect("demo budget should accept each glyph");
    print_report("  resolved", first.processed());
    print_stats(&cache, &[page0, page1]);
    println!();

    println!("phase 2: request a higher-priority glyph and force churn");
    cache.begin_epoch(residency::Epoch::new(2));
    let request = build_priority_glyph(
        GlyphKey {
            font: "demo-sans",
            glyph: 'C',
        },
        90,
    );
    println!(
        "  request glyph '{}' from font '{}' as class {} at {}x{} pixels",
        request.request().key().glyph,
        request.request().key().font,
        request.class().get(),
        request.size().width(),
        request.size().height()
    );
    cache
        .queue(request)
        .expect("glyph keys should use stable atlas metadata");

    let second = cache
        .process_queued(|_| 1)
        .expect("demo budget should accept the replacement glyph");
    print_report("  processed", second.processed());
    for artifact in second.evicted() {
        println!(
            "  evicted glyph '{}' from page {} rect ({}, {}) {}x{}",
            artifact.key().glyph,
            artifact.page().get(),
            artifact.rect().x(),
            artifact.rect().y(),
            artifact.rect().width(),
            artifact.rect().height()
        );
    }
    print_stats(&cache, &[page0, page1]);
    println!();

    println!("phase 3: inspect the live glyphs after churn");
    for glyph in ['A', 'B', 'C'] {
        let key = GlyphKey {
            font: "demo-sans",
            glyph,
        };
        if let Some(artifact) = cache.resolved_by_key(&key) {
            println!(
                "  glyph '{}' is live on page {} at ({}, {}) {}x{}",
                glyph,
                artifact.page().get(),
                artifact.rect().x(),
                artifact.rect().y(),
                artifact.rect().width(),
                artifact.rect().height()
            );
        } else {
            println!("  glyph '{}' is not resident", glyph);
        }
    }

    let summary = cache.tracker().end_epoch();
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

fn initial_glyphs() -> [GlyphKey; 2] {
    [
        GlyphKey {
            font: "demo-sans",
            glyph: 'A',
        },
        GlyphKey {
            font: "demo-sans",
            glyph: 'B',
        },
    ]
}

fn build_glyph_request(key: GlyphKey, index: usize) -> atlas::ArtifactRequest<GlyphKey> {
    build_priority_glyph(key, 100 - index as u32)
}

fn build_priority_glyph(key: GlyphKey, priority: u32) -> atlas::ArtifactRequest<GlyphKey> {
    atlas::ArtifactRequest::new(
        key,
        atlas::AtlasClass::new(1),
        atlas::ArtifactSize::new(12, 16),
        residency::Priority::new(0, priority),
        0,
    )
}

fn print_report(label: &str, processed: &[atlas::AtlasProcessedRequest<GlyphKey>]) {
    for processed in processed {
        match processed {
            atlas::AtlasProcessedRequest::Resolved {
                handle, artifact, ..
            }
            | atlas::AtlasProcessedRequest::AlreadyResident {
                handle, artifact, ..
            } => {
                println!(
                    "{} glyph '{}' -> handle {} -> page {} -> atlas rect ({}, {}) {}x{}",
                    label,
                    artifact.key().glyph,
                    handle.get(),
                    artifact.page().get(),
                    artifact.rect().x(),
                    artifact.rect().y(),
                    artifact.rect().width(),
                    artifact.rect().height()
                );
            }
            atlas::AtlasProcessedRequest::Rejected { request, cost, .. } => {
                println!(
                    "{} glyph '{}' rejected by residency at cost {}",
                    label,
                    request.request().key().glyph,
                    cost
                );
            }
            atlas::AtlasProcessedRequest::AllocationRejected { request, error, .. } => {
                println!(
                    "{} glyph '{}' rejected by storage with {:?}",
                    label,
                    request.request().key().glyph,
                    error
                );
            }
        }
    }
}

fn print_stats(cache: &atlas::AtlasCache<GlyphKey>, pages: &[storage::AtlasPageId]) {
    let stats = cache.stats();
    println!(
        "  cache stats: pending {}, residents {}, pages {}, allocated {}, free regions {}",
        stats.pending(),
        stats.residents(),
        stats.pages().pages(),
        stats.pages().allocated(),
        stats.pages().free_regions()
    );
    for page in pages {
        let stats = cache.page_stats(*page).expect("page stats exist");
        println!(
            "    page {} -> allocated {}, free regions {}, next cursor ({}, {})",
            page.get(),
            stats.allocated(),
            stats.free_regions(),
            stats.next_x(),
            stats.next_y()
        );
    }
}

// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Demonstrates separate CPU-backed R8 coverage and RGBA8 color glyph atlases.

use std::error::Error;

use cachet::{
    AtlasConfig, CpuAtlasCache, CpuAtlasConfig, Evicted, Extent, Raster, ReservationStatus,
    UploadRegion,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct CoverageGlyph {
    font: u32,
    glyph: u16,
    pixels_per_em: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ColorGlyph {
    font: u32,
    glyph: u16,
    pixels_per_em: u16,
    palette: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GlyphMetadata {
    bearing: [i16; 2],
    advance: i16,
}

fn main() -> Result<(), Box<dyn Error>> {
    let placement = AtlasConfig::new(Extent::new(64, 64), 2).with_padding(1);
    let mut coverage = CpuAtlasCache::new(CpuAtlasConfig::new(placement, 1))?;
    let mut color = CpuAtlasCache::new(CpuAtlasConfig::new(placement, 4))?;

    let coverage_key = CoverageGlyph {
        font: 1,
        glyph: 42,
        pixels_per_em: 24,
    };
    let mut coverage_evictions: Vec<Evicted<CoverageGlyph, GlyphMetadata>> = Vec::new();
    let coverage_reservation =
        coverage.reserve(coverage_key, Extent::new(3, 4), &mut coverage_evictions)?;
    assert_eq!(coverage_reservation.status(), ReservationStatus::Vacant);
    let coverage_pixels = [
        0, 80, 0, //
        80, 255, 80, //
        255, 255, 255, //
        80, 0, 80,
    ];
    let coverage_publication = coverage.populate(
        coverage_reservation.entry(),
        Raster::new(Extent::new(3, 4), 3, &coverage_pixels),
        GlyphMetadata {
            bearing: [0, 4],
            advance: 4,
        },
    )?;

    let color_key = ColorGlyph {
        font: 1,
        glyph: 900,
        pixels_per_em: 24,
        palette: 2,
    };
    let mut color_evictions: Vec<Evicted<ColorGlyph, GlyphMetadata>> = Vec::new();
    let color_reservation = color.reserve(color_key, Extent::new(2, 2), &mut color_evictions)?;
    let color_pixels = [
        255, 30, 30, 255, 30, 255, 30, 255, //
        30, 30, 255, 255, 255, 220, 30, 255,
    ];
    let color_publication = color.populate(
        color_reservation.entry(),
        Raster::new(Extent::new(2, 2), 8, &color_pixels),
        GlyphMetadata {
            bearing: [0, 2],
            advance: 3,
        },
    )?;

    upload_dirty("coverage R8", &mut coverage);
    upload_dirty("color RGBA8", &mut color);

    // The sample upload completes immediately. An asynchronous renderer would
    // move these leases into its submission-retirement queue instead.
    drop(coverage_publication);
    drop(color_publication);

    let coverage_artifact = coverage.lookup(&coverage_key).expect("coverage hit");
    let coverage_entry = coverage_artifact.entry();
    let coverage_placement = coverage_artifact.placement();
    let coverage_metadata = *coverage_artifact.metadata();
    let coverage_draw = coverage.lease([coverage_entry])?;

    let color_artifact = color.lookup(&color_key).expect("color hit");
    let color_entry = color_artifact.entry();
    let color_placement = color_artifact.placement();
    let color_draw = color.lease([color_entry])?;

    println!(
        "coverage page={} rect={:?} bearing={:?}; color page={} rect={:?}",
        coverage_placement.page().index(),
        coverage_placement.content(),
        coverage_metadata.bearing,
        color_placement.page().index(),
        color_placement.content(),
    );
    println!(
        "coverage: {} page bytes, {:.0}% hit rate; color: {} page bytes",
        coverage.stats().page_bytes(),
        coverage.atlas_metrics().hit_rate() * 100.0,
        color.stats().page_bytes(),
    );

    // Prepared draws keep their domain-specific placements alive independently.
    drop((coverage_draw, color_draw));
    Ok(())
}

fn upload_dirty<K, M>(label: &str, cache: &mut CpuAtlasCache<K, M>)
where
    K: Clone + Eq + std::hash::Hash,
{
    let mut regions: Vec<UploadRegion> = Vec::new();
    let checkpoint = cache.collect_uploads(&mut regions);
    for region in &regions {
        let upload = cache
            .page_upload(*region)
            .expect("collected upload is valid");
        println!(
            "upload {label}: page={} rect={:?} bytes={} stride={}",
            region.page().index(),
            region.rect(),
            upload.bytes().len(),
            upload.bytes_per_row(),
        );
    }
    let acknowledged = cache.acknowledge_uploads(checkpoint);
    assert_eq!(acknowledged.regions() as usize, regions.len());
}

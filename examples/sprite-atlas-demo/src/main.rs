// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Demonstrates an RGBA8 CPU-backed atlas with sprite-native keys and metadata.

use std::error::Error;

use cachet::{AtlasConfig, CpuAtlasCache, CpuAtlasConfig, Extent, Raster};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct SpriteKey {
    sheet: u32,
    frame: u16,
    scale: u8,
}

#[derive(Clone, Copy, Debug)]
struct SpriteMetadata {
    pivot: [i16; 2],
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut sprites = CpuAtlasCache::<SpriteKey, SpriteMetadata>::new(CpuAtlasConfig::new(
        AtlasConfig::new(Extent::new(128, 128), 2).with_padding(1),
        4,
    ))?;
    let key = SpriteKey {
        sheet: 12,
        frame: 3,
        scale: 2,
    };
    let extent = Extent::new(4, 2);
    let pixels = [
        255, 0, 0, 255, 255, 128, 0, 255, 255, 255, 0, 255, 255, 255, 255, 255, //
        0, 0, 255, 255, 0, 128, 255, 255, 0, 255, 255, 255, 128, 255, 255, 255,
    ];

    let reservation = sprites.reserve(key, extent, &mut Vec::new())?;
    let publication = sprites.populate(
        reservation.entry(),
        Raster::new(extent, 16, &pixels),
        SpriteMetadata { pivot: [2, 1] },
    )?;

    let mut uploads = Vec::new();
    let checkpoint = sprites.collect_uploads(&mut uploads);
    for region in &uploads {
        let view = sprites.page_upload(*region).expect("valid upload view");
        println!(
            "sprite upload page={} rect={:?} bytes={} stride={}",
            region.page().index(),
            region.rect(),
            view.bytes().len(),
            view.bytes_per_row(),
        );
    }
    sprites.acknowledge_uploads(checkpoint);
    drop(publication);

    let artifact = sprites.lookup(&key).expect("sprite hit");
    let entry = artifact.entry();
    let placement = artifact.placement();
    let pivot = artifact.metadata().pivot;
    let draw_lease = sprites.lease([entry])?;
    println!(
        "draw sprite at page={} rect={:?} pivot={pivot:?}; allocated={} free={}",
        placement.page().index(),
        placement.content(),
        sprites.stats().atlas().allocated_texels(),
        sprites.stats().atlas().free_texels(),
    );
    drop(draw_lease);
    Ok(())
}

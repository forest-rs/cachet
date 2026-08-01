// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Demonstrates caller-managed direct GPU population and submission leases.

use std::{collections::BTreeMap, error::Error};

use cachet::{AtlasCache, AtlasConfig, EntryId, Extent, Lease, PageId, Placement};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PaintGlyph {
    font: u32,
    glyph: u16,
    palette: u16,
}

#[derive(Clone, Copy, Debug)]
struct GlyphMetadata {
    advance: f32,
}

#[derive(Debug)]
struct GpuPage {
    extent: Extent,
}

#[derive(Debug)]
struct EncodedWrite {
    placement: Placement,
}

#[derive(Debug)]
struct Submission {
    serial: u64,
    _lease: Lease,
}

#[derive(Debug, Default)]
struct FakeGpu {
    pages: BTreeMap<PageId, GpuPage>,
    writes: Vec<EncodedWrite>,
    submissions: Vec<Submission>,
    next_serial: u64,
}

impl FakeGpu {
    fn ensure_page(&mut self, page: PageId, extent: Extent) {
        self.pages.entry(page).or_insert(GpuPage { extent });
    }

    fn encode_render(&mut self, placement: Placement) {
        assert!(self.pages.contains_key(&placement.page()));
        self.writes.push(EncodedWrite { placement });
    }

    fn submit(&mut self, lease: Lease) -> u64 {
        self.next_serial += 1;
        let serial = self.next_serial;
        self.submissions.push(Submission {
            serial,
            _lease: lease,
        });
        serial
    }

    fn complete(&mut self, serial: u64) {
        self.submissions
            .retain(|submission| submission.serial > serial);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = AtlasConfig::new(Extent::new(1024, 1024), 2).with_padding(2);
    let mut atlas = AtlasCache::<PaintGlyph, GlyphMetadata>::new(config)?;
    let mut gpu = FakeGpu::default();
    let key = PaintGlyph {
        font: 7,
        glyph: 501,
        palette: 3,
    };

    let reservation = atlas.reserve(key, Extent::new(40, 36), &mut Vec::new())?;
    let placement = reservation.placement();
    gpu.ensure_page(placement.page(), config.page_extent());
    gpu.encode_render(placement);

    // Encoding establishes producer order. Publication makes the entry
    // discoverable and returns a lease before any eviction can intervene.
    let publication = atlas.publish(reservation.entry(), GlyphMetadata { advance: 42.0 })?;
    let producer_serial = gpu.submit(publication.into_lease());
    assert_eq!(atlas.stats().leased_entries(), 1);

    // Tavolo would call this when its real submission tracker observes the
    // fence. Cachet sees only the ordinary Rust lease being dropped.
    gpu.complete(producer_serial);
    assert_eq!(atlas.stats().leased_entries(), 0);

    let artifact = atlas.lookup(&key).expect("published GPU artifact");
    let entry: EntryId = artifact.entry();
    let draw_placement = artifact.placement();
    let advance = artifact.metadata().advance;
    let reader_lease = atlas.lease([entry])?;
    let reader_serial = gpu.submit(reader_lease);

    let page_extent = gpu.pages[&draw_placement.page()].extent;
    let encoded_rect = gpu.writes[0].placement.content();
    println!(
        "GPU page {:?}, encoded {:?}, draw {:?}, advance {advance}",
        page_extent,
        encoded_rect,
        draw_placement.content(),
    );
    println!(
        "pages={}, leased={}, publications={}",
        atlas.stats().pages(),
        atlas.stats().leased_entries(),
        atlas.metrics().publications(),
    );

    gpu.complete(reader_serial);
    assert_eq!(atlas.stats().leased_entries(), 0);
    Ok(())
}

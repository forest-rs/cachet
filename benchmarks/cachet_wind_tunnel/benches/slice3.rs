//! Slice-3 steady-state batch-processing benchmarks.

use cachet_atlas::{
    ArtifactRequest, ArtifactSize, AtlasCache, AtlasClass, AtlasPageRouter, AtlasProcessedRequest,
    AtlasProcessingBatch, AtlasProcessingOutput, AtlasProcessingSink, ResolvedArtifact,
};
use cachet_residency::{
    Budget, Epoch, Priority, ProcessedRequest, Request, RequestProcessingBatch,
    RequestProcessingOutput, RequestProcessingSink, ResidencyTracker, ResidentEntry,
};
use cachet_storage::{AtlasPageId, RectAtlasSet};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

const WORKING_SET: u32 = 64;
const PAGE_COUNT: u32 = 32;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct GlyphKey(u32);

#[derive(Clone, Debug, Default)]
struct ResidencyCountingSink {
    processed: usize,
    evicted: usize,
    checksum: u64,
}

impl ResidencyCountingSink {
    fn reset(&mut self) {
        self.processed = 0;
        self.evicted = 0;
        self.checksum = 0;
    }

    fn checksum(&self) -> u64 {
        self.checksum ^ self.processed as u64 ^ (self.evicted as u64).rotate_left(7)
    }
}

impl RequestProcessingSink<u32> for ResidencyCountingSink {
    fn processed(&mut self, processed: ProcessedRequest<u32>) {
        self.processed += 1;
        self.checksum = self
            .checksum
            .wrapping_add(u64::from(*processed.request().key()));
        if let Some(handle) = processed.handle() {
            self.checksum ^= handle.get();
        }
    }

    fn evicted(&mut self, evicted: ResidentEntry<u32>) {
        self.evicted += 1;
        self.checksum = self.checksum.wrapping_add(u64::from(*evicted.key()));
    }
}

#[derive(Clone, Debug, Default)]
struct AtlasCountingSink {
    processed: usize,
    evicted: usize,
    checksum: u64,
}

impl AtlasCountingSink {
    fn reset(&mut self) {
        self.processed = 0;
        self.evicted = 0;
        self.checksum = 0;
    }

    fn checksum(&self) -> u64 {
        self.checksum ^ self.processed as u64 ^ (self.evicted as u64).rotate_left(11)
    }
}

impl AtlasProcessingSink<GlyphKey> for AtlasCountingSink {
    fn processed(&mut self, processed: AtlasProcessedRequest<GlyphKey>) {
        self.processed += 1;
        self.checksum = self
            .checksum
            .wrapping_add(u64::from(processed.request().request().key().0));
    }

    fn evicted(&mut self, evicted: ResolvedArtifact<GlyphKey>) {
        self.evicted += 1;
        self.checksum = self.checksum.wrapping_add(u64::from(evicted.key().0));
    }
}

fn residency_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("residency_batch");
    group.throughput(Throughput::Elements(u64::from(WORKING_SET)));

    group.bench_function("fresh_output_baseline", |b| {
        let mut tracker = seeded_residency_tracker();
        let mut epoch = 2_u64;
        queue_residency_work(&mut tracker, Epoch::new(epoch));
        let _ = process_residency_owned_baseline(&mut tracker);

        b.iter(|| {
            epoch = epoch.saturating_add(1);
            queue_residency_work(&mut tracker, Epoch::new(epoch));
            let output = process_residency_owned_baseline(&mut tracker);
            black_box(residency_output_checksum(&output));
        });
    });

    group.bench_function("reusable_batch_after", |b| {
        let mut tracker = seeded_residency_tracker();
        let mut batch = RequestProcessingBatch::with_capacity(
            WORKING_SET as usize,
            ResidencyCountingSink::default(),
        );
        let mut epoch = 2_u64;
        queue_residency_work(&mut tracker, Epoch::new(epoch));
        tracker
            .process_requests(&mut batch, |_| 1)
            .expect("warmup processes");

        b.iter(|| {
            epoch = epoch.saturating_add(1);
            batch.sink_mut().reset();
            queue_residency_work(&mut tracker, Epoch::new(epoch));
            tracker
                .process_requests(&mut batch, |_| 1)
                .expect("buffered path processes");
            black_box(batch.sink().checksum());
        });
    });

    group.finish();
}

fn atlas_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("atlas_batch");
    group.throughput(Throughput::Elements(u64::from(WORKING_SET)));

    group.bench_function("fresh_output_baseline", |b| {
        let mut cache = seeded_atlas_cache();
        let mut epoch = 2_u64;
        queue_atlas_work(&mut cache, Epoch::new(epoch));
        let _ = process_atlas_owned_baseline(&mut cache);

        b.iter(|| {
            epoch = epoch.saturating_add(1);
            queue_atlas_work(&mut cache, Epoch::new(epoch));
            let output = process_atlas_owned_baseline(&mut cache);
            black_box(atlas_output_checksum(&output));
        });
    });

    group.bench_function("reusable_batch_after", |b| {
        let mut cache = seeded_atlas_cache();
        let mut batch =
            AtlasProcessingBatch::with_capacity(WORKING_SET as usize, AtlasCountingSink::default());
        let mut epoch = 2_u64;
        queue_atlas_work(&mut cache, Epoch::new(epoch));
        cache
            .process_queued(&mut batch, |_| 1)
            .expect("warmup processes");

        b.iter(|| {
            epoch = epoch.saturating_add(1);
            batch.sink_mut().reset();
            queue_atlas_work(&mut cache, Epoch::new(epoch));
            cache
                .process_queued(&mut batch, |_| 1)
                .expect("buffered path processes");
            black_box(batch.sink().checksum());
        });
    });

    group.finish();
}

fn candidate_pages(c: &mut Criterion) {
    let mut group = c.benchmark_group("candidate_pages");
    group.throughput(Throughput::Elements(u64::from(PAGE_COUNT)));

    group.bench_function("fresh_output_baseline", |b| {
        let (router, pages, request) = routed_pages();
        b.iter(|| {
            let candidates = candidate_pages_fresh_output_baseline(
                &router,
                black_box(&request),
                black_box(&pages),
            );
            black_box(candidate_checksum(&candidates));
        });
    });

    group.bench_function("reusable_output_after", |b| {
        let (router, pages, request) = routed_pages();
        let mut candidates = Vec::with_capacity(PAGE_COUNT as usize);
        router.candidate_pages(&request, &pages, &mut candidates);

        b.iter(|| {
            router.candidate_pages(black_box(&request), black_box(&pages), &mut candidates);
            black_box(candidate_checksum(&candidates));
        });
    });

    group.finish();
}

fn seeded_residency_tracker() -> ResidencyTracker<u32> {
    let mut tracker = ResidencyTracker::new(Budget::new(WORKING_SET, WORKING_SET));
    tracker.begin_epoch(Epoch::new(1));
    for key in 0..WORKING_SET {
        let request = Request::new(key, Priority::new(1, key), 0);
        tracker.admit(&request, 1).expect("warm resident fits");
    }
    tracker
}

fn queue_residency_work(tracker: &mut ResidencyTracker<u32>, epoch: Epoch) {
    tracker.begin_epoch(epoch);
    for key in 0..WORKING_SET {
        tracker.request(Request::new(key, Priority::new(1, key), 0));
    }
}

fn seeded_atlas_cache() -> AtlasCache<GlyphKey> {
    let mut pages = RectAtlasSet::new();
    let page = pages.add_page(128, 128).expect("page id fits");
    let mut router = AtlasPageRouter::new();
    assert!(router.register_page(AtlasClass::new(1), page));

    let mut cache = AtlasCache::new(Budget::new(WORKING_SET, WORKING_SET), pages, router);
    cache.begin_epoch(Epoch::new(1));
    for key in 0..WORKING_SET {
        cache
            .queue(glyph_request(key, key))
            .expect("metadata should be stable");
    }
    let _ = process_atlas_owned_baseline(&mut cache);
    cache
}

fn queue_atlas_work(cache: &mut AtlasCache<GlyphKey>, epoch: Epoch) {
    cache.begin_epoch(epoch);
    for key in 0..WORKING_SET {
        cache
            .queue(glyph_request(key, key))
            .expect("metadata should be stable");
    }
}

fn routed_pages() -> (AtlasPageRouter, RectAtlasSet, ArtifactRequest<GlyphKey>) {
    let mut pages = RectAtlasSet::new();
    let mut router = AtlasPageRouter::new();

    for index in 0..PAGE_COUNT {
        let page = pages.add_page(32, 32).expect("page id fits");
        assert!(router.register_page(AtlasClass::new(1), page));
        for _ in 0..(index % 4) {
            let _ = pages.allocate_in(page, 2, 2).expect("page has room");
        }
    }

    (router, pages, glyph_request(0, 100))
}

fn glyph_request(key: u32, priority: u32) -> ArtifactRequest<GlyphKey> {
    ArtifactRequest::new(
        GlyphKey(key),
        AtlasClass::new(1),
        ArtifactSize::new(4, 4),
        Priority::new(1, priority),
        0,
    )
}

fn process_residency_owned_baseline(
    tracker: &mut ResidencyTracker<u32>,
) -> RequestProcessingOutput<u32> {
    let mut batch = RequestProcessingBatch::with_capacity(
        WORKING_SET as usize,
        RequestProcessingOutput::with_capacity(WORKING_SET as usize),
    );
    tracker
        .process_requests(&mut batch, |_| 1)
        .expect("owned baseline path processes");
    batch.into_sink()
}

fn process_atlas_owned_baseline(
    cache: &mut AtlasCache<GlyphKey>,
) -> AtlasProcessingOutput<GlyphKey> {
    let mut batch = AtlasProcessingBatch::with_capacity(
        WORKING_SET as usize,
        AtlasProcessingOutput::with_capacity(WORKING_SET as usize),
    );
    cache
        .process_queued(&mut batch, |_| 1)
        .expect("owned baseline path processes");
    batch.into_sink()
}

fn candidate_pages_fresh_output_baseline<K>(
    router: &AtlasPageRouter,
    request: &ArtifactRequest<K>,
    pages: &RectAtlasSet,
) -> Vec<AtlasPageId> {
    let mut candidates = Vec::new();
    router.candidate_pages(request, pages, &mut candidates);
    candidates
}

fn residency_output_checksum(output: &RequestProcessingOutput<u32>) -> u64 {
    let processed = output.processed().iter().fold(0_u64, |sum, processed| {
        sum.wrapping_add(u64::from(*processed.request().key()))
            .wrapping_add(processed.handle().map_or(0, |handle| handle.get()))
    });
    processed
        .wrapping_add(output.processed().len() as u64)
        .wrapping_add((output.evicted().len() as u64).rotate_left(7))
}

fn atlas_output_checksum(output: &AtlasProcessingOutput<GlyphKey>) -> u64 {
    let processed = output.processed().iter().fold(0_u64, |sum, processed| {
        sum.wrapping_add(u64::from(processed.request().request().key().0))
    });
    processed
        .wrapping_add(output.processed().len() as u64)
        .wrapping_add((output.evicted().len() as u64).rotate_left(11))
}

fn candidate_checksum(pages: &[AtlasPageId]) -> u64 {
    pages.iter().fold(pages.len() as u64, |sum, page| {
        sum.wrapping_add(u64::from(page.get()))
    })
}

criterion_group!(benches, residency_batch, atlas_batch, candidate_pages);
criterion_main!(benches);

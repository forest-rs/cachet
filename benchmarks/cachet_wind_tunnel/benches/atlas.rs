// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Criterion workloads for the public placement and CPU-backed atlas paths.

use std::hint::black_box;

use cachet::{
    AtlasCache, AtlasConfig, CpuAtlasCache, CpuAtlasConfig, EntryId, Extent, Lease, Raster,
};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

const PAGE: Extent = Extent::new(64, 64);
const ARTIFACT: Extent = Extent::new(8, 8);
const PAGE_ENTRIES: u64 = 64;

fn benchmark_ready_paths(criterion: &mut Criterion) {
    let (mut atlas, entry) = one_ready_atlas();
    let mut evicted = Vec::with_capacity(1);
    let mut group = criterion.benchmark_group("ready");
    group.throughput(Throughput::Elements(1));
    group.bench_function("lookup", |bencher| {
        bencher.iter(|| {
            let artifact = atlas.lookup(black_box(&0)).expect("ready artifact");
            black_box((artifact.placement(), artifact.metadata()));
        });
    });
    group.bench_function("reserve_hit", |bencher| {
        bencher.iter(|| {
            evicted.clear();
            black_box(
                atlas
                    .reserve(black_box(0), ARTIFACT, &mut evicted)
                    .expect("ready reservation"),
            );
        });
    });
    group.bench_function("lease_one", |bencher| {
        bencher.iter(|| {
            let lease = atlas.lease([black_box(entry)]).expect("ready lease");
            black_box(lease.entries());
        });
    });
    group.finish();
}

fn benchmark_batch_lease(criterion: &mut Criterion) {
    let (mut atlas, entries) = ready_atlas(32);
    let mut reusable = Lease::empty();
    atlas
        .lease_into(entries.iter().copied(), &mut reusable)
        .expect("warm reusable lease");
    reusable.clear();
    let mut group = criterion.benchmark_group("lease_batch");
    group.throughput(Throughput::Elements(entries.len() as u64));
    group.bench_function("32_entries", |bencher| {
        bencher.iter(|| {
            let lease = atlas
                .lease(entries.iter().copied())
                .expect("ready batch lease");
            black_box(lease.entries());
        });
    });
    group.bench_function("32_entries_reused", |bencher| {
        bencher.iter(|| {
            atlas
                .lease_into(entries.iter().copied(), &mut reusable)
                .expect("ready reusable batch lease");
            black_box(reusable.entries());
            reusable.clear();
        });
    });
    group.finish();
}

fn benchmark_placement_churn(criterion: &mut Criterion) {
    let (mut atlas, _) = ready_atlas(PAGE_ENTRIES);
    let mut next_key = PAGE_ENTRIES;
    let mut evicted = Vec::with_capacity(1);
    let mut group = criterion.benchmark_group("placement");
    group.throughput(Throughput::Elements(1));
    group.bench_function("reserve_publish_evict", |bencher| {
        bencher.iter(|| {
            evicted.clear();
            let reservation = atlas
                .reserve(black_box(next_key), ARTIFACT, &mut evicted)
                .expect("bounded churn reservation");
            let publication = atlas
                .publish(reservation.entry(), next_key as u32)
                .expect("bounded churn publication");
            black_box((publication.placement(), &evicted));
            drop(publication);
            next_key += 1;
        });
    });
    group.finish();
}

fn benchmark_cpu_paths(criterion: &mut Criterion) {
    let mut cache = full_cpu_atlas();
    let pixels = [0x80_u8; 64];
    let mut next_key = PAGE_ENTRIES;
    let mut evicted = Vec::with_capacity(1);
    let mut uploads = Vec::with_capacity(1);
    let mut group = criterion.benchmark_group("cpu");
    group.throughput(Throughput::Bytes(64));
    group.bench_function("reserve_populate_upload_ack", |bencher| {
        bencher.iter(|| {
            evicted.clear();
            let reservation = cache
                .reserve(black_box(next_key), ARTIFACT, &mut evicted)
                .expect("bounded CPU churn reservation");
            let publication = cache
                .populate(
                    reservation.entry(),
                    Raster::new(ARTIFACT, 8, &pixels),
                    next_key as u32,
                )
                .expect("CPU population");
            let checkpoint = cache.collect_uploads(&mut uploads);
            let acknowledged = cache.acknowledge_uploads(checkpoint);
            black_box((publication.placement(), acknowledged, &uploads));
            drop(publication);
            next_key += 1;
        });
    });
    group.finish();

    let cache = one_dirty_cpu_atlas();
    let mut uploads = Vec::with_capacity(1);
    let mut group = criterion.benchmark_group("uploads");
    group.throughput(Throughput::Elements(1));
    group.bench_function("collect_one_dirty_region", |bencher| {
        bencher.iter(|| {
            let checkpoint = cache.collect_uploads(&mut uploads);
            black_box((checkpoint, &uploads));
        });
    });
    group.finish();
}

fn one_ready_atlas() -> (AtlasCache<u64, u32>, EntryId) {
    let (atlas, entries) = ready_atlas(1);
    (atlas, entries[0])
}

fn ready_atlas(count: u64) -> (AtlasCache<u64, u32>, Vec<EntryId>) {
    let mut atlas = AtlasCache::new(AtlasConfig::new(PAGE, 1)).expect("valid atlas");
    let mut entries = Vec::with_capacity(count as usize);
    let mut evicted = Vec::new();
    for key in 0..count {
        let reservation = atlas
            .reserve(key, ARTIFACT, &mut evicted)
            .expect("warm reservation");
        entries.push(reservation.entry());
        drop(
            atlas
                .publish(reservation.entry(), key as u32)
                .expect("warm publication"),
        );
    }
    (atlas, entries)
}

fn full_cpu_atlas() -> CpuAtlasCache<u64, u32> {
    let mut cache = cpu_atlas();
    let pixels = [0x80_u8; 64];
    let mut uploads = Vec::new();
    for key in 0..PAGE_ENTRIES {
        let reservation = cache
            .reserve(key, ARTIFACT, &mut Vec::new())
            .expect("warm CPU reservation");
        drop(
            cache
                .populate(
                    reservation.entry(),
                    Raster::new(ARTIFACT, 8, &pixels),
                    key as u32,
                )
                .expect("warm CPU population"),
        );
    }
    let checkpoint = cache.collect_uploads(&mut uploads);
    cache.acknowledge_uploads(checkpoint);
    cache
}

fn one_dirty_cpu_atlas() -> CpuAtlasCache<u64, u32> {
    let mut cache = cpu_atlas();
    let reservation = cache
        .reserve(1, ARTIFACT, &mut Vec::new())
        .expect("warm CPU reservation");
    drop(
        cache
            .populate(
                reservation.entry(),
                Raster::new(ARTIFACT, 8, &[0x80; 64]),
                1,
            )
            .expect("warm CPU population"),
    );
    cache
}

fn cpu_atlas() -> CpuAtlasCache<u64, u32> {
    CpuAtlasCache::new(CpuAtlasConfig::new(AtlasConfig::new(PAGE, 1), 1)).expect("valid CPU atlas")
}

criterion_group!(
    benches,
    benchmark_ready_paths,
    benchmark_batch_lease,
    benchmark_placement_churn,
    benchmark_cpu_paths
);
criterion_main!(benches);

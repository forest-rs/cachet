// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Allocation phase reporting for warmed public Cachet paths.

#[cfg(feature = "allocation-counting")]
mod counted {
    use std::hint::black_box;

    use allocation_counter::AllocationInfo;
    use cachet::{
        AtlasCache, AtlasConfig, CpuAtlasCache, CpuAtlasConfig, EntryId, Evicted, Extent, Lease,
        Raster, UploadRegion,
    };

    const OPERATIONS: u64 = 10_000;
    const PAGE: Extent = Extent::new(64, 64);
    const ARTIFACT: Extent = Extent::new(8, 8);
    const PAGE_ENTRIES: u64 = 64;

    pub fn run() {
        ready_lookup();
        reserve_hit();
        lease_one();
        lease_batch_owned();
        lease_batch_reused();
        placement_churn();
        cpu_churn();
        collect_uploads();
    }

    fn ready_lookup() {
        let (mut atlas, _) = ready_atlas(1);
        let allocations = report("ready-lookup", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                let artifact = atlas.lookup(black_box(&0)).expect("ready artifact");
                black_box((artifact.placement(), artifact.metadata()));
            }
        });
        assert_zero("ready-lookup", allocations);
    }

    fn reserve_hit() {
        let (mut atlas, _) = ready_atlas(1);
        let mut evicted = Vec::with_capacity(1);
        let allocations = report("reserve-hit", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                evicted.clear();
                black_box(
                    atlas
                        .reserve(black_box(0), ARTIFACT, &mut evicted)
                        .expect("ready reservation"),
                );
            }
        });
        assert_zero("reserve-hit", allocations);
    }

    fn lease_one() {
        let (mut atlas, entries) = ready_atlas(1);
        let allocations = report("lease-one", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                let lease = atlas.lease([entries[0]]).expect("ready lease");
                black_box(lease.entries());
            }
        });
        assert_zero("lease-one", allocations);
    }

    fn lease_batch_owned() {
        let (mut atlas, entries) = ready_atlas(32);
        let allocations = report("lease-batch-32-owned", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                let lease = atlas
                    .lease(entries.iter().copied())
                    .expect("ready batch lease");
                black_box(lease.entries());
            }
        });
        assert_eq!(allocations.count_total, OPERATIONS);
        assert_eq!(allocations.count_current, 0);
    }

    fn lease_batch_reused() {
        let (mut atlas, entries) = ready_atlas(32);
        let mut lease = Lease::empty();
        atlas
            .lease_into(entries.iter().copied(), &mut lease)
            .expect("warm reusable batch lease");
        lease.clear();
        let allocations = report("lease-batch-32-reused", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                atlas
                    .lease_into(entries.iter().copied(), &mut lease)
                    .expect("ready reusable batch lease");
                black_box(lease.entries());
                lease.clear();
            }
        });
        assert_zero("lease-batch-32-reused", allocations);
    }

    fn placement_churn() {
        let (mut atlas, _) = ready_atlas(PAGE_ENTRIES);
        let mut next_key = PAGE_ENTRIES;
        let mut evicted = Vec::with_capacity(1);

        // One complete replacement cycle establishes the repeated allocator
        // and hash-table shape outside the steady-state gate.
        for _ in 0..PAGE_ENTRIES {
            churn_placement(&mut atlas, &mut next_key, &mut evicted);
        }
        let allocations = report("placement-churn-transition", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                churn_placement(&mut atlas, &mut next_key, &mut evicted);
            }
        });
        assert_at_most("placement-churn-transition", allocations, 1);
        let allocations = report("placement-churn-steady", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                churn_placement(&mut atlas, &mut next_key, &mut evicted);
            }
        });
        assert_zero("placement-churn-steady", allocations);
    }

    fn cpu_churn() {
        let mut cache = full_cpu_atlas();
        let pixels = [0x80_u8; 64];
        let mut next_key = PAGE_ENTRIES;
        let mut evicted = Vec::with_capacity(1);
        let mut uploads = Vec::with_capacity(1);

        for _ in 0..PAGE_ENTRIES {
            churn_cpu(
                &mut cache,
                &pixels,
                &mut next_key,
                &mut evicted,
                &mut uploads,
            );
        }
        let allocations = report("cpu-populate-churn-transition", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                churn_cpu(
                    &mut cache,
                    &pixels,
                    &mut next_key,
                    &mut evicted,
                    &mut uploads,
                );
            }
        });
        assert_at_most("cpu-populate-churn-transition", allocations, 1);
        let allocations = report("cpu-populate-churn-steady", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                churn_cpu(
                    &mut cache,
                    &pixels,
                    &mut next_key,
                    &mut evicted,
                    &mut uploads,
                );
            }
        });
        assert_zero("cpu-populate-churn-steady", allocations);
    }

    fn collect_uploads() {
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
        let mut uploads = Vec::with_capacity(1);
        cache.collect_uploads(&mut uploads);
        let allocations = report("collect-one-upload", OPERATIONS, || {
            for _ in 0..OPERATIONS {
                black_box(cache.collect_uploads(&mut uploads));
            }
        });
        assert_zero("collect-one-upload", allocations);
    }

    fn churn_placement(
        atlas: &mut AtlasCache<u64, u32>,
        next_key: &mut u64,
        evicted: &mut Vec<Evicted<u64, u32>>,
    ) {
        evicted.clear();
        let reservation = atlas
            .reserve(*next_key, ARTIFACT, evicted)
            .expect("bounded churn reservation");
        let publication = atlas
            .publish(reservation.entry(), *next_key as u32)
            .expect("bounded churn publication");
        black_box((publication.placement(), &*evicted));
        drop(publication);
        *next_key += 1;
    }

    fn churn_cpu(
        cache: &mut CpuAtlasCache<u64, u32>,
        pixels: &[u8; 64],
        next_key: &mut u64,
        evicted: &mut Vec<Evicted<u64, u32>>,
        uploads: &mut Vec<UploadRegion>,
    ) {
        evicted.clear();
        let reservation = cache
            .reserve(*next_key, ARTIFACT, evicted)
            .expect("bounded CPU churn reservation");
        let publication = cache
            .populate(
                reservation.entry(),
                Raster::new(ARTIFACT, 8, pixels),
                *next_key as u32,
            )
            .expect("CPU population");
        let checkpoint = cache.collect_uploads(uploads);
        black_box(cache.acknowledge_uploads(checkpoint));
        drop(publication);
        *next_key += 1;
    }

    fn report(phase: &str, operations: u64, operation: impl FnOnce()) -> AllocationInfo {
        let allocations = allocation_counter::measure(operation);
        println!(
            "allocations\tphase={phase}\toperations={operations}\tcalls={}\ttotal_bytes={}\tpeak_calls={}\tpeak_bytes={}\tnet_calls={}\tnet_bytes={}",
            allocations.count_total,
            allocations.bytes_total,
            allocations.count_max,
            allocations.bytes_max,
            allocations.count_current,
            allocations.bytes_current,
        );
        allocations
    }

    fn assert_zero(phase: &str, allocations: AllocationInfo) {
        assert_eq!(allocations.count_total, 0, "{phase} allocated");
        assert_eq!(allocations.bytes_total, 0, "{phase} allocated bytes");
        assert_eq!(allocations.count_current, 0, "{phase} retained calls");
        assert_eq!(allocations.bytes_current, 0, "{phase} retained bytes");
    }

    fn assert_at_most(phase: &str, allocations: AllocationInfo, calls: u64) {
        assert!(
            allocations.count_total <= calls,
            "{phase} exceeded its allocation-call budget"
        );
        assert_eq!(allocations.count_current, 0, "{phase} retained calls");
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
        let mut evicted = Vec::new();
        let mut uploads = Vec::new();
        for key in 0..PAGE_ENTRIES {
            let reservation = cache
                .reserve(key, ARTIFACT, &mut evicted)
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

    fn cpu_atlas() -> CpuAtlasCache<u64, u32> {
        CpuAtlasCache::new(CpuAtlasConfig::new(AtlasConfig::new(PAGE, 1), 1))
            .expect("valid CPU atlas")
    }
}

#[cfg(feature = "allocation-counting")]
fn main() {
    counted::run();
}

#[cfg(not(feature = "allocation-counting"))]
fn main() {
    eprintln!(
        "allocation reporting requires: cargo run --release -p cachet_wind_tunnel --features allocation-counting"
    );
}

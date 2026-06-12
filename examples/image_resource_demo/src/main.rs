// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Runnable dense-slot image resource management story for Cachet.
//!
//! This example shows a registry-style consumer that uses
//! `cachet::residency` directly without going through an atlas or tile
//! allocator.
//!
//! Run it with:
//!
//! ```text
//! cargo run -p image_resource_demo
//! ```
//!
//! What to look for:
//!
//! - the registry keeps its own dense slot model
//! - `cachet::residency` handles budget, recency, eviction, and request batching
//! - `cachet::residency::ResidencyBindings` carries the handle-to-slot glue
//! - `RequestProcessingBatch` lets the demo reuse batch scratch across frames
//! - no atlas or tile vocabulary leaks into this use case

use cachet::residency;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ImageHandle(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResidentImage {
    logical: ImageHandle,
    resident: residency::ResidencyHandle,
    dense_slot: u32,
}

struct ImageBatchSink {
    processed: Vec<residency::ProcessedRequest<ImageHandle>>,
    evicted: Vec<residency::ResidentEntry<ImageHandle>>,
}

impl ImageBatchSink {
    fn new() -> Self {
        Self {
            processed: Vec::new(),
            evicted: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.processed.clear();
        self.evicted.clear();
    }
}

impl residency::RequestProcessingSink<ImageHandle> for ImageBatchSink {
    fn processed(&mut self, processed: residency::ProcessedRequest<ImageHandle>) {
        self.processed.push(processed);
    }

    fn evicted(&mut self, evicted: residency::ResidentEntry<ImageHandle>) {
        self.evicted.push(evicted);
    }
}

fn main() {
    println!("image resource demo");
    println!("===================");
    println!();

    let mut tracker = residency::ResidencyTracker::new(residency::Budget::new(2, 2));
    let mut bindings = residency::ResidencyBindings::new();
    let mut free_slots = vec![1_u32, 0_u32];
    let mut slots = Vec::<ResidentImage>::new();
    let mut batch = residency::RequestProcessingBatch::new(ImageBatchSink::new());

    println!("frame 1: queue two images and process them through a reusable batch");
    tracker.begin_epoch(residency::Epoch::new(1));
    let first = residency::Request::new(ImageHandle(10), residency::Priority::new(0, 80), 0);
    let second = residency::Request::new(ImageHandle(11), residency::Priority::new(0, 70), 0);
    tracker.request(first.clone());
    tracker.request(second.clone());

    tracker
        .process_requests(&mut batch, |_| 1)
        .expect("demo handle space should remain");
    assign_admitted_slots(batch.sink(), &mut bindings, &mut free_slots, &mut slots);
    print_slots("after frame 1 admission", &slots);

    println!();
    println!("frame 2: touch one image, queue another, and let the tracker evict for us");
    tracker.begin_epoch(residency::Epoch::new(2));
    batch.sink_mut().clear();
    let first_handle = tracker
        .resident_handle_for_key(&ImageHandle(10))
        .expect("image 10 should still be resident");
    let _ = tracker.mark_used(first_handle);
    println!("  touched image 10 so it is no longer the least-recently-used resident");

    let third = residency::Request::new(ImageHandle(12), residency::Priority::new(0, 90), 0);
    tracker.request(third);
    tracker
        .process_requests(&mut batch, |_| 1)
        .expect("demo handle space should remain");
    for evicted in &batch.sink().evicted {
        let freed_slot = bindings
            .unbind(evicted.handle())
            .expect("evicted residents should have a bound slot");
        println!(
            "  over budget, so evict image {} with resident handle {} from dense slot {}",
            evicted.key().0,
            evicted.handle().get(),
            freed_slot
        );
        free_slots.push(freed_slot);
        slots.retain(|slot| slot.resident != evicted.handle());
    }
    assign_admitted_slots(batch.sink(), &mut bindings, &mut free_slots, &mut slots);

    let summary = tracker.end_epoch();
    println!();
    print_slots("after frame 2 eviction and reuse", &slots);
    println!();
    println!("epoch summary");
    println!(
        "  epoch {}: {} admissions, {} evictions, used cost {}",
        summary.epoch().get(),
        summary.admissions(),
        summary.evictions(),
        summary.used_cost()
    );
}

fn print_slots(label: &str, slots: &[ResidentImage]) {
    println!("{label}");
    for slot in slots {
        println!(
            "  image {} -> resident handle {} in dense slot {}",
            slot.logical.0,
            slot.resident.get(),
            slot.dense_slot
        );
    }
}

fn assign_admitted_slots(
    sink: &ImageBatchSink,
    bindings: &mut residency::ResidencyBindings<u32>,
    free_slots: &mut Vec<u32>,
    slots: &mut Vec<ResidentImage>,
) {
    for processed in &sink.processed {
        if let residency::ProcessedRequest::Admitted {
            request, handle, ..
        } = processed
        {
            let dense_slot = free_slots
                .pop()
                .expect("demo should have a free dense slot");
            let _ = bindings.bind(*handle, dense_slot);
            slots.push(ResidentImage {
                logical: *request.key(),
                resident: *handle,
                dense_slot,
            });
            println!(
                "  admitted image {} into dense slot {} with resident handle {}",
                request.key().0,
                dense_slot,
                handle.get()
            );
        }
    }
}

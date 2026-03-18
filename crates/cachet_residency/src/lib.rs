// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
//! Core residency policy primitives for Cachet.
//!
//! # Overview
//!
//! Goal:
//! provide the small, reusable residency kernel that higher-level Cachet
//! adapters can share.
//!
//! Non-goals:
//! GPU resources, atlas geometry, tiled-surface planning, fallback policy, or
//! background production jobs.
//!
//! This crate owns the boring center:
//!
//! - requests
//! - priorities
//! - budget accounting
//! - epoch bookkeeping
//! - simple resident-entry tracking
//! - least-recently-used eviction for the first slice
//!
//! It intentionally does not own workload-specific fallback semantics,
//! producer execution, or backend resource handles.
//!
//! # First Read
//!
//! The main type here is [`ResidencyTracker`]. A caller submits logical
//! [`Request`] values, explicitly admits the ones it can afford, and uses the
//! returned [`ResidencyHandle`] values to track residency over time.
//!
//! Read the core names literally:
//!
//! - [`Budget`] is passed to [`ResidencyTracker::new`]
//! - [`Epoch`] is passed to [`ResidencyTracker::begin_epoch`]
//! - [`Request`] is passed to [`ResidencyTracker::request`] and
//!   [`ResidencyTracker::admit`]
//! - [`ResidencyHandle`] is returned by [`ResidencyTracker::admit`]
//! - [`ResidentEntry`] is returned by lookup and eviction methods
//! - [`ResidencyBindings`] is the helper for binding handles to caller-owned
//!   metadata
//! - [`RequestProcessingReport`] is returned by
//!   [`ResidencyTracker::process_requests`]
//! - [`EpochSummary`] is returned by [`ResidencyTracker::end_epoch`]
//!
//! ```
//! use cachet_residency::{Budget, Epoch, Priority, Request, ResidencyTracker};
//!
//! let mut tracker = ResidencyTracker::new(Budget::new(4, 4));
//! tracker.begin_epoch(Epoch::new(1));
//!
//! let request = Request::new("glyph:A", Priority::new(0, 100), 0);
//! tracker.request(request.clone());
//! let handle = tracker.admit(&request, 1).expect("budget has room");
//!
//! assert_eq!(tracker.used_cost(), 1);
//! assert_eq!(tracker.resident(handle).expect("resident").key(), &"glyph:A");
//!
//! let summary = tracker.end_epoch();
//! assert_eq!(summary.requests(), 1);
//! assert_eq!(summary.admissions(), 1);
//! ```
//!
//! # Second Read
//!
//! Concepts:
//!
//! - [`Request`]: a caller-owned desire for logical residency
//! - [`ResidencyHandle`]: an opaque identifier for one admitted resident
//! - [`Budget`]: the capacity and soft limit for the current working set
//! - [`Epoch`]: a frame-like counter used for recency bookkeeping
//! - [`EpochSummary`]: counters for one completed epoch
//!
//! Epochs vs generations:
//!
//! - [`Epoch`] answers "when was this resident last used?"
//! - `Request::generation()` answers "which version of this logical content is
//!   this request asking for?"
//!
//! In practice, callers usually advance [`Epoch`] once per frame, pass, or
//! planning cycle, then call [`ResidencyTracker::mark_used`] for the residents
//! they touched during that epoch.
//!
//! Generations are different: callers bump a request generation when the
//! content behind a logical key becomes stale. Re-requesting the same key with
//! a newer generation causes the tracker to displace the stale resident and
//! admit the newer version.
//!
//! A simple rule of thumb is:
//!
//! - use [`Epoch`] for recency
//! - use request generations for freshness
//!
//! Request lifecycle:
//!
//! - queue a [`Request`] with [`ResidencyTracker::request`]
//! - admit it with [`ResidencyTracker::admit`] or
//!   [`ResidencyTracker::process_requests`]
//! - track the resulting resident through its [`ResidencyHandle`]
//! - optionally bind that handle to workload-specific metadata through
//!   [`ResidencyBindings`]
//!
//! Handles vs allocations:
//!
//! - [`ResidencyHandle`] names a logical resident in the policy layer
//! - `cachet_storage::AllocationId` names a physical placement in the storage
//!   layer
//!
//! A dense-slot registry may stop at [`ResidencyHandle`]. Atlas or tile-based
//! workloads usually carry both: one handle for residency policy and one
//! allocation id for physical placement.
//!
//! Budget terms:
//!
//! - [`Budget::capacity`] is the hard limit; admission beyond it fails
//! - [`Budget::soft_limit`] is the preferred working-set ceiling; callers can
//!   cross it, but should treat that as pressure
//!
//! Cost:
//!
//! Cost is an abstract budget unit chosen by the caller. It might represent
//! bytes, tiles, decoded images, upload slots, or some other bounded resource.
//! The kernel only requires that the same cost model be applied consistently
//! within one tracker.
//!
//! Outcome vocabulary:
//!
//! - invalidated: the caller explicitly declared a resident stale
//! - displaced: a newer generation of the same logical key replaced a stale
//!   resident
//! - evicted: a resident was removed to make room under budget pressure
//!
//! This crate is intentionally usable by more than one consumer shape:
//!
//! - atlas adapters such as `cachet_atlas`
//! - surface planners such as `cachet_surface`
//! - dense resident-slot registries and other resource tables
//!
//! Extension points:
//!
//! - richer priority policies can stay outside the kernel and still emit
//!   [`Priority`] values
//! - different eviction policies can replace the first-slice LRU logic later
//! - higher-level crates can map [`ResidencyHandle`] values to atlas rects,
//!   tile slots, array layers, or dense resident slots through
//!   [`ResidencyBindings`]
//!
//! Gotchas:
//!
//! - admission is explicit on purpose; queuing a [`Request`] does not make it
//!   resident
//! - the first slice tracks cost and LRU recency, not a full asynchronous
//!   production pipeline
//! - invalidation and eviction remove bookkeeping; they do not free GPU
//!   resources because this crate does not own any

extern crate alloc;

mod binding;
mod budget;
mod epoch;
mod handle;
mod priority;
mod request;
mod summary;
mod tracker;

pub use binding::ResidencyBindings;
pub use budget::Budget;
pub use epoch::Epoch;
pub use handle::ResidencyHandle;
pub use priority::Priority;
pub use request::Request;
pub use summary::EpochSummary;
pub use tracker::{
    AdmissionError, ProcessedRequest, RequestProcessingReport, ResidencyTracker, ResidentEntry,
};

// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
//! Atlas-oriented request and resolve metadata for Cachet.
//!
//! # Overview
//!
//! Goal:
//! make atlas-style workloads feel explicit and calm on top of the generic
//! residency kernel.
//!
//! Non-goals:
//! glyph rasterization, image decoding, GPU uploads, or atlas binding policy.
//!
//! This adapter turns artifact descriptions into residency requests and
//! attaches atlas-facing resolve metadata to resident entries.
//!
//! # First Read
//!
//! A normal atlas flow has three steps:
//!
//! 1. describe an artifact with [`ArtifactRequest`]
//! 2. admit its logical residency through `cachet_residency`
//! 3. allocate a physical rect through `cachet_storage` and expose a
//!    [`ResolvedArtifact`]
//!
//! Read the atlas names literally:
//!
//! - [`ArtifactRequest`] is the thing you pass onward into the residency layer
//! - [`AtlasClass`] and [`ArtifactSize`] are the extra atlas inputs needed to
//!   build that request
//! - [`ResolvedArtifact`] is what you hand back to callers after storage has
//!   assigned a slot
//!
//! This example wraps one glyph request, admits it, allocates one atlas slot,
//! and then exposes the resolved placement.
//!
//! ```
//! use cachet_atlas::{ArtifactRequest, ArtifactSize, AtlasClass, ResolvedArtifact};
//! use cachet_residency::{Budget, Epoch, Priority, ResidencyTracker};
//! use cachet_storage::RectAtlas;
//!
//! #[derive(Clone, Debug, Eq, Hash, PartialEq)]
//! struct GlyphKey(&'static str);
//!
//! let request = ArtifactRequest::new(
//!     GlyphKey("A"),
//!     AtlasClass::new(1),
//!     ArtifactSize::new(16, 20),
//!     Priority::new(0, 100),
//!     0,
//! );
//!
//! let mut tracker = ResidencyTracker::new(Budget::new(8, 8));
//! tracker.begin_epoch(Epoch::new(1));
//! tracker.request(request.request().clone());
//! let _handle = tracker.admit(request.request(), 1).expect("room for one glyph");
//!
//! let mut atlas = RectAtlas::new(64, 64);
//! let slot = atlas
//!     .allocate(request.size().width(), request.size().height())
//!     .expect("atlas has room");
//! let resolved = ResolvedArtifact::new(
//!     request.request().key().clone(),
//!     request.class(),
//!     slot,
//! );
//!
//! assert_eq!(resolved.class().get(), 1);
//! assert_eq!(resolved.rect().height(), 20);
//! ```
//!
//! # Second Read
//!
//! Concepts:
//!
//! - [`AtlasClass`]: a compatibility bucket for content that can share atlas
//!   placement rules
//! - [`ArtifactSize`]: the requested artifact extent in atlas pixels
//! - [`ArtifactRequest`]: the atlas-facing wrapper around a generic residency
//!   request
//! - [`ResolvedArtifact`]: the resolved atlas placement for a logical key
//!
//! Resolve metadata:
//!
//! `ResolvedArtifact` is the atlas workload's resolve metadata: the answer you
//! hand back after residency has been admitted and storage has assigned a
//! physical slot.
//!
//! Atlas classes vs atlas pages:
//!
//! [`AtlasClass`] is not a concrete page id. It describes which artifacts can
//! share the same atlas family or packing regime. A future multi-page atlas may
//! route one class across several pages, or keep several classes apart even
//! when they all live in the same backend.
//!
//! Extension points:
//!
//! - a future atlas backend can add page classes, multiple pages, or better
//!   packing without changing the basic request vocabulary
//! - callers can use their own key types as long as they fit the
//!   `cachet_residency` requirements
//!
//! Gotchas:
//!
//! - this crate does not decide how the atlas is uploaded or bound to shaders
//! - a missing artifact is still a caller concern; this crate only models the
//!   request and resolved placement vocabulary
//! - [`AtlasClass`] is a compatibility bucket today, not a built-in page id or
//!   multi-page routing mechanism; the first slice still uses a single-page
//!   allocator
// TODO(cachet): Add a multi-page atlas story. Use AtlasClass and related
// workload metadata for page selection without pulling allocation policy up
// into cachet_atlas itself.

extern crate alloc;

mod key;
mod request;
mod resolve;

pub use key::{ArtifactSize, AtlasClass};
pub use request::ArtifactRequest;
pub use resolve::ResolvedArtifact;

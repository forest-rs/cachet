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
//! 3. route to a compatible page, allocate a physical rect through
//!    `cachet_storage`, and expose a [`ResolvedArtifact`]
//!
//! Read the atlas names literally:
//!
//! - [`ArtifactRequest`] is the thing you pass onward into the residency layer
//! - [`AtlasClass`] and [`ArtifactSize`] are the extra atlas inputs needed to
//!   build that request
//! - [`AtlasPageRouter`] is the atlas-side routing policy for choosing among
//!   compatible storage pages
//! - [`AtlasCache`] is the small composition layer that wires residency,
//!   routing, and storage together
//! - [`ResolvedArtifact`] is what you hand back to callers after storage has
//!   assigned a slot
//! - one logical key is expected to have one stable [`AtlasClass`] and
//!   [`ArtifactSize`]
//!
//! This example wraps one glyph request, admits it, allocates one atlas slot,
//! and then exposes the resolved placement.
//!
//! ```
//! use cachet_atlas::{ArtifactRequest, ArtifactSize, AtlasCache, AtlasClass, AtlasPageRouter};
//! use cachet_residency::{Budget, Epoch, Priority};
//! use cachet_storage::RectAtlasSet;
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
//! let mut pages = RectAtlasSet::new();
//! let page = pages.add_page(64, 64).expect("page id fits");
//! let mut router = AtlasPageRouter::new();
//! assert!(router.register_page(AtlasClass::new(1), page));
//!
//! let mut cache = AtlasCache::new(Budget::new(8, 8), pages, router);
//! cache.begin_epoch(Epoch::new(1));
//! cache
//!     .queue(request.clone())
//!     .expect("atlas keys should use stable metadata");
//!
//! let report = cache
//!     .process_queued(|_| 1)
//!     .expect("room for one glyph");
//! let resolved = report
//!     .processed()
//!     .iter()
//!     .find_map(|processed| match processed {
//!         cachet_atlas::AtlasProcessedRequest::Resolved { artifact, .. } => Some(artifact),
//!         _ => None,
//!     })
//!     .expect("glyph resolves");
//!
//! assert_eq!(resolved.class().get(), 1);
//! assert_eq!(resolved.page(), page);
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
//! - [`AtlasPageRouter`]: atlas-side routing policy that maps classes to
//!   compatible storage pages
//! - [`AtlasCache`]: the small atlas composition layer over residency,
//!   routing, and storage
//! - [`ResolvedArtifact`]: the resolved atlas placement for a logical key
//! - stable atlas metadata: one logical key should not be queued or reused
//!   with conflicting [`AtlasClass`] or [`ArtifactSize`]
//!
//! Resolve metadata:
//!
//! `ResolvedArtifact` is the atlas workload's resolve metadata: the answer you
//! hand back after residency has been admitted, a compatible page has been
//! chosen, and storage has assigned a physical slot.
//!
//! Controller flow:
//!
//! [`AtlasCache`] is the intended calm integration path for atlas workloads:
//! queue [`ArtifactRequest`] values, process them once per epoch, and receive
//! atlas-facing outcomes plus any resolved or evicted artifacts back.
//! [`AtlasCache::stats`] and [`AtlasCache::page_stats`] then expose enough
//! diagnostics to explain page spill, occupancy, and fragmentation pressure.
//! [`AtlasCache::queue`] also validates that one logical key maps to stable
//! atlas metadata; if a caller needs a different class or size, that
//! distinction should be reflected in the key itself.
//!
//! Atlas classes vs atlas pages:
//!
//! [`AtlasClass`] is not a concrete page id. It describes which artifacts can
//! share the same atlas family or packing regime. One class may map to several
//! compatible pages, and one backend may host several classes without turning
//! the class into a storage page identifier.
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
//! - [`AtlasPageRouter`] only chooses among compatible pages; it does not own
//!   storage pages or allocation policy
//! - page routing is still explicit composition, not a hidden global service
//! - if one logical key needs different atlas metadata, model that distinction
//!   in the key rather than queuing contradictory requests

extern crate alloc;

mod controller;
mod key;
mod request;
mod resolve;
mod routing;

pub use controller::{
    AtlasAllocationError, AtlasCache, AtlasCacheStats, AtlasProcessedRequest,
    AtlasProcessingReport, AtlasQueueError,
};
pub use key::{ArtifactSize, AtlasClass};
pub use request::ArtifactRequest;
pub use resolve::ResolvedArtifact;
pub use routing::{AtlasPageAssignment, AtlasPageRouter};

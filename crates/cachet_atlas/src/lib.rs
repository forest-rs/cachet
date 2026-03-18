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
//! - [`ResolvedArtifact`] is what you hand back to callers after storage has
//!   assigned a slot
//!
//! This example wraps one glyph request, admits it, allocates one atlas slot,
//! and then exposes the resolved placement.
//!
//! ```
//! use cachet_atlas::{
//!     ArtifactRequest, ArtifactSize, AtlasClass, AtlasPageRouter, ResolvedArtifact,
//! };
//! use cachet_residency::{Budget, Epoch, Priority, ResidencyTracker};
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
//! let mut tracker = ResidencyTracker::new(Budget::new(8, 8));
//! tracker.begin_epoch(Epoch::new(1));
//! tracker.request(request.request().clone());
//! let _handle = tracker.admit(request.request(), 1).expect("room for one glyph");
//!
//! let mut pages = RectAtlasSet::new();
//! let page = pages.add_page(64, 64).expect("page id fits");
//! let mut router = AtlasPageRouter::new();
//! assert!(router.register_page(AtlasClass::new(1), page));
//!
//! let page = router
//!     .best_page_for(&request, &pages)
//!     .expect("one compatible page");
//! let slot = pages
//!     .allocate_in(page, request.size().width(), request.size().height())
//!     .expect("atlas has room");
//! let resolved = ResolvedArtifact::new(
//!     request.request().key().clone(),
//!     request.class(),
//!     slot,
//! );
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
//! - [`ResolvedArtifact`]: the resolved atlas placement for a logical key
//!
//! Resolve metadata:
//!
//! `ResolvedArtifact` is the atlas workload's resolve metadata: the answer you
//! hand back after residency has been admitted, a compatible page has been
//! chosen, and storage has assigned a physical slot.
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

extern crate alloc;

mod key;
mod request;
mod resolve;
mod routing;

pub use key::{ArtifactSize, AtlasClass};
pub use request::ArtifactRequest;
pub use resolve::ResolvedArtifact;
pub use routing::{AtlasPageAssignment, AtlasPageRouter};

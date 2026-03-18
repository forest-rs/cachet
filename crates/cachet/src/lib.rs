// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
//! Documentation-led residency primitives for atlas, surface, and registry
//! workloads.
//!
//! # Overview
//!
//! Goal:
//! provide one calm entry point that explains how Cachet's four core crates fit
//! together.
//!
//! Non-goals:
//! hiding the boundaries between those crates, or pretending the current sketch
//! is more stable than it is.
//!
//! Cachet is split into four pieces:
//!
//! - [`residency`]: generic budgeting, request, invalidation, and eviction
//!   policy
//! - [`storage`]: optional physical allocation strategies such as rect atlases
//!   and fixed tile pools
//! - [`atlas`]: vocabulary for glyph, icon, and image-patch caches
//! - [`surface`]: vocabulary for tiled logical spaces such as maps and
//!   zoomable documents
//!
//! # First Read
//!
//! Most users should start by identifying which shape they need:
//!
//! - glyph cache or image patch cache: start with [`atlas`]
//! - map tiles or zoomable document cache: start with [`surface`]
//! - dense resident-slot registry: start with [`residency`] and add
//!   [`storage`] only if packing becomes necessary
//!
//! A small atlas-oriented flow looks like this:
//!
//! ```
//! use cachet::{atlas, residency, storage};
//!
//! #[derive(Clone, Debug, Eq, Hash, PartialEq)]
//! struct GlyphKey(&'static str);
//!
//! let request = atlas::ArtifactRequest::new(
//!     GlyphKey("A"),
//!     atlas::AtlasClass::new(1),
//!     atlas::ArtifactSize::new(16, 20),
//!     residency::Priority::new(0, 100),
//!     0,
//! );
//!
//! let mut pages = storage::RectAtlasSet::new();
//! let page = pages.add_page(64, 64).expect("page id fits");
//! let mut router = atlas::AtlasPageRouter::new();
//! assert!(router.register_page(request.class(), page));
//!
//! let mut cache = atlas::AtlasCache::new(residency::Budget::new(8, 8), pages, router);
//! cache.begin_epoch(residency::Epoch::new(1));
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
//!         atlas::AtlasProcessedRequest::Resolved { artifact, .. } => Some(artifact),
//!         _ => None,
//!     })
//!     .expect("glyph resolves");
//!
//! assert_eq!(resolved.page(), page);
//! assert_eq!(resolved.rect().width(), 16);
//! ```
//!
//! A small surface-oriented flow looks like this:
//!
//! The surface names are intentionally explicit: `TilePlannerConfig` is passed
//! to `TilePlanner::new`, `TilePlannerView` is passed to `TilePlanner::plan`,
//! and each returned item is a `TilePlanEntry`.
//!
//! ```
//! use cachet::{storage, surface};
//!
//! let planner = surface::TilePlanner::new(
//!     surface::TilePlannerConfig::new(256.0, 1, 0)
//!         .with_priority_tier(2)
//!         .with_generation(5),
//! );
//! let planned = planner.plan(
//!     surface::SurfaceId::new(7),
//!     &surface::TilePlannerView::new(128.0, 128.0, 256.0, 256.0, 1.0),
//! );
//!
//! let key = *planned.first().expect("one center tile").request().key();
//! let _: &surface::TilePlanEntry = planned.first().expect("one center tile");
//! let mut pool = storage::TilePool::new(2);
//! let resolved = surface::ResolvedTile::new(key, pool.allocate().expect("tile slot"));
//!
//! assert_eq!(resolved.key().surface(), surface::SurfaceId::new(7));
//! assert_eq!(planned[0].request().generation(), 5);
//! ```
//!
//! # Second Read
//!
//! Concepts:
//!
//! - logical key: what the caller wants to make resident
//! - resident handle: the generic kernel's opaque identifier for admitted
//!   residency
//! - allocation: an optional physical placement from the storage layer
//! - resolve metadata: workload-facing output such as atlas rects or tile slots
//! - planner: workload-specific logic that turns a view or scene into requests
//!
//! Lifecycle:
//!
//! 1. create or plan a logical request
//! 2. admit it through [`residency`]
//! 3. optionally assign physical placement through [`storage`]
//! 4. return workload-facing resolve metadata to the caller
//!
//! Handles vs allocations:
//!
//! - [`residency::ResidencyHandle`] is the policy-layer identity of a live
//!   resident
//! - `storage::AllocationId` is the storage-layer identity of a physical
//!   placement
//!
//! How the four crates fit together:
//!
//! 1. a workload crate emits logical requests
//! 2. [`residency`] decides what can stay resident
//! 3. [`storage`] optionally assigns physical placement
//! 4. the workload crate exposes resolved metadata back to the caller
//!
//! Registry-style consumers typically keep their own mapping from
//! [`residency::ResidencyHandle`] to workload-facing metadata such as a dense
//! slot index, image handle, or backend resource record. The
//! [`residency::ResidencyBindings`] helper and the `image_resource_demo`
//! example show that pattern without introducing atlas or tile vocabulary into
//! the kernel.
//!
//! Atlas-style consumers now also have a calmer composition path through
//! [`atlas::AtlasCache`], with [`atlas::AtlasCacheStats`] exposing queue and
//! page-pressure diagnostics. One atlas key is also expected to carry stable
//! atlas metadata, so [`atlas::AtlasCache::queue`] rejects contradictory class
//! or size information for the same key.
//!
//! This keeps one use case's nouns from taking over the others:
//!
//! - atlas workloads care about classes, sizes, and packed rects
//! - surface workloads care about view planning and fallback semantics
//! - registry workloads may only need dense resident slots and never touch
//!   [`storage`]
//!
//! Extension points:
//!
//! - add richer atlas and surface policies without widening the kernel
//! - add a backend adapter later without teaching the core about `wgpu`
//! - add more allocator strategies when a second consumer proves the seam
//!
//! Gotchas:
//!
//! - this is still an exploratory sketch, even though the current API is meant
//!   to stay calm
//! - explicit seams are a design feature here; the facade crate is a guide, not
//!   a mandate to erase boundaries

/// Atlas-oriented request and resolve vocabulary.
pub use cachet_atlas as atlas;
/// Generic residency policy primitives.
pub use cachet_residency as residency;
/// Optional physical allocation strategies.
pub use cachet_storage as storage;
/// Surface-oriented planning and resolve vocabulary.
pub use cachet_surface as surface;

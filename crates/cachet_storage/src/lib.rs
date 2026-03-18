// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
//! Physical slot allocators for Cachet.
//!
//! # Overview
//!
//! Goal:
//! provide the optional physical allocation layer that sits between generic
//! residency policy and workload-specific resolve metadata.
//!
//! Non-goals:
//! priority policy, view planning, raster production, GPU uploads, or binding
//! decisions.
//!
//! The current sketch includes both a bounded rect atlas and a fixed-size tile
//! pool so higher-level adapters can share one allocator crate without forcing
//! backend details into the residency kernel. It also includes a small
//! multi-page wrapper around rect atlases so higher layers can route placement
//! without teaching this crate any atlas workload policy.
//!
//! Read the storage names literally:
//!
//! - [`RectAtlas`], [`RectAtlasSet`], and [`TilePool`] are the allocators you
//!   call directly
//! - [`AtlasSlot`], [`PagedAtlasSlot`], and [`TileSlot`] are returned by
//!   allocation methods
//! - [`AllocationId`] is the opaque id that free operations key on
//! - [`RectAtlasStats`], [`RectAtlasSetStats`], and [`TilePoolStats`] are
//!   returned by `stats()`
//!
//! # First Read
//!
//! Use [`RectAtlas`] when your workload packs many small discrete artifacts,
//! [`RectAtlasSet`] when you need several storage pages with explicit page
//! identity, and [`TilePool`] when every resident slot has the same size.
//!
//! This example shows one atlas allocation and one tile allocation so the two
//! allocator shapes can be compared quickly.
//!
//! ```
//! use cachet_storage::{RectAtlas, TilePool};
//!
//! let mut atlas = RectAtlas::new(64, 64);
//! let glyph = atlas.allocate(12, 16).expect("glyph fits");
//! assert_eq!(glyph.rect().width(), 12);
//!
//! let mut pages = cachet_storage::RectAtlasSet::new();
//! let page = pages.add_page(64, 64).expect("page id fits");
//! let packed = pages.allocate_in(page, 10, 10).expect("page has room");
//! assert_eq!(packed.page(), page);
//!
//! let mut tiles = TilePool::new(2);
//! let tile = tiles.allocate().expect("tile slot fits");
//! assert_eq!(tile.index(), 0);
//! ```
//!
//! # Second Read
//!
//! Concepts:
//!
//! - [`AllocationId`]: an opaque identifier for one physical allocation
//! - [`RectAtlas`]: a bounded shelf allocator for atlas-style workloads
//! - [`RectAtlasSet`]: a storage-side collection of atlas pages with explicit
//!   page identity
//! - [`TilePool`]: a fixed-size pool for equal-sized slots
//! - [`AtlasRect`]: the packed rectangle returned by the atlas allocator
//! - [`TileSlot`]: the physical slot returned by the tile pool
//!
//! Allocation ids vs residency handles:
//!
//! - [`AllocationId`] names a physical placement owned by this crate
//! - `cachet_residency::ResidencyHandle` names a logical admitted resident in
//!   the policy layer
//!
//! Workloads that use both layers usually map one to the other rather than
//! treating them as interchangeable.
//!
//! Extension points:
//!
//! - future array or page allocators can live beside the current atlas and tile
//!   allocators
//! - higher-level crates can interpret allocations as UV metadata, page-table
//!   entries, or dense tile indices
//!
//! Gotchas:
//!
//! - this crate owns placement, not residency policy
//! - not every Cachet consumer needs this crate; a dense resident-slot registry
//!   can stop at `cachet_residency`

extern crate alloc;

mod allocation;
mod rect_atlas;
mod tile_pool;

pub use allocation::{AllocationError, AllocationId};
pub use rect_atlas::{
    AtlasPageId, AtlasRect, AtlasSlot, PagedAtlasSlot, RectAtlas, RectAtlasSet, RectAtlasSetStats,
    RectAtlasStats,
};
pub use tile_pool::{TilePool, TilePoolStats, TileSlot};

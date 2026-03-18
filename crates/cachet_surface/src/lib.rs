// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
//! Surface-oriented planning and resolve metadata for Cachet.
//!
//! # Overview
//!
//! Goal:
//! make tiled logical spaces explicit on top of the generic residency kernel.
//!
//! Non-goals:
//! map rendering, document rasterization, GPU page tables, or renderer-global
//! frame orchestration.
//!
//! This adapter turns viewport-like samples into `TileKey` requests and
//! attaches workload-specific fallback metadata to resolved tiles.
//!
//! # First Read
//!
//! Use [`TilePlanner`] to turn a view into an ordered working set. The first
//! slice keeps that planning explicit so map, canvas, and document caches can
//! tune the policy later.
//!
//! Read the names in this crate literally:
//!
//! - [`TilePlannerConfig`] is passed to [`TilePlanner::new`]
//! - [`TilePlannerView`] is passed to [`TilePlanner::plan`]
//! - [`TilePlanEntry`] is one item returned by [`TilePlanner::plan`]
//!
//! ```
//! use cachet_storage::TilePool;
//! use cachet_surface::{
//!     ResolvedTile, SurfaceId, TilePlanEntry, TilePlanner, TilePlannerConfig, TilePlannerView,
//! };
//!
//! let planner = TilePlanner::new(
//!     TilePlannerConfig::new(256.0, 1, 0)
//!         .with_priority_tier(2)
//!         .with_generation(4),
//! );
//! let planned = planner.plan(
//!     SurfaceId::new(42),
//!     &TilePlannerView::new(128.0, 128.0, 256.0, 256.0, 1.0),
//! );
//!
//! let key = *planned.first().expect("at least one tile").request().key();
//! let _: &TilePlanEntry = planned.first().expect("at least one planned tile");
//! let mut pool = TilePool::new(4);
//! let resolved = ResolvedTile::new(key, pool.allocate().expect("tile slot"));
//!
//! assert_eq!(resolved.key().surface(), SurfaceId::new(42));
//! assert_eq!(resolved.key().level(), 0);
//! assert_eq!(planned[0].request().priority().tier(), 2);
//! assert_eq!(planned[0].request().generation(), 4);
//! ```
//!
//! # Second Read
//!
//! Concepts:
//!
//! - [`SurfaceId`]: the caller-owned identity of a large logical surface
//! - [`TileKey`]: one logical tile within that surface
//! - [`TilePlannerView`]: the viewport-like input to planning
//! - [`TilePlanner`]: the component that turns a view into residency requests
//! - [`TilePlannerConfig`]: static configuration for a planner instance
//! - [`TilePlanEntry`]: one planned residency request emitted by a planner
//! - [`ResolvedTile`]: the surface-facing resolve result for a resident tile
//!
//! Extension points:
//!
//! - view planning can grow richer priority heuristics without changing the
//!   generic kernel
//! - fallback semantics can evolve independently from atlas or registry-style
//!   consumers
//!
//! Gotchas:
//!
//! - this crate owns planning vocabulary, not generic residency policy
//! - the first slice keeps fallback metadata simple and explicit

extern crate alloc;

mod key;
mod planner;
mod resolve;

pub use key::{SurfaceId, TileKey};
pub use planner::{TilePlanEntry, TilePlanner, TilePlannerConfig, TilePlannerView};
pub use resolve::{FallbackRef, ResolvedTile};

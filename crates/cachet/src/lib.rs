// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
//! A bounded artifact atlas for glyphs, sprites, icons, and image patches.
//!
//! [`AtlasCache`] owns placement, publication, eviction, and lease-safe reuse
//! without owning pixel storage. Rasterization, pixel interpretation, GPU
//! resources, command submission, and completion remain caller-owned.

extern crate alloc;

mod allocator;
mod cache;
mod config;
mod cpu;
mod error;
mod geometry;
mod model;
mod stats;
mod upload;

pub use cache::AtlasCache;
pub use config::{AtlasConfig, ConfigError};
pub use cpu::{CpuAtlasCache, CpuAtlasConfig, CpuMetrics, CpuPageStats, CpuStats, Raster};
pub use error::{
    AbortError, CpuConfigError, LeaseError, PopulateError, PublishError, ReserveError,
};
pub use geometry::{Extent, Rect};
pub use model::{
    ArtifactRef, EntryId, Evicted, Invalidated, Lease, PageId, Placement, Publication, Reservation,
    ReservationStatus,
};
pub use stats::{CacheMetrics, CacheStats, PageStats};
pub use upload::{PageData, PageUpload, UploadAcknowledgement, UploadCheckpoint, UploadRegion};

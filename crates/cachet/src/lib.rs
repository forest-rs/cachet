// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
//! A bounded artifact atlas for glyphs, sprites, icons, and image patches.
//!
//! [`AtlasCache`] owns placement, publication, eviction, and lease-safe reuse
//! without owning pixel storage. Rasterization, pixel interpretation, GPU
//! resources, command submission, and completion remain caller-owned.
//!
//! CPU-produced artifacts use [`CpuAtlasCache`]:
//!
//! ```
//! use cachet::{AtlasConfig, CpuAtlasCache, CpuAtlasConfig, Extent, Raster};
//!
//! let config = CpuAtlasConfig::new(
//!     AtlasConfig::new(Extent::new(256, 256), 2).with_padding(1),
//!     1, // R8 coverage
//! );
//! let mut cache = CpuAtlasCache::<u32, i16>::new(config)?;
//! let reservation = cache.reserve(42, Extent::new(2, 2), &mut Vec::new())?;
//! let publication = cache.populate(
//!     reservation.entry(),
//!     Raster::new(Extent::new(2, 2), 2, &[0, 255, 255, 0]),
//!     3, // caller-owned bearing metadata
//! )?;
//!
//! // Retain this lease through an asynchronous upload submission.
//! let upload_lease = publication.into_lease();
//! drop(upload_lease);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Direct GPU producers use [`AtlasCache`] without a CPU mirror:
//!
//! ```
//! use cachet::{AtlasCache, AtlasConfig, Extent};
//!
//! let config = AtlasConfig::new(Extent::new(1024, 1024), 2).with_padding(2);
//! let mut atlas = AtlasCache::<u32, ()>::new(config)?;
//! let reservation = atlas.reserve(7, Extent::new(32, 40), &mut Vec::new())?;
//! let placement = reservation.placement();
//! // Create the caller-owned page if necessary, then encode a write to placement.
//! let publication = atlas.publish(reservation.entry(), ())?;
//! // Move this lease into the caller's GPU submission-retirement queue.
//! let producer_lease = publication.into_lease();
//! # let _ = placement;
//! drop(producer_lease);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

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

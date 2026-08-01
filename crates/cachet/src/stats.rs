// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Extent, PageId};

/// Cumulative work counters for one placement cache lifetime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CacheMetrics {
    pub(crate) probes: u64,
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) reservations: u64,
    pub(crate) publications: u64,
    pub(crate) aborts: u64,
    pub(crate) lease_batches: u64,
    pub(crate) leased_entries: u64,
    pub(crate) evictions: u64,
    pub(crate) invalidations: u64,
    pub(crate) deferred_reuses: u64,
    pub(crate) reclaims: u64,
    pub(crate) pressure_failures: u64,
    pub(crate) too_large_rejections: u64,
}

impl CacheMetrics {
    /// Returns lookup and reservation probes.
    #[must_use]
    pub const fn probes(self) -> u64 {
        self.probes
    }

    /// Returns probes that found ready artifacts.
    #[must_use]
    pub const fn hits(self) -> u64 {
        self.hits
    }

    /// Returns probes that did not find ready artifacts.
    #[must_use]
    pub const fn misses(self) -> u64 {
        self.misses
    }

    /// Returns newly allocated physical reservations.
    #[must_use]
    pub const fn reservations(self) -> u64 {
        self.reservations
    }

    /// Returns reservations published successfully.
    #[must_use]
    pub const fn publications(self) -> u64 {
        self.publications
    }

    /// Returns vacant reservations explicitly aborted.
    #[must_use]
    pub const fn aborts(self) -> u64 {
        self.aborts
    }

    /// Returns successful explicit batch-lease operations.
    #[must_use]
    pub const fn lease_batches(self) -> u64 {
        self.lease_batches
    }

    /// Returns distinct entries pinned by explicit batch leases.
    #[must_use]
    pub const fn leased_entries(self) -> u64 {
        self.leased_entries
    }

    /// Returns ready artifacts evicted under capacity pressure.
    #[must_use]
    pub const fn evictions(self) -> u64 {
        self.evictions
    }

    /// Returns explicit key invalidations.
    #[must_use]
    pub const fn invalidations(self) -> u64 {
        self.invalidations
    }

    /// Returns invalidations whose physical reuse waited for a lease.
    #[must_use]
    pub const fn deferred_reuses(self) -> u64 {
        self.deferred_reuses
    }

    /// Returns deferred placements reclaimed after lease release.
    #[must_use]
    pub const fn reclaims(self) -> u64 {
        self.reclaims
    }

    /// Returns failures caused by full non-evictable pages.
    #[must_use]
    pub const fn pressure_failures(self) -> u64 {
        self.pressure_failures
    }

    /// Returns padded artifacts rejected as larger than one page.
    #[must_use]
    pub const fn too_large_rejections(self) -> u64 {
        self.too_large_rejections
    }

    /// Returns ready hits divided by all probes.
    #[must_use]
    pub fn hit_rate(self) -> f64 {
        if self.probes == 0 {
            return 0.0;
        }
        self.hits as f64 / self.probes as f64
    }
}

/// Current packing gauges for one stable page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageStats {
    pub(crate) page: PageId,
    pub(crate) extent: Extent,
    pub(crate) ready_entries: u32,
    pub(crate) reserved_entries: u32,
    pub(crate) retired_entries: u32,
    pub(crate) leased_entries: u32,
    pub(crate) allocated_texels: u64,
    pub(crate) free_texels: u64,
    pub(crate) free_regions: u32,
    pub(crate) largest_free_region: Option<Extent>,
}

impl PageStats {
    /// Returns the page identifier.
    #[must_use]
    pub const fn page(self) -> PageId {
        self.page
    }

    /// Returns the page extent.
    #[must_use]
    pub const fn extent(self) -> Extent {
        self.extent
    }

    /// Returns populated entries on the page.
    #[must_use]
    pub const fn ready_entries(self) -> u32 {
        self.ready_entries
    }

    /// Returns reservations awaiting publication.
    #[must_use]
    pub const fn reserved_entries(self) -> u32 {
        self.reserved_entries
    }

    /// Returns invalidated allocations waiting for lease release.
    #[must_use]
    pub const fn retired_entries(self) -> u32 {
        self.retired_entries
    }

    /// Returns ready or retired entries protected by leases.
    #[must_use]
    pub const fn leased_entries(self) -> u32 {
        self.leased_entries
    }

    /// Returns allocated texels including padding.
    #[must_use]
    pub const fn allocated_texels(self) -> u64 {
        self.allocated_texels
    }

    /// Returns free texels.
    #[must_use]
    pub const fn free_texels(self) -> u64 {
        self.free_texels
    }

    /// Returns free rectangles tracked by the packer.
    #[must_use]
    pub const fn free_regions(self) -> u32 {
        self.free_regions
    }

    /// Returns the extent of the largest free rectangle by area.
    #[must_use]
    pub const fn largest_free_region(self) -> Option<Extent> {
        self.largest_free_region
    }

    /// Returns free texels outside the largest individual free rectangle.
    #[must_use]
    pub const fn stranded_free_texels(self) -> u64 {
        self.free_texels
            .saturating_sub(match self.largest_free_region {
                Some(extent) => extent.area() as u64,
                None => 0,
            })
    }
}

/// Current atlas gauges plus cumulative lifecycle metrics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheStats {
    pub(crate) pages: u32,
    pub(crate) ready_entries: u32,
    pub(crate) reserved_entries: u32,
    pub(crate) retired_entries: u32,
    pub(crate) leased_entries: u32,
    pub(crate) allocated_texels: u64,
    pub(crate) free_texels: u64,
    pub(crate) free_regions: u32,
    pub(crate) metrics: CacheMetrics,
}

impl CacheStats {
    /// Returns lazily allocated pages.
    #[must_use]
    pub const fn pages(self) -> u32 {
        self.pages
    }

    /// Returns populated entries.
    #[must_use]
    pub const fn ready_entries(self) -> u32 {
        self.ready_entries
    }

    /// Returns reservations awaiting publication.
    #[must_use]
    pub const fn reserved_entries(self) -> u32 {
        self.reserved_entries
    }

    /// Returns invalidated allocations waiting for lease release.
    #[must_use]
    pub const fn retired_entries(self) -> u32 {
        self.retired_entries
    }

    /// Returns ready or retired entries protected by leases.
    #[must_use]
    pub const fn leased_entries(self) -> u32 {
        self.leased_entries
    }

    /// Returns allocated texels including padding.
    #[must_use]
    pub const fn allocated_texels(self) -> u64 {
        self.allocated_texels
    }

    /// Returns free texels across created pages.
    #[must_use]
    pub const fn free_texels(self) -> u64 {
        self.free_texels
    }

    /// Returns free rectangles across created pages.
    #[must_use]
    pub const fn free_regions(self) -> u32 {
        self.free_regions
    }

    /// Returns cumulative lifecycle counters.
    #[must_use]
    pub const fn metrics(self) -> CacheMetrics {
        self.metrics
    }
}

// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

use crate::Epoch;

/// Summary returned by [`ResidencyTracker::end_epoch`](crate::ResidencyTracker::end_epoch).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EpochSummary {
    epoch: Epoch,
    requests: usize,
    admissions: usize,
    evictions: usize,
    invalidations: usize,
    used_cost: u32,
}

impl EpochSummary {
    /// Creates a new epoch summary.
    #[must_use]
    pub const fn new(
        epoch: Epoch,
        requests: usize,
        admissions: usize,
        evictions: usize,
        invalidations: usize,
        used_cost: u32,
    ) -> Self {
        Self {
            epoch,
            requests,
            admissions,
            evictions,
            invalidations,
            used_cost,
        }
    }

    /// Returns the completed epoch.
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        self.epoch
    }

    /// Returns the number of queued requests observed during the epoch.
    #[must_use]
    pub const fn requests(self) -> usize {
        self.requests
    }

    /// Returns the number of admissions performed during the epoch.
    #[must_use]
    pub const fn admissions(self) -> usize {
        self.admissions
    }

    /// Returns the number of evictions performed during the epoch.
    #[must_use]
    pub const fn evictions(self) -> usize {
        self.evictions
    }

    /// Returns the number of invalidations performed during the epoch.
    #[must_use]
    pub const fn invalidations(self) -> usize {
        self.invalidations
    }

    /// Returns the total admitted cost after the epoch completed.
    #[must_use]
    pub const fn used_cost(self) -> u32 {
        self.used_cost
    }
}

impl fmt::Display for EpochSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "epoch {}: {} requests, {} admissions, {} evictions, {} invalidations, used cost {}",
            self.epoch.get(),
            self.requests,
            self.admissions,
            self.evictions,
            self.invalidations,
            self.used_cost
        )
    }
}

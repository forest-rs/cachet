// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Capacity settings passed to [`ResidencyTracker::new`](crate::ResidencyTracker::new).
///
/// `capacity` is the hard limit for admitted cost. `soft_limit` is the
/// preferred working-set ceiling that callers can use as a backpressure or
/// pre-eviction signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    capacity: u32,
    soft_limit: u32,
}

impl Budget {
    /// Creates a budget with an explicit hard capacity and soft admission limit.
    #[must_use]
    pub const fn new(capacity: u32, soft_limit: u32) -> Self {
        assert!(
            soft_limit <= capacity,
            "soft_limit must not exceed capacity"
        );
        Self {
            capacity,
            soft_limit,
        }
    }

    /// Creates a budget if `soft_limit <= capacity`.
    #[must_use]
    pub const fn try_new(capacity: u32, soft_limit: u32) -> Option<Self> {
        if soft_limit <= capacity {
            Some(Self {
                capacity,
                soft_limit,
            })
        } else {
            None
        }
    }

    /// Returns the maximum representable capacity.
    #[must_use]
    pub const fn capacity(self) -> u32 {
        self.capacity
    }

    /// Returns the preferred admission limit before eviction pressure begins.
    #[must_use]
    pub const fn soft_limit(self) -> u32 {
        self.soft_limit
    }

    /// Returns whether the budget is internally coherent.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.soft_limit <= self.capacity
    }
}

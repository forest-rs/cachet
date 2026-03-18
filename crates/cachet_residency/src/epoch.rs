// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Monotonic recency counter passed to
/// [`ResidencyTracker::begin_epoch`](crate::ResidencyTracker::begin_epoch).
///
/// Epochs are for usage time, not content freshness. Callers typically advance
/// the epoch once per frame, pass, or planning cycle so the tracker can record
/// when a resident was last touched.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Epoch(u64);

impl Epoch {
    /// Creates a new epoch value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw epoch number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

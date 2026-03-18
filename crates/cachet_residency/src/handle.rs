// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Opaque identifier returned by [`ResidencyTracker::admit`](crate::ResidencyTracker::admit).
///
/// A residency handle names one admitted logical resident in the policy layer.
/// It is not a physical placement token. Workloads that also use
/// `cachet_storage` typically map a `ResidencyHandle` to a
/// `cachet_storage::AllocationId` or other caller-owned metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResidencyHandle(u64);

impl ResidencyHandle {
    /// Creates a handle from a raw integer.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw integer backing the handle.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

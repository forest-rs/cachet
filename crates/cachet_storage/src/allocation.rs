// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// Opaque identifier returned by storage allocators and later passed to free
/// operations.
///
/// An allocation id names one physical placement owned by the storage layer.
/// It is distinct from `cachet_residency::ResidencyHandle`, which names a
/// logical resident in the policy layer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AllocationId(u64);

impl AllocationId {
    /// Creates an allocation id from a raw integer.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw integer backing the allocation id.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Error returned by storage allocation and free operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationError {
    /// The bounded allocator has no compatible free region left.
    Exhausted,
    /// The request is larger than the allocator can ever represent.
    TooLarge,
    /// An existing allocation was freed with mismatched or stale metadata.
    InvalidFree,
    /// The requested allocation id does not refer to a live allocation.
    UnknownAllocation,
    /// The allocator cannot mint another unique allocation id.
    IdExhausted,
}

impl fmt::Display for AllocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted => f.write_str("allocator is exhausted"),
            Self::TooLarge => f.write_str("allocation request is larger than allocator capacity"),
            Self::InvalidFree => f.write_str("allocation free request does not match a live slot"),
            Self::UnknownAllocation => {
                f.write_str("allocation id does not refer to a live allocation")
            }
            Self::IdExhausted => f.write_str("allocator cannot mint another unique allocation id"),
        }
    }
}

impl core::error::Error for AllocationError {}

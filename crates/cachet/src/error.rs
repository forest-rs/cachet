// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

use crate::{EntryId, Extent};

/// Failure to reserve physical atlas placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReserveError {
    /// The requested content has a zero dimension.
    EmptyArtifact,
    /// The padded artifact cannot fit on one configured page.
    ArtifactTooLarge {
        /// Requested content extent before padding.
        requested: Extent,
        /// Configured page extent.
        page: Extent,
    },
    /// The same key is already reserved with a different content extent.
    ConflictingExtent {
        /// Extent retained by the existing entry.
        existing: Extent,
        /// Extent supplied by the new request.
        requested: Extent,
    },
    /// All pages are full and no unleased ready entry can satisfy the request.
    NoSpace,
    /// The generational entry identifier space is exhausted.
    EntryIdExhausted,
    /// The stable page identifier space is exhausted.
    PageIdExhausted,
}

impl fmt::Display for ReserveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArtifact => formatter.write_str("atlas artifacts must be nonempty"),
            Self::ArtifactTooLarge { .. } => {
                formatter.write_str("the padded artifact does not fit on an atlas page")
            }
            Self::ConflictingExtent { .. } => {
                formatter.write_str("one atlas key cannot use conflicting raster extents")
            }
            Self::NoSpace => formatter.write_str(
                "atlas pages are full and no unleased entry can satisfy the reservation",
            ),
            Self::EntryIdExhausted => formatter.write_str("atlas entry identifiers are exhausted"),
            Self::PageIdExhausted => formatter.write_str("atlas page identifiers are exhausted"),
        }
    }
}

impl core::error::Error for ReserveError {}

/// Failure to publish a vacant reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishError {
    /// The entry identifier is stale or unknown.
    InvalidEntry(EntryId),
    /// The entry is not a vacant reservation.
    NotReserved(EntryId),
}

impl fmt::Display for PublishError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEntry(_) => formatter.write_str("the atlas entry is stale or unknown"),
            Self::NotReserved(_) => formatter.write_str("the atlas entry is not vacant"),
        }
    }
}

impl core::error::Error for PublishError {}

/// Failure to abort a reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbortError {
    /// The entry identifier is stale or unknown.
    InvalidEntry(EntryId),
    /// The entry has already been populated or retired.
    NotReserved(EntryId),
}

impl fmt::Display for AbortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEntry(_) => formatter.write_str("the atlas entry is stale or unknown"),
            Self::NotReserved(_) => formatter.write_str("the atlas entry is not vacant"),
        }
    }
}

impl core::error::Error for AbortError {}

/// Failure to lease a set of ready entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeaseError {
    /// An entry identifier is stale or unknown.
    InvalidEntry(EntryId),
    /// An entry exists but is not populated and ready.
    NotReady(EntryId),
}

impl fmt::Display for LeaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEntry(_) => formatter.write_str("a leased entry is stale or unknown"),
            Self::NotReady(_) => formatter.write_str("a leased entry is not ready"),
        }
    }
}

impl core::error::Error for LeaseError {}

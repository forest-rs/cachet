// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::Priority;

/// Logical residency request passed to
/// [`ResidencyTracker::request`](crate::ResidencyTracker::request) and
/// [`ResidencyTracker::admit`](crate::ResidencyTracker::admit).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request<K> {
    key: K,
    priority: Priority,
    generation: u64,
}

impl<K> Request<K> {
    /// Creates a request for the residency tracker.
    ///
    /// The request does not carry physical size or allocation details. The
    /// caller supplies those separately as a cost when admitting the request.
    #[must_use]
    pub const fn new(key: K, priority: Priority, generation: u64) -> Self {
        Self {
            key,
            priority,
            generation,
        }
    }

    /// Returns the caller-owned logical key for this request.
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the priority that the residency tracker should record.
    #[must_use]
    pub const fn priority(&self) -> Priority {
        self.priority
    }

    /// Returns the caller-controlled freshness or invalidation version.
    ///
    /// Generations are for content freshness, not recency. Re-requesting the
    /// same logical key with a newer generation tells the residency tracker to
    /// replace a stale resident with the newer version.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Consumes the request and returns its logical key.
    #[must_use]
    pub fn into_key(self) -> K {
        self.key
    }
}

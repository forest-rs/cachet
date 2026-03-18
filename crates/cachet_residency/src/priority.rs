// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Request priority used by the residency kernel.
///
/// Higher `tier` values outrank lower `tier` values. Within the same tier,
/// higher `score` values outrank lower `score` values.
///
/// `tier` is the coarse scheduling bucket. Use it when some work should always
/// outrank other work, for example visible content over speculative prefetch.
/// `score` is the fine-grained ordering within that bucket.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Priority {
    tier: u8,
    score: u32,
}

impl Priority {
    /// Creates a priority from a coarse tier and an intra-tier score.
    #[must_use]
    pub const fn new(tier: u8, score: u32) -> Self {
        Self { tier, score }
    }

    /// Returns the coarse scheduling tier.
    #[must_use]
    pub const fn tier(self) -> u8 {
        self.tier
    }

    /// Returns the fine-grained score within one tier.
    #[must_use]
    pub const fn score(self) -> u32 {
        self.score
    }
}

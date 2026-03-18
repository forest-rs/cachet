// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use cachet_storage::TileSlot;

use crate::TileKey;

/// Fallback record optionally attached to [`ResolvedTile`] by
/// [`ResolvedTile::with_fallback`].
///
/// A fallback is a substitute tile that can be shown temporarily when the
/// desired tile is not resident yet, usually because a coarser parent tile is
/// already available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FallbackRef {
    key: TileKey,
    level_delta: u8,
}

impl FallbackRef {
    /// Creates fallback metadata for a resolved tile.
    #[must_use]
    pub const fn new(key: TileKey, level_delta: u8) -> Self {
        Self { key, level_delta }
    }

    /// Returns the fallback tile key.
    #[must_use]
    pub const fn key(self) -> TileKey {
        self.key
    }

    /// Returns how many levels coarser the fallback is than the desired tile.
    #[must_use]
    pub const fn level_delta(self) -> u8 {
        self.level_delta
    }
}

/// Surface resolve record returned to a tiled-surface workload.
///
/// This is surface-facing resolve metadata: the caller-facing answer produced
/// after planning has identified a logical tile and storage has assigned a
/// physical slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedTile {
    key: TileKey,
    slot: TileSlot,
    fallback: Option<FallbackRef>,
}

impl ResolvedTile {
    /// Creates a resolved tile record from a logical key and physical slot.
    #[must_use]
    pub const fn new(key: TileKey, slot: TileSlot) -> Self {
        Self {
            key,
            slot,
            fallback: None,
        }
    }

    /// Adds fallback metadata to this resolved tile record.
    #[must_use]
    pub const fn with_fallback(mut self, fallback: FallbackRef) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Returns the logical tile key that this record resolves.
    #[must_use]
    pub const fn key(self) -> TileKey {
        self.key
    }

    /// Returns the physical slot assigned by `cachet_storage`.
    #[must_use]
    pub const fn slot(self) -> TileSlot {
        self.slot
    }

    /// Returns any fallback metadata attached to this resolved tile.
    #[must_use]
    pub const fn fallback(self) -> Option<FallbackRef> {
        self.fallback
    }
}

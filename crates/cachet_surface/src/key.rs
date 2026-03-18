// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Surface identifier passed to [`TilePlanner::plan`](crate::TilePlanner::plan)
/// and embedded in [`TileKey`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceId(u64);

impl SurfaceId {
    /// Creates a surface identifier from a caller-owned integer.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw identifier backing this surface id.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Logical tile key emitted by planning and consumed by resolve metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TileKey {
    surface: SurfaceId,
    level: u8,
    x: i32,
    y: i32,
}

impl TileKey {
    /// Creates a tile key.
    #[must_use]
    pub const fn new(surface: SurfaceId, level: u8, x: i32, y: i32) -> Self {
        Self {
            surface,
            level,
            x,
            y,
        }
    }

    /// Returns the surface identifier.
    #[must_use]
    pub const fn surface(self) -> SurfaceId {
        self.surface
    }

    /// Returns the mip or zoom level.
    #[must_use]
    pub const fn level(self) -> u8 {
        self.level
    }

    /// Returns the tile x coordinate.
    #[must_use]
    pub const fn x(self) -> i32 {
        self.x
    }

    /// Returns the tile y coordinate.
    #[must_use]
    pub const fn y(self) -> i32 {
        self.y
    }
}

// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use cachet_residency::{Priority, Request};

use crate::{SurfaceId, TileKey};

/// Configuration passed to [`TilePlanner::new`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TilePlannerConfig {
    tile_size: f32,
    prefetch_ring: i32,
    level: u8,
    priority_tier: u8,
    generation: u64,
}

impl TilePlannerConfig {
    /// Creates configuration for [`TilePlanner`].
    #[must_use]
    pub const fn new(tile_size: f32, prefetch_ring: i32, level: u8) -> Self {
        Self {
            tile_size,
            prefetch_ring,
            level,
            priority_tier: 0,
            generation: 0,
        }
    }

    /// Returns the logical tile size in view-space units.
    #[must_use]
    pub const fn tile_size(self) -> f32 {
        self.tile_size
    }

    /// Returns the number of tiles to prefetch beyond the visible rectangle.
    #[must_use]
    pub const fn prefetch_ring(self) -> i32 {
        self.prefetch_ring
    }

    /// Returns the planned level for generated requests.
    #[must_use]
    pub const fn level(self) -> u8 {
        self.level
    }

    /// Returns the priority tier assigned to planned requests.
    #[must_use]
    pub const fn priority_tier(self) -> u8 {
        self.priority_tier
    }

    /// Returns the generation assigned to planned requests.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns a copy of this configuration with a different request tier.
    #[must_use]
    pub const fn with_priority_tier(self, priority_tier: u8) -> Self {
        Self {
            priority_tier,
            ..self
        }
    }

    /// Returns a copy of this configuration with a different request generation.
    #[must_use]
    pub const fn with_generation(self, generation: u64) -> Self {
        Self { generation, ..self }
    }
}

/// View sample passed to [`TilePlanner::plan`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TilePlannerView {
    center_x: f32,
    center_y: f32,
    width: f32,
    height: f32,
    scale: f32,
}

impl TilePlannerView {
    /// Creates a planner view sample.
    #[must_use]
    pub const fn new(center_x: f32, center_y: f32, width: f32, height: f32, scale: f32) -> Self {
        Self {
            center_x,
            center_y,
            width,
            height,
            scale,
        }
    }

    /// Returns the view center x coordinate.
    #[must_use]
    pub const fn center_x(self) -> f32 {
        self.center_x
    }

    /// Returns the view center y coordinate.
    #[must_use]
    pub const fn center_y(self) -> f32 {
        self.center_y
    }

    /// Returns the view width in logical units.
    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }

    /// Returns the view height in logical units.
    #[must_use]
    pub const fn height(self) -> f32 {
        self.height
    }

    /// Returns the zoom scale for the view.
    #[must_use]
    pub const fn scale(self) -> f32 {
        self.scale
    }
}

/// One tile-planning result returned by [`TilePlanner::plan`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TilePlanEntry {
    request: Request<TileKey>,
    distance: u32,
}

impl TilePlanEntry {
    /// Creates one entry in a tile plan.
    #[must_use]
    pub const fn new(request: Request<TileKey>, distance: u32) -> Self {
        Self { request, distance }
    }

    /// Returns the logical residency request.
    #[must_use]
    pub const fn request(&self) -> &Request<TileKey> {
        &self.request
    }

    /// Returns the planner-computed distance from the view center.
    #[must_use]
    pub const fn distance(&self) -> u32 {
        self.distance
    }
}

/// Planner for tiled-surface working sets.
///
/// # Examples
///
/// This example asks the planner for the tile working set around one centered
/// view and then inspects the first plan entry.
///
/// ```
/// use cachet_surface::{SurfaceId, TilePlanEntry, TilePlanner, TilePlannerConfig, TilePlannerView};
///
/// let config = TilePlannerConfig::new(256.0, 0, 0)
///     .with_priority_tier(3)
///     .with_generation(9);
/// let planner = TilePlanner::new(config);
/// let planned = planner.plan(
///     SurfaceId::new(7),
///     &TilePlannerView::new(128.0, 128.0, 256.0, 256.0, 1.0),
/// );
///
/// let first = planned.first().expect("a center tile");
/// let _: &TilePlanEntry = first;
/// assert_eq!(first.request().key().surface(), SurfaceId::new(7));
/// assert_eq!(first.distance(), 0);
/// assert_eq!(first.request().priority().tier(), 3);
/// assert_eq!(first.request().generation(), 9);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TilePlanner {
    config: TilePlannerConfig,
}

impl TilePlanner {
    /// Creates a planner from static planner configuration.
    #[must_use]
    pub const fn new(config: TilePlannerConfig) -> Self {
        Self { config }
    }

    /// Returns the planner configuration.
    #[must_use]
    pub const fn config(self) -> TilePlannerConfig {
        self.config
    }

    /// Plans an ordered set of tile residency requests for one planner view.
    #[must_use]
    pub fn plan(&self, surface: SurfaceId, view: &TilePlannerView) -> Vec<TilePlanEntry> {
        // TODO(cachet): Add richer surface-planning hooks once a concrete
        // consumer proves the need for motion prediction, fallback planning, or
        // more nuanced prefetch heuristics than center-distance ordering.
        let scale = if view.scale() <= 0.0 {
            1.0
        } else {
            view.scale()
        };
        let tile_span = self.config.tile_size() / scale;
        let half_width = view.width() * 0.5;
        let half_height = view.height() * 0.5;

        let min_x =
            floor_to_tile(view.center_x() - half_width, tile_span) - self.config.prefetch_ring();
        let max_x =
            floor_to_tile(view.center_x() + half_width, tile_span) + self.config.prefetch_ring();
        let min_y =
            floor_to_tile(view.center_y() - half_height, tile_span) - self.config.prefetch_ring();
        let max_y =
            floor_to_tile(view.center_y() + half_height, tile_span) + self.config.prefetch_ring();
        let center_tile_x = floor_to_tile(view.center_x(), tile_span);
        let center_tile_y = floor_to_tile(view.center_y(), tile_span);

        let mut planned = Vec::new();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let distance = center_tile_x.abs_diff(x) + center_tile_y.abs_diff(y);
                let request = Request::new(
                    TileKey::new(surface, self.config.level(), x, y),
                    Priority::new(self.config.priority_tier(), u32::MAX - distance),
                    self.config.generation(),
                );
                planned.push(TilePlanEntry::new(request, distance));
            }
        }

        planned.sort_by_key(|tile| tile.distance());
        planned
    }
}

fn floor_to_tile(coord: f32, tile_span: f32) -> i32 {
    let scaled = coord / tile_span;
    let truncated = scaled as i32;
    if scaled < 0.0 && scaled != truncated as f32 {
        truncated.saturating_sub(1)
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_orders_center_tile_first() {
        let planner = TilePlanner::new(TilePlannerConfig::new(256.0, 0, 0));
        let tiles = planner.plan(
            SurfaceId::new(7),
            &TilePlannerView::new(128.0, 128.0, 256.0, 256.0, 1.0),
        );
        assert_eq!(
            tiles
                .first()
                .expect("at least one tile")
                .request()
                .key()
                .x(),
            0
        );
        assert_eq!(
            tiles
                .first()
                .expect("at least one tile")
                .request()
                .key()
                .y(),
            0
        );
        assert_eq!(tiles.first().expect("at least one tile").distance(), 0);
    }

    #[test]
    fn planner_uses_configured_generation_and_tier() {
        let config = TilePlannerConfig::new(256.0, 0, 2)
            .with_priority_tier(4)
            .with_generation(11);
        let planner = TilePlanner::new(config);
        let tiles = planner.plan(
            SurfaceId::new(5),
            &TilePlannerView::new(128.0, 128.0, 256.0, 256.0, 1.0),
        );
        let first = tiles.first().expect("at least one tile");

        assert_eq!(first.request().key().surface(), SurfaceId::new(5));
        assert_eq!(first.request().key().level(), 2);
        assert_eq!(first.request().priority().tier(), 4);
        assert_eq!(first.request().generation(), 11);
    }

    #[test]
    fn floor_to_tile_handles_negative_integer_boundaries() {
        assert_eq!(floor_to_tile(-512.0, 256.0), -2);
        assert_eq!(floor_to_tile(-511.9, 256.0), -2);
        assert_eq!(floor_to_tile(-256.0, 256.0), -1);
    }
}

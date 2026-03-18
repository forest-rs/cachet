# cachet_surface

Surface-oriented planning and resolve vocabulary for Cachet.

## Role

`cachet_surface` makes tiled logical spaces explicit on top of the generic
residency kernel.

It owns:

- tile identity
- planner configuration and planner inputs
- ordered tile-planning output
- surface-facing resolved metadata
- fallback metadata

It does not own map rendering, document rasterization, page tables, or
renderer-global orchestration.

## Main Types

- `TilePlannerConfig`: passed to `TilePlanner::new`
- `TilePlannerView`: passed to `TilePlanner::plan`
- `TilePlanEntry`: returned by `TilePlanner::plan`
- `ResolvedTile`: metadata handed back once storage assigned a slot
- `FallbackRef`: optional fallback metadata attached to a resolved tile

## Good Fits

- map tiles
- zoomable documents
- large canvas or whiteboard caches

## Notes

- The planner vocabulary is intentionally explicit so surface-specific pressure
  stays visible in the design.

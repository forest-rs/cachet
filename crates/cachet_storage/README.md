# cachet_storage

Physical slot allocators for Cachet.

## Role

`cachet_storage` is the optional placement layer between generic residency
policy and workload-facing resolve metadata.

It owns:

- rect allocation for atlas-like workloads
- fixed-slot allocation for equal-sized tile or page workloads
- allocation ids and allocator stats

It does not own residency policy, view planning, uploads, or binding.

## Main Types

- `RectAtlas`: bounded single-page rect allocator
- `TilePool`: fixed-size equal-slot allocator
- `AtlasSlot` / `TileSlot`: allocation results
- `AllocationId`: opaque id used by free operations

## Notes

- `RectAtlas` is still single-page in this first slice.
- `RectAtlas` reuses freed regions but does not coalesce them; watch
  `RectAtlasStats::free_regions()` for fragmentation pressure.
- A dense resident-slot registry may not need this crate at all.

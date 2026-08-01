# ADR-0001: Cachet is a homogeneous raster-artifact atlas

- Status: Accepted
- Date: 2026-08-02
- Decision issue: `cac-2qv.1`
- Epic: `cac-2qv`

## Context

Cachet needs one real ownership boundary after an unpublished experiment tried
to serve atlas artifacts, tiled surfaces, and general GPU resources through a
shared residency vocabulary. Those systems have materially different demand,
fallback, and lifetime models.

Tavolo and Underwood provide a concrete first consumer: repeated glyph rasters
need bounded packing, partial uploads, stable placements, and protection from
pixel reuse while prepared work still samples them. Sprite, icon, and small
image-patch consumers have the same storage lifecycle when their artifacts are
small immutable raster rectangles.

## Fence

`cachet` owns bounded page storage, rectangle packing, keyed artifact validity,
lease-safe pixel reuse, CPU page bytes, dirty upload regions, and atlas
diagnostics; it explicitly does not own rasterization, font or animation
semantics, pixel interpretation, GPU resources, upload execution, draw
batching, submissions, tiled surfaces, or general resource lifetime.

## Invariants

1. A caller key identifies every fact affecting raster pixels. Cachet never
   decides whether two glyphs, sprites, or image patches are equivalent.
2. One `AtlasCache` instance is one complete compatibility domain: page extent,
   bytes per texel, padding, pixel interpretation, sampling contract, and
   retention policy agree for every entry.
3. Incompatible domains use separate cache instances. R8 coverage and RGBA8
   color glyphs are the first proof; equal byte widths alone never establish
   compatibility.
4. A reservation is not valid for lookup until raster bytes and caller-owned
   artifact metadata have been populated together.
5. Entry identifiers are generational. Eviction or invalidation makes stale
   identifiers fail even after their table slot is reused.
6. Pixels protected by a live lease are never overwritten. Invalidation removes
   the logical key immediately but defers physical reuse until leases drop.
7. Capacity is physical: fixed page dimensions and a maximum page count replace
   abstract costs, priorities, and generic budgets.
8. Cachet records dirty page regions and exposes page bytes. Consumers own GPU
   page creation, transfers, synchronization, and acknowledgement.
9. Current gauges and cumulative counters expose memory, packing,
   fragmentation, lookup work, evictions, deferred reuse, and upload work.

## Options considered

### Generic residency kernel plus workload adapters

Rejected. It shares vocabulary while pushing ownership behavior into callbacks
and bindings. Tiled surfaces and GPU resource domains should develop locally.

### One multi-format atlas manager

Rejected for the first slice. A format registry and routing policy would make
the core responsible for domain composition before a consumer proves that the
same batching, padding, sampling, and capacity policy should be shared.

### One homogeneous cache composed by the caller

Chosen. The core owns one complete atlas lifecycle. A glyph integration can
hold separate coverage and color caches; a sprite system can hold another
cache with sprite-native keys and metadata. A small higher-level set may be
added only after two integrations prove identical composition semantics.

## First slice

The public seam consists of:

- `AtlasConfig` describing fixed page extent, maximum pages, bytes per texel,
  and padding;
- caller-defined keys and caller metadata stored by `AtlasCache<K, M>`;
- direct reserve, populate, lookup, abort, and invalidate operations;
- generational entry and stable page identifiers;
- leases over ready entries;
- checkpointed dirty-region collection, page byte access, and explicit upload
  acknowledgement;
- deterministic LRU eviction among ready unleased entries;
- cache-wide metrics and per-page packing/upload gauges.

The rectangle allocator is private and replaceable. The first implementation
uses deterministic guillotine splits and adjacent-region coalescing.

## Tradeoffs and extension points

- Separate format domains mean callers dispatch between coverage, color, or
  sprite caches. This keeps compatibility visible and prevents an arbitrary
  class router from becoming policy infrastructure.
- A small per-entry lease anchor costs one allocation but makes release follow
  ordinary Rust ownership without importing GPU submission concepts.
- CPU page mirrors consume predictable memory and enable backend-independent
  raster population and upload recovery.
- LRU is fixed initially. A second measured workload must justify another
  eviction policy before the API grows a policy trait.
- Palette, foreground color, variation coordinates, synthesis, hinting, and
  other raster-affecting facts belong in color-glyph keys. Paint applied after
  sampling does not belong in R8 coverage keys.
- Sprite animation stays outside Cachet: frames may be separate immutable keys
  or regions within a caller-produced sheet.

## Consequences

Cachet is useful to glyph and sprite consumers without claiming ownership of
their semantics. Deliberate composition above this crate is preferred over
speculative common control below it.

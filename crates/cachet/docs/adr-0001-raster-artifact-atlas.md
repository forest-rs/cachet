# ADR-0001: Separate atlas placement from pixel backing

- Status: Accepted
- Date: 2026-08-02
- Decision issue: `cac-2qv.1`
- Epic: `cac-2qv`

## Context

Cachet needs one real ownership boundary after an unpublished experiment tried
to serve atlas artifacts, tiled surfaces, and general GPU resources through a
shared residency vocabulary. Those systems have materially different demand,
fallback, and lifetime models.

Tavolo and Underwood provide the first concrete consumers. Repeated glyph
rasters need bounded rectangle packing, stable placements, and protection from
reuse while prepared work still samples them. Some artifacts arrive as CPU
rasters that require partial uploads; others may be rendered directly into a
GPU atlas page. Pixel backing and production therefore cannot be an invariant
of the placement cache itself.

## Fence

`AtlasCache` owns bounded rectangle placement, keyed artifact validity,
publication state, lease-safe reuse, and lifecycle and packing diagnostics; it
explicitly does not own pixel storage, rasterization, pixel interpretation,
GPU resources, command submission, completion fences, tiled surfaces, or
general resource lifetime.

`CpuAtlasCache` composes `AtlasCache` with homogeneous CPU page mirrors,
validated raster copying, dirty upload regions, and byte and upload
diagnostics; it explicitly does not execute uploads or own GPU synchronization.

## Invariants

1. A caller key identifies every fact affecting produced pixels. Cachet never
   decides whether two glyphs, sprites, or image patches are equivalent.
2. One cache instance is one complete compatibility domain: page extent,
   padding, backing authority, production ordering, pixel interpretation,
   sampling contract, and retention policy agree for every entry.
3. Incompatible domains use separate cache instances. R8 coverage, RGBA8
   color glyphs, sprites, CPU-mirrored pages, and directly rendered GPU pages
   are separate whenever any of those contracts differ. Equal byte widths do
   not establish compatibility.
4. A vacant reservation owns physical placement and is not evictable, but is
   not visible to lookup.
5. Publication stores caller metadata and makes an entry visible. It
   atomically returns an initial lease, closing the interval in which newly
   published pixels could otherwise be reused before producer or consumer work
   retains them.
6. Publication means the caller has made content available under its execution
   ordering contract. It does not mean a GPU submission has completed.
7. Entry identifiers are generational. Eviction or invalidation makes stale
   identifiers fail even after their table slot is reused.
8. A placement protected by a live lease is never reused. Invalidation removes
   the logical key immediately but defers physical reuse until leases drop.
9. Cachet owns no GPU fence. Tavolo or another caller retains publication and
   reader leases until every relevant submission can no longer write or sample
   their placements.
10. CPU population publishes only after a complete validated copy. CPU pages
    record dirty regions and expose bytes; consumers own page creation,
    transfers, synchronization, and acknowledgement.
11. Current gauges and cumulative counters expose placement occupancy,
    fragmentation, probes, production, eviction, deferred reuse, and—where
    applicable—CPU memory and upload work.

## Options considered

### One cache with CPU/GPU modes

Rejected. Optional page bytes and mode-dependent methods would make invalid
states part of the central API. Mixing a CPU mirror with direct GPU writes can
also make the mirror stale and allow a later upload to overwrite GPU-produced
content.

### A generic backing trait

Rejected for the first slice. Production, visibility, and synchronization
policy would move into callbacks while making the public cache harder to
explain. Two concrete layers express the real ownership boundary directly.

### A placement core plus a concrete CPU facade

Chosen. `AtlasCache<K, M>` provides the storage-independent atlas lifecycle.
`CpuAtlasCache<K, M>` delegates that lifecycle while adding CPU storage and
dirty uploads. A direct GPU consumer uses the placement core and owns the
mapping from stable page identifiers to GPU textures.

This shares atlas behavior below the pixel-backing boundary without becoming
a generic residency kernel.

## First slices

The placement seam consists of:

- `AtlasConfig` describing fixed page extent, maximum pages, and padding;
- caller-defined keys and metadata stored by `AtlasCache<K, M>`;
- direct reserve, publish, lookup, abort, invalidate, and reclaim operations;
- generational entry and stable append-only page identifiers;
- publication and reader leases over ready entries;
- deterministic LRU eviction among ready unleased entries; and
- cache-wide lifecycle metrics and per-page packing gauges.

The CPU seam adds:

- a homogeneous bytes-per-texel configuration;
- lazily allocated zeroed page mirrors;
- checked raster extent, row-stride, byte-length, and padded copying;
- checkpointed dirty-region collection and explicit acknowledgement;
- whole-page and region byte views; and
- CPU memory, copied-byte, and pending-upload diagnostics.

The rectangle allocator is private and replaceable. The first implementation
uses deterministic guillotine splits and adjacent-region coalescing.

## Direct GPU production protocol

1. Reserve a vacant placement. Its reservation state prevents reuse.
2. Ensure the caller-owned GPU page for the stable page identifier exists.
3. Encode the write into the reserved rectangle.
4. Publish the metadata and receive the initial lease atomically.
5. Retain that lease until producer ordering and every prepared reader permit
   reuse. Tavolo translates its submission completion into ordinary lease
   drops.

On an ordered queue, publication may occur after command encoding rather than
after physical GPU completion. Cross-queue visibility remains entirely the
caller's responsibility.

## Tradeoffs and extension points

- The CPU facade repeats a small amount of lifecycle-shaped API so callers do
  not need to manipulate its inner placement cache or bypass byte invariants.
- Separate backing domains require caller dispatch. This prevents CPU mirrors
  and direct GPU writers from silently corrupting each other's authority.
- A small per-entry lease anchor costs one allocation but follows ordinary Rust
  ownership without importing submission concepts.
- LRU is fixed initially. A second measured workload must justify another
  eviction policy before the API grows a policy trait.
- Palette, variation coordinates, synthesis, hinting, and other
  raster-affecting facts belong in color-glyph keys. Paint applied after
  sampling does not belong in R8 coverage keys.
- Sprite animation stays outside Cachet: frames may be separate immutable keys
  or regions within a caller-produced sheet.

## Consequences

CPU glyph and sprite consumers get an ergonomic raster cache. Direct GPU
producers get the same placement, validity, and reuse guarantees without a
redundant or stale CPU mirror. Cachet remains an atlas system rather than the
conceptual foundation for Tavolo's broader GPU resource domain.

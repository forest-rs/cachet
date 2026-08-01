# `cac-2qv`: Placement-first raster atlas

## Goal

Build a durable `no_std` atlas placement cache plus an ergonomic CPU raster
facade, proven by mixed coverage/color glyphs, caller-managed GPU production,
and a sprite-shaped consumer.

## Non-goals

- rasterization, image decoding, or asynchronous job scheduling;
- a font, text, sprite-animation, or image model;
- GPU objects, transfers, submissions, queue ordering, or completion fences;
- tiled surfaces or a general resource registry;
- multi-format or multi-backing routing inside one cache;
- compatibility with the unpublished predecessor implementation.

## Issue sequence

1. `cac-2qv.1`: record the placement/backing boundary and publication protocol.
2. `cac-2qv.2`: implement placement, publication, leases, packing, eviction,
   and core observability.
3. `cac-2qv.5`: compose CPU pages, raster copying, dirty uploads, and CPU
   observability over the placement core.
4. `cac-2qv.3`: prove R8/RGBA8 glyph caches, direct GPU production, and a
   sprite consumer with runnable top-level samples.
5. `cac-2qv.4`: run gates and publish source and Dolt histories.

The Beads dependency graph is the source of truth for ordering.

## Risks

- Publication can expose a placement before producer work is safe from reuse;
  publishing atomically returns the initial lease.
- A caller can violate GPU ordering after publication; Cachet documents the
  contract while Tavolo owns submission and completion tracking.
- A CPU mirror can become stale if direct GPU writes share its pages; backing
  strategy is part of the compatibility domain and samples keep them separate.
- Free-rectangle splitting can create overlaps or lose area; deterministic
  allocator tests cover split, free, coalesce, and full-page recovery.
- Invalidation can race retained placements; leases defer physical reuse.
- Upload acknowledgement can erase later writes; checkpoints acknowledge only
  dirty records that existed when collected.
- Bytes per texel can be mistaken for complete compatibility; examples keep
  coverage, color glyph, GPU-written, and sprite domains explicit.

## Validation

```text
typos
cargo fmt --all
taplo fmt
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --no-deps
cargo run -p glyph-atlas-demo
cargo run -p gpu-atlas-demo
cargo run -p sprite-atlas-demo
bd config validate
bd dep cycles
bd dolt push
```

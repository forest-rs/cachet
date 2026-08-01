# `cac-2qv`: Raster-artifact atlas

## Goal

Build one durable `no_std` cache for small immutable raster artifacts, proven
by mixed coverage/color glyphs and a sprite-shaped consumer.

## Non-goals

- rasterization or asynchronous production;
- a font, text, sprite-animation, or image-decoding model;
- GPU objects, transfers, submissions, or deferred GPU destruction;
- tiled surfaces or a generic resource registry;
- multi-format routing inside one cache;
- compatibility with the unpublished predecessor implementation.

## Issue sequence

1. `cac-2qv.1`: record the homogeneous raster-atlas boundary.
2. `cac-2qv.2`: implement cache lifecycle, packing, leases, uploads, and
   observability.
3. `cac-2qv.3`: prove separate R8/RGBA8 glyph caches and a sprite consumer.
4. `cac-2qv.4`: run gates and publish source and Dolt histories.

The Beads dependency graph is the source of truth for ordering.

## Risks

- Free-rectangle splitting can create overlaps or lose area; deterministic
  allocator tests cover split, free, coalesce, and full-page recovery.
- Invalidation can race retained placements; leases defer physical reuse.
- Upload acknowledgement can erase later writes; checkpoints acknowledge only
  dirty records that existed when collected.
- Bytes-per-texel can be mistaken for complete compatibility; examples keep
  coverage, color glyph, and sprite domains explicitly separate.

## Validation

```text
typos
cargo fmt --all
taplo fmt
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --no-deps
bd config validate
bd dolt push
```

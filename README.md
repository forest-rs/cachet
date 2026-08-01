# cachet

Cachet is a small `no_std` atlas for glyphs, sprites, icons, and other repeated
image patches. It separates placement and reuse from the storage holding the
pixels.

## Fence

The placement core owns rectangle packing, keyed validity, publication,
lease-safe reuse, and atlas diagnostics. A concrete CPU facade adds page bytes
and dirty uploads. Cachet does not own rasterization, font or animation
semantics, GPU objects, command submission, completion fences, or general
resource lifetime.

One cache instance represents one complete compatibility domain. An R8
coverage atlas and an RGBA8 color atlas therefore use separate caches, as do
CPU-mirrored and directly GPU-rendered pages or sprites whose sampling,
padding, color-space, or retention requirements differ from color glyphs.

The clean-root implementation is tracked under Beads epic `cac-2qv`. Its
accepted boundary is recorded in
[`crates/cachet/docs/adr-0001-raster-artifact-atlas.md`](crates/cachet/docs/adr-0001-raster-artifact-atlas.md).

## Runnable examples

```sh
cargo run -p glyph-atlas-demo
cargo run -p gpu-atlas-demo
cargo run -p sprite-atlas-demo
```

- `glyph-atlas-demo` composes separate CPU-backed R8 coverage and RGBA8 color
  caches and walks their dirty-upload lifecycle.
- `gpu-atlas-demo` simulates the boundary Tavolo would own: page textures,
  command ordering, submission serials, and fence-driven lease release.
- `sprite-atlas-demo` uses sprite-native keys and pivot metadata with a
  CPU-backed RGBA8 cache.

## Issue tracking

Issue history lives in a Dolt database synchronized through this repository's
remote. It is independent from ordinary Git source history.

```sh
# Fresh clone
chmod 700 .beads
bd bootstrap

# Work session
bd prime
bd dolt pull
bd ready

# Publish issue changes explicitly
bd dolt push
```

Automatic Dolt pushes are disabled intentionally.

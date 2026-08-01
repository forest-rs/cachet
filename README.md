# cachet

Cachet is a small `no_std` raster-artifact atlas for glyphs, sprites, icons,
and other repeated image patches.

## Fence

Cachet owns bounded page storage, rectangle packing, keyed validity,
lease-safe pixel reuse, CPU page bytes, dirty upload regions, and atlas
diagnostics. It does not own rasterization, font or animation semantics, GPU
objects, uploads, draw batching, submissions, or general resource lifetime.

One cache instance represents one complete page-compatibility domain. An R8
coverage atlas and an RGBA8 color atlas therefore use separate caches, as do
sprites whose sampling, padding, color-space, or retention requirements differ
from color glyphs.

The initial public API is being implemented under Beads epic `cac-2qv`. The
accepted boundary is recorded in
[`crates/cachet/docs/adr-0001-raster-artifact-atlas.md`](crates/cachet/docs/adr-0001-raster-artifact-atlas.md).

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

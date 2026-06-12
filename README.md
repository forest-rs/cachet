# cachet

Design area for a proposed residency-oriented crate family that can support
Vello-family and related renderers without binding the core design to any
one engine or GPU backend.

This workspace is documentation-led: the crates, examples, and tests exist to
prove the seams without pretending the design is fully settled.

Current documentation and implementation landmarks:

- `docs/adr-0001-four-crate-initial-sketch.md` records the chosen crate
  boundary.
- `crates/` contains compilable first-slice crate sketches.

Current workspace members:

- `cachet`
- `cachet_residency`
- `cachet_storage`
- `cachet_atlas`
- `cachet_surface`
- `glyph_cache_demo`
- `image_resource_demo`

The goal is still documentation-led: the crates exist to prove the seam and
first slice, not to freeze a broad public API too early. The sketch includes
both atlas and surface adapters so the competing workload pressures stay
visible early. Atlas is now the more-developed demo path, with
`cachet_atlas::AtlasCache` providing the intended composition over residency,
page routing, and storage. The residency core is also intended to help
registry-style resource tables and other non-atlas consumers.

Slice 3 has started to make the runtime story more explicit as well: the
residency and atlas layers now expose reusable batch-processing contexts for
steady-state work with caller-owned scratch and output storage.

The facade crate in `crates/cachet/` is the top-level rustdoc landing page for
future users. The example crates under `examples/` provide runnable stories for
glyph caching and image resource management, including a multi-page glyph cache
churn story. Cross-crate composition is also covered by an integration test in
`crates/cachet/tests/residency_flow.rs`.

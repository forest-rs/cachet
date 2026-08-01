# cachet

A bounded `no_std` artifact atlas with explicit pixel backing.

`AtlasCache` owns keyed reservations, packed placement, publication,
lease-safe reuse, and diagnostics without owning pixels. `CpuAtlasCache` adds
CPU page mirrors and dirty upload regions. Callers own artifact identity and
production, pixel semantics, GPU resources, uploads, and synchronization.

Use a separate cache for each complete page-compatibility domain. See
[`docs/adr-0001-raster-artifact-atlas.md`](docs/adr-0001-raster-artifact-atlas.md)
for the boundary and first-slice invariants.

# cachet

A bounded `no_std` raster-artifact atlas.

Cachet owns keyed reservations, packed page placement, entry validity,
lease-safe reuse, CPU page bytes, dirty upload regions, and diagnostics.
Callers own raster identity and production, pixel semantics, GPU resources,
uploads, and synchronization.

Use a separate cache for each complete page-compatibility domain. See
[`docs/adr-0001-raster-artifact-atlas.md`](docs/adr-0001-raster-artifact-atlas.md)
for the boundary and first-slice invariants.

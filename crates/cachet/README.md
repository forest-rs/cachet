# cachet

Facade crate and rustdoc landing page for the Cachet residency family.

## Role

This crate does not hide the underlying crate boundaries. It exists to give
future users one calm entry point that explains how the pieces fit together and
to re-export the four core crates:

- `cachet::residency`
- `cachet::storage`
- `cachet::atlas`
- `cachet::surface`

## Start Here

Choose the workload shape first:

- glyph, icon, or image-patch cache: start with `cachet::atlas`, especially
  `cachet::atlas::AtlasCache` once you want a calm queue/process/resolve path
- tiled map, document, or canvas cache: start with `cachet::surface`
- dense resident-slot registry: start with `cachet::residency`

## Notes

- This is still an exploratory sketch.
- The facade is a guide, not an abstraction barrier.
- Batch processing takes explicit reusable batch/output state for steady-state
  work.
- Runnable examples live in `examples/`, including a multi-page glyph cache
  churn story.
- Cross-crate composition is covered by `crates/cachet/tests/residency_flow.rs`.

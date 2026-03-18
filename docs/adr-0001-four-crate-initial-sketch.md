# ADR-0001: Cachet starts as a four-crate initial sketch

- Status: Proposed
- Date: 2026-03-18
- Deciders: Cachet maintainers

## Context

Cachet is intended to provide a reusable residency-oriented subsystem that can
serve more than one renderer or UI stack. The target workloads include:

- atlas-style caches for glyphs, icons, and small image artifacts
- tiled surface caches for zoomable document, CAD, browser, and scene tooling
- future page-based GPU resources where bounded residency matters

The core design pressure is architectural, not feature breadth:

- keep the center small and testable
- preserve `no_std` where it is honest
- separate policy from allocation and runtime integration
- avoid premature crate count and speculative public APIs

There is also a concrete class of non-atlas integration target: registry-style
resource tables want strategy-agnostic residency and stable lookup without
committing to bindless, array, or atlas binding policy. That makes them likely
consumers of Cachet's policy core even when the concrete storage model is not
an atlas.

The design pressure is to keep the major layers visible without overcommitting
to a broader family than the boundary needs on day one. At the same time,
keeping only one workload adapter in the initial sketch risks letting one use
case silently define the whole API.

## Decision

Cachet starts as a four-crate initial sketch.

Fence:
`cachet` owns bounded residency policy and optional physical slot assignment; it
explicitly does not own rasterization, async execution, renderer orchestration,
or backend handles.

### Crate graph

1. `cachet_residency`

This crate is the kernel. It should be `#![no_std]` with `alloc`.

It owns:

- requests
- priority ordering
- budget accounting
- epoch/frame accounting
- usage tracking
- invalidation generations
- eviction decisions
- summary/event types

It does not own:

- atlas geometry
- tile coordinates
- workload-specific fallback semantics
- producers or async jobs
- GPU upload state

2. `cachet_storage`

This crate is the optional physical-space allocator layer. It should also stay
`#![no_std]` with `alloc`.

It owns:

- rect atlas allocation
- fixed tile pools
- array/slab-like slot allocators
- fragmentation statistics

It does not own:

- residency priority
- request scheduling
- workload planning
- GPU textures or views

Not every consumer must use this crate. A registry-style resource table may use
`cachet_residency` with a dense resident-slot model first, and only pull in
`cachet_storage` when it wants atlas, tile, or array-like packing policy.

3. `cachet_atlas`

This adapter owns:

- workload key types for discrete artifacts
- atlas compatibility classes and artifact sizes
- atlas-facing request metadata
- mapping from opaque residency/allocation handles to atlas-facing resolve
  metadata such as page selection and UV rects

4. `cachet_surface`

This adapter owns:

- tiled logical-space key types
- view-driven planning from viewport state to residency requests
- surface-specific fallback policy such as parent-tile substitution
- mapping from opaque residency/allocation handles to surface-facing resolve
  metadata

The initial sketch includes both adapters so the boundary is tested against
competing workload needs from the start. Atlas remains the first more-developed
demo slice because it is the easiest adoption story for likely Linebender
consumers.

### Deliberate non-decisions

Cachet does not start with standalone `cachet_gpu` or `cachet_debug` crates.

- A backend adapter should be named by backend, for example `cachet_wgpu`, once
  there is a concrete need.
- Debug formatting and visualization should start as modules or examples until
  there is clear pressure from a second consumer.

### API shape constraints

The kernel should not resolve directly to atlas rects, tile UVs, or GPU-facing
handles. It should traffic in opaque allocation identifiers or residency
handles, leaving workload adapters to interpret those handles.

The core should also support non-packed consumers. A dense-slot registry should
be able to use `cachet_residency` without pretending that its resident slot is
an atlas rect or tile allocation.

The kernel should also avoid a producer trait in the public core API. Content
production belongs above the kernel so the center remains synchronous,
deterministic, and testable without imposing job or async semantics.

### Target Consumers

Cachet is intended to help at least three different consumer shapes:

1. Atlas-style workloads

- glyph, icon, and patch caches
- explicit rect packing and bounded hot sets

2. Surface-style workloads

- map tiles
- zoomable documents and canvases
- view-planned working sets with parent fallback semantics

3. Registry-style workloads

- registry-style resource tables
- dense resident-slot lookup keyed by stable handles
- upload and `wgpu` resource ownership staying outside Cachet

## Consequences

### Positive

- The core boundary stays calm and portable across multiple renderer and UI
  stacks.
- The center remains testable on CPU without GPU access.
- The sketch keeps atlas and surface pressures visible at the same time.
- Registry-style consumers can adopt the policy kernel without adopting atlas
  or surface semantics.
- Future backend crates depend inward only.

### Negative

- The four-crate sketch is slightly heavier than a single-adapter prototype.
- One adapter will still be more developed than the other at first.
- The docs need to be careful not to let the richer atlas demo shape read like
  the only supported integration model.

## Alternatives Considered

### Start with a broader multi-crate family immediately

Rejected for the first slice because it fixes too many boundaries before the
core handle model and workload seams have been exercised.

### Start with one monolithic `cachet` crate

Rejected because policy and allocation are different reasons to change, and the
repo’s engineering style favors small durable cores with explicit seams.

### Start with only one workload adapter

Rejected because the main architectural risk is letting one use case's nouns
silently become the whole system's nouns. Carrying both `cachet_atlas` and
`cachet_surface` in the sketch makes those conflicting pressures visible while
the APIs are still cheap to change.

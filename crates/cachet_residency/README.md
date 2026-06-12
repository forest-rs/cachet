# cachet_residency

Core residency policy primitives for Cachet.

## Role

`cachet_residency` is the boring center:

- requests
- priorities
- budget accounting
- epoch bookkeeping
- resident tracking
- eviction bookkeeping

It intentionally does not own atlas geometry, tiled-surface planning, GPU
resources, or background execution.

## Main Types

- `Budget`: passed to `ResidencyTracker::new`
- `Epoch`: passed to `ResidencyTracker::begin_epoch`
- `Request<K>`: passed to `request` and `admit`
- `ResidencyHandle`: returned by `admit`
- `ResidencyBindings<V>`: caller-owned glue from handles to resolved metadata
- `RequestProcessingBatch<K, S>`: passed to `process_requests` when the
  caller wants a reusable batch context
- `RequestProcessingOutput<K>`: reusable Vec-backed output sink for collected
  results
- `RequestProcessingSink<K>`: streamed output interface carried by a batch

## Good Fits

- atlas adapters
- tiled-surface adapters
- dense resident-slot registries

## Notes

- The core is `no_std` plus `alloc`.
- Batch admission currently uses priority-aware eviction with recency as a
  tiebreaker.
- `process_requests()` takes explicit caller-owned batch state; reuse the same
  batch across epochs for steady-state work.
- Handle lookup currently uses a sparse side table keyed by minted handle id,
  so memory growth follows lifetime admissions rather than current resident
  count.
- Higher layers still own physical allocation and backend resources.

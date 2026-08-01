# Migration note: reusable leases

The clean-root API has no known external users, but the allocation wind tunnel
added one public lease path after the initial local API checkpoint.

- Existing `AtlasCache::lease` and `CpuAtlasCache::lease` calls remain valid.
- One-entry leases no longer allocate storage for a vector.
- `Lease::empty`, `Lease::clear`, and `lease_into` permit callers to retain one
  batch buffer per in-flight frame and refill it after submission completion.
- `LeaseError::TargetNotEmpty` is new. Exhaustive matches on `LeaseError` must
  handle it.

`lease_into` never clears a non-empty target: doing so could release work the
caller still considers in flight. The caller explicitly clears the token only
after its producer and reader submissions have completed.

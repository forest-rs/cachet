# image_resource_demo

Runnable dense-slot image resource management story for Cachet.

This example is aimed at future users who do not want atlas or tiled-surface
semantics at all, but still want Cachet's budgeting, recency tracking, and
eviction policy.

What it demonstrates:

- a registry-style consumer built directly on `cachet::residency`
- `ResidencyTracker::process_requests` as the calmer batch admission path
- `ResidencyBindings` as the handle-to-slot glue
- a caller-owned dense resident-slot table
- reuse of dense slots after eviction
- no dependency on `cachet::storage`

Run it with:

```text
cargo run -p image_resource_demo
```

The output is structured as two frame-like phases so you can see how explicit
usage tracking changes which resident gets evicted.

# Cachet Wind Tunnel

Criterion measurement harness for Cachet's slice-3 batch-processing paths.

Run it with:

```sh
cargo bench -p cachet_wind_tunnel
```

It compares fresh per-batch output storage with reusable batch/output storage for:

- residency-only batch processing
- atlas queued processing
- atlas page-candidate selection

Criterion is scoped to this benchmark crate as a dev-dependency. The core
crates do not gain benchmark dependencies or dev-dependencies from this harness.

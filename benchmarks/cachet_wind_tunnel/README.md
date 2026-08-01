# Cachet wind tunnel

This workspace-only crate measures Cachet through public APIs. It keeps timing
and allocator instrumentation out of the `no_std` core.

Run the Criterion suite:

```sh
cargo bench -p cachet_wind_tunnel --bench atlas
```

The suite covers ready hits, single and batch lease formation, bounded
placement churn, CPU raster population, and dirty upload collection. Setup and
retained scratch capacity remain outside each timed iteration where that
matches a long-lived renderer.

Run the allocation phases with the same optional counting allocator used by
Underwood's wind tunnels:

```sh
cargo run --release -p cachet_wind_tunnel --features allocation-counting
```

The output distinguishes total allocation calls and bytes, peak live growth,
and net retained memory. Counts cover only the named operation after warm-up;
they are requested-allocation observations, not allocator metadata or process
resident memory. Criterion timings and allocation-counter results answer
different questions and should not be conflated.

The executable also enforces its allocation budgets. Ready hits, single leases,
reused batch leases, steady placement/CPU churn, and upload collection must
remain allocation-free. An owned 32-entry lease may allocate its one returned
buffer. See [`RESULTS.md`](RESULTS.md) for the current baseline and measurement
environment.

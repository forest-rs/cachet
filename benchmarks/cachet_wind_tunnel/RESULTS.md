# Wind-tunnel baseline: 2026-08-02

## Environment

- host: Apple arm64, macOS 26.5.2 (25F84);
- allocator: system allocator observed through `allocation-counter` 0.8.1;
- compiler: rustc 1.97.1 (`aarch64-apple-darwin`), LLVM 22.1.6;
- Criterion: 0.8.2, default features disabled;
- timing protocol: release mode, 1 second warm-up, 2 second measurement, 50
  samples;
- allocation protocol: release mode, one full 64-entry replacement warm-up,
  then 10,000 operations per reported window.

These are local reference-machine baselines, not universal performance claims.

## Allocation result

| Public path | Baseline calls / 10k | Current calls / 10k | Current requested bytes |
|---|---:|---:|---:|
| ready lookup | 0 | 0 | 0 |
| ready reserve hit | 0 | 0 | 0 |
| one-entry owned lease | 20,000 | 0 | 0 |
| 32-entry owned lease | 50,000 | 10,000 | 4,960,000 |
| 32-entry reused lease | not available | 0 | 0 |
| placement churn | 20,002 | 1 transition; then 0 | 4,360 transition; then 0 |
| CPU populate/upload churn | 20,002 | 1 transition; then 0 | 4,360 transition; then 0 |
| collect one dirty region | 0 | 0 | 0 |

The owned batch path necessarily returns owned variable-length storage and now
allocates exactly once. `AtlasCache::lease_into` fills a caller-owned empty
`Lease`; clearing and reusing one token per retired frame amortizes that buffer
to zero allocations.

Placement and CPU churn each observe one internal table rebalance in the first
10,000-operation transition window. The immediately following 10,000-operation
window is allocation-free. Slot generations, pin anchors, eviction output, and
upload output all reuse retained capacity rather than growing per artifact.

## Criterion result

| Workload | Estimate |
|---|---:|
| ready lookup | 3.404–3.431 ns |
| ready reserve hit | 4.516–4.539 ns |
| one-entry lease | 8.964–9.072 ns |
| owned 32-entry lease | 349.5–352.5 ns |
| reused 32-entry lease | 321.0–324.9 ns |
| reserve + publish + evict | 191.4–192.7 ns |
| reserve + R8 populate + collect + acknowledge | 248.9–250.4 ns |
| collect one dirty region | 1.366–1.385 ns |

The pre-optimization quick screen put one-entry leasing at 32.9–34.2 ns. Its
allocation removal is therefore both the clearest allocation win and the only
large timing change claimed from this slice. Other timing differences remain
screens until repeated on additional runs and machines.

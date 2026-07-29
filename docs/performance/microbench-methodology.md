# Focused Microbenchmark Methodology

Deloxide uses short microbenchmarks as an implementation checkpoint. They are not a
replacement for application benchmarks or the separate evaluation suite.

## Compared revisions

- Release baseline: commit `3b28ace`
- Candidate: the current hardening branch at the point of measurement

Both revisions use the same benchmark source, Rust toolchain, release profile,
feature set, and machine.

## Focused cases

- Uncontended Mutex lock/unlock
- Uncontended Mutex lock/drop without reading protected data
- Matched `parking_lot::Mutex::lock` control
- Matched `parking_lot::Mutex::try_lock` control
- Uncontended RwLock read
- Uncontended RwLock write
- Two-thread Mutex handoff

The full evaluation suite is intentionally excluded.

## Interpretation

Criterion reports an estimate interval and outliers for 30 short samples. A movement
above 3% is repeated before interpretation. A repeatable movement above 5% triggers
implementation review; it is not hidden by averaging it with slower application
workloads. Small nanosecond changes are reported both as percentages and absolute
time because either representation alone can be misleading.

Machine load, thermal state, frequency scaling, and scheduler placement can change
results between runs. A single run is never described as a regression or an
improvement.

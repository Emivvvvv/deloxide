# Performance

The focused harness measures uncontended Mutex, RwLock read/write, and a small
two-thread handoff. Methodology is versioned in
[`docs/performance/microbench-methodology.md`](https://github.com/Emivvvvv/deloxide/blob/main/docs/performance/microbench-methodology.md).

Short runs vary with CPU frequency, thermals, and scheduling. A 3% movement triggers
a repeat; it is not automatically a regression. Results are published only with
baseline/candidate commits, toolchain, hardware, feature set, median, and spread.
The complete evaluation remains a separate release-candidate activity.

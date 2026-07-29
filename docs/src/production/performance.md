# Performance and benchmarks

Deloxide was evaluated across correctness tests, stress-driven manifestation, lock
microbenchmarks, and an application-shaped raytracer. The complete study is
described in the preprint
[“Deloxide: Low-Overhead Real-time Deadlock Detection and Visualization Framework for Rust”](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=6389109).

The full benchmark suite has not yet been rerun on the 1.1 release candidate.
Focused 1.1 microbenchmarks remain in the same nanosecond range and show no
material default fast-path regression. The historical full-suite tables below
remain the broad comparison until the complete 1.1 rerun is published.

Abbreviations:

- **STD**: `std::sync`;
- **PL+DD**: `parking_lot` with `deadlock_detection`;
- **ND**: `no_deadlocks`;
- **DX**: Deloxide default;
- **DX (LOG)**: Deloxide logging and visualization; and
- **DX (COMP)**: Deloxide component-based stress mode.

## Evaluation design

The evaluation asks three different questions:

1. **Correctness:** does the wait-for detector report deterministic cycles and
   avoid reporting selected valid synchronization patterns?
2. **Manifestation:** how often do scheduling strategies make a deliberately
   timing-sensitive deadlock appear?
3. **Performance:** what is the cost in focused lock operations and in a
   lock-heavy rendering workload?

These measurements should not be collapsed into one number. Stress mode is
supposed to slow and perturb a test. The production-oriented comparison is the
default fast path.

## Lock microbenchmarks

| Metric | STD | PL+DD | **DX** | DX (LOG) | DX (COMP) | ND |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| **Mutex lock** | 8.7 ns | 9.9 ns | **10.8 ns** | 58.1 ns | 229.4 ns | 10,527 ns |
| **RwLock write** | 10.1 ns | 12.8 ns | **13.9 ns** | 57.7 ns | 234.1 ns | 10,797 ns |
| **RwLock read** | 13.9 ns | 16.1 ns | **62.4 ns** | 85.5 ns | 222.5 ns | 10,895 ns |
| **Condvar** | 17.1 µs | 17.2 µs | **19.6 µs** | 17.4 µs | 20.3 µs | 2,100 µs |

Default Deloxide stays close to the primitive baselines in the focused Mutex,
write-lock, and Condvar cases. RwLock reads cost more because Deloxide must count
live readers for later writer dependencies. Logging adds event construction and
queueing. Component stress adds intentional delay and should never be interpreted
as production overhead.

## Raytracing workload

The application benchmark uses a shared, tile-based framebuffer. At 1920×1080,
workers perform 129,600 lock acquisitions per frame while tracing a scene with a
maximum recursion depth of 50.

| Configuration | 426×240 | 854×480 | 1280×720 | 1920×1080 |
| :--- | ---: | ---: | ---: | ---: |
| **STD** | 0.81 s ± 0.03 | 3.41 s ± 0.15 | 7.33 s ± 0.31 | 17.22 s ± 0.65 |
| **PL+DD** | 0.81 s ± 0.00 | 3.26 s ± 0.02 | 7.19 s ± 0.03 | 18.32 s ± 0.06 |
| **DX (default)** | **0.80 s ± 0.00** | **3.18 s ± 0.01** | **7.09 s ± 0.03** | **16.67 s ± 0.09** |
| **ND** | 33.0 s ± 31.4 | 220.9 s ± 182 | 192.9 s ± 281 | 329.1 s ± 554 |

In this historical run, Deloxide completed the 1080p workload 9% faster than
PL+DD. Detection does not inherently make an application faster; the result
reflects the complete lock implementation and the workload's contention shape.

## Stress-testing manifestation

Each strategy ran the same deliberately deadlocking scenarios 1,000 times:

| Scenario | DX passive | DX random | DX aggressive | **DX component** | PL+DD | ND |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| `dining_philosophers` | 40.3% | 65.4% | 83.6% | **98.7%** | 54.1% | 75.5% |
| `five_lock_cycle` | 100.0% | 100.0% | 100.0% | **100.0%** | 100.0% | 99.4% |
| `rwlock_deadlock` | 41.5% | 71.1% | 88.1% | **100.0%** | 60.4% | 100.0% |
| `three_lock_cycle` | 88.7% | 98.7% | 99.4% | **100.0%** | 77.4% | 98.7% |
| `two_lock` | 17.6% | 57.2% | 81.8% | **98.7%** | 25.2% | 73.6% |
| **Average** | **57.6%** | **78.5%** | **90.6%** | **99.5%** | **63.4%** | **89.4%** |

The component strategy's purpose is to make latent schedules appear during
testing. These percentages are manifestation rates for the evaluated scenarios,
not a claim that every real-world deadlock will reproduce.

## Correctness controls

The suite includes deterministic cycles built with barriers plus valid patterns
designed to look suspicious to a simpler graph. In the evaluated cases, the WFG
detector reported the guaranteed deadlocks and produced zero flags across the
selected false-positive controls.

That is an empirical result for the tested patterns, not a proof of perfect
accuracy for every possible program.

## Focused 1.1 checkpoint

The current 1.1-focused Criterion run used 30 samples, a one-second warmup, and a
two-second measurement window on an Apple M1 Pro:

| Operation | Median | 95% interval |
| --- | ---: | ---: |
| Deloxide Mutex, uncontended | **9.12 ns** | 9.07 to 9.22 ns |
| `parking_lot` Mutex, uncontended | 10.28 ns | 9.95 to 10.50 ns |
| Deloxide RwLock write, uncontended | **9.17 ns** | 9.08 to 9.21 ns |
| Deloxide RwLock read, uncontended | 58.07 ns | 54.06 to 62.78 ns |
| Deloxide Mutex, two-thread handoff | 37.89 µs | 37.12 to 39.03 µs |

![Focused 1.1 Mutex latency medians](../assets/mutex-latency.svg)

The focused check supports the “no material fast-path regression” statement; it
does not replace the full cross-tool suite.

## Reproducing the evidence

The repository's
[evaluation record](../../performance/evaluation-2026-07-29.md) contains the
current commands, toolchain, commits, raw CSVs, and paired-seed controls.

Benchmark on the hardware, feature set, contention topology, and workload you
plan to ship. Microbenchmarks establish mechanism cost; only the application can
establish production impact.

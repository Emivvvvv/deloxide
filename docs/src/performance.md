# Performance and benchmarks

The current measurements were recorded on an Apple M1 Pro with Rust
1.90.0-nightly at Deloxide commit
`baa9e89ef87191d25832b4ecf567c5dd26b4a6ae`. They are evidence from one machine,
not a promise for every application.

## Fast-path latency

The default-feature Criterion run used 30 samples, a one-second warmup, and a
two-second measurement window:

| Operation | Median | 95% interval |
| --- | ---: | ---: |
| Deloxide Mutex, uncontended | **9.12 ns** | 9.07–9.22 ns |
| `parking_lot` Mutex, uncontended | 10.28 ns | 9.95–10.50 ns |
| Deloxide RwLock write, uncontended | **9.17 ns** | 9.08–9.21 ns |
| Deloxide RwLock read, uncontended | 58.07 ns | 54.06–62.78 ns |
| Deloxide Mutex, two-thread handoff | 37.89 µs | 37.12–39.03 µs |

![Focused Mutex latency medians](assets/mutex-latency.svg)

The Mutex comparison is encouraging, but it is deliberately narrow. The
RwLock-read path performs more detector work, and the handoff result includes OS
scheduling noise.

## Making rare deadlocks appear

In 1,000 paired runs of the deliberate two-lock scenario, the active deadlock
manifested in:

| Mode | Detections |
| --- | ---: |
| Default | 170 / 1,000 (17.0%) |
| Random stress | 591 / 1,000 (59.1%) |
| Aggressive stress | 837 / 1,000 (83.7%) |
| Component delays | **999 / 1,000 (99.9%)** |

![Active-cycle manifestation rates](assets/manifestation-rate.svg)

These are reproduction rates, not throughput results or guarantees that another
bug will appear.

## Application result

A 1920×1080 raytracer workload with 129,600 lock acquisitions per frame produced
these ten-run means:

| Configuration | Mean frame time |
| --- | ---: |
| `parking_lot` | **21.63 s** |
| `std::sync` | 23.48 s |
| Deloxide | 23.75 s |

On this workload, `parking_lot` was faster and Deloxide was 1.2% above
`std::sync`. That less-flattering result is useful: benchmark your real lock mix
instead of assuming a microbenchmark predicts the application.

The [full evaluation record](../performance/evaluation-2026-07-29.md) contains
commands, raw CSV links, environment details, and all manifestation scenarios.

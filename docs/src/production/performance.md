# Measure Performance

Performance evidence is a workload-specific comparison, not a property that
transfers unchanged from one service to another. The current record is
[`evaluation-2026-07-29.md`](../../performance/evaluation-2026-07-29.md), with
raw CSVs, commands, commit, toolchain, and machine capture. It measured Deloxide
commit `baa9e89ef87191d25832b4ecf567c5dd26b4a6ae` on an Apple M1 Pro (16 GiB,
Darwin/aarch64) with Rust and Cargo 1.90.0-nightly. It is deliberately one
machine's evidence, not a universal speed or overhead guarantee.

## What the default fast path avoids

For an eligible uncontended `Mutex` or exclusive `RwLock` acquisition in the
default build, Deloxide first uses the physical `parking_lot` try operation,
publishes the owner hint, and observes the per-lock slow-waiter state. When no
slow waiter is present and optional lock-order tracking is not compiled, it
returns without the global detector mutex, ownership-map updates, wait-intent
allocation, or wait-for graph traversal. Release can remain local under the
corresponding conditions.

That statement does not apply to every operation: an `RwLock` read enters the
detector so every live reader can be counted for a later writer, contention
registers and validates current dependencies, and optional features add their
own work. The detailed branch and contention protocol is in [Architecture and
Fast Path](../internals/architecture.md).

## Focused latency evidence

The focused Criterion harness used the default feature set, 30 samples, a
one-second warmup, a two-second measurement window, and 95% bootstrap confidence
intervals. Its command was:

```sh
cargo bench --bench fast_path -- --noplot
```

| Case | Median | 95% interval | Relative reading |
| --- | ---: | ---: | --- |
| Deloxide Mutex, uncontended | 9.12 ns | 9.07–9.22 ns | 1.16 ns (11.2%) below the included `parking_lot` Mutex result in this harness. |
| `parking_lot` Mutex, uncontended | 10.28 ns | 9.95–10.50 ns | Reference row for the preceding, same-harness comparison. |
| Deloxide RwLock read, uncontended | 58.07 ns | 54.06–62.78 ns | 6.34× the measured Deloxide write median; the modes do different detector work, so this is not a substitute implementation ranking. |
| Deloxide RwLock write, uncontended | 9.17 ns | 9.08–9.21 ns | 0.46% above the measured Deloxide Mutex median; this cross-primitive ratio is descriptive only. |
| Deloxide Mutex, two-thread handoff | 37.89 µs | 37.12–39.03 µs | About 4,154× the uncontended Mutex median; physical parking and scheduling dominate this different operation shape. |

The CSV has no same-run alternative RwLock implementation, so it cannot support a
relative claim against `parking_lot` for reads or writes. The RwLock-read interval
is broad, and the handoff result includes scheduler behavior; neither supports a
fine-grained ranking. In particular, lower values in a microbenchmark are not a
prediction of lower request latency in a loaded application.

![Focused Mutex latency medians from the 2026-07-29 CSV](../assets/mutex-latency.svg)

## Optional work changes the question being measured

The focused latency rows are default-build rows. They do not price the following
separate behaviors:

- `lock-order-graph` makes eligible exclusive acquisitions visible for historical
  `Lock -> Lock` order tracking and cycle checks. Its findings are potential
  `LockOrderViolation` risks, not active wait-for cycles.
- `logging-and-visualization` adds event construction, an asynchronous logger,
  serialization, file I/O, and queue growth. The current ordinary-event queue is
  unbounded, so its memory behavior needs operational monitoring rather than a
  fixed queue-capacity assumption.
- `stress-test` intentionally delays or perturbs lock attempts to expose
  schedules. It is a focused-test/reproduction tool and is expected to alter
  execution time and determinism.

Measure each selected feature and their combination with the deployment's real
workload. See [Select Features and Configuration](../rust/features.md) for which
runtime work each feature enables.

## Manifestation is schedule evidence, not throughput evidence

The manifestation measurement built four feature configurations once, copied the
five scenario executables per configuration, then ran 1,000 paired seeds through
each scenario × mode combination. Each outer seed was reused across all 20
combinations; the final files contained 20,000 rows. `detected` counts only an
active `WaitForGraph` result from deliberately deadlocking scenarios. No
lock-order finding is included.

For example, the two-lock scenario manifested in 170/1,000 default runs (17.0%),
591/1,000 random-default runs (59.1%), 837/1,000 aggressive runs (83.7%), and
999/1,000 component-delay runs (99.9%). The five-lock cycle manifested in all
1,000 runs for every selected mode on this machine. Those results show that the
selected schedule perturbations made these intended cycles appear more often in
this harness; they do not compare production throughput or prove a finite run
will reproduce a different bug.

![Active-cycle manifestation rate for selected deliberate deadlock schedules](../assets/manifestation-rate.svg)

The complete scenario table, stopped wrapper attempt, direct-executable method,
paired-seed control, and finite false-positive control are recorded in the
[evaluation record](../../performance/evaluation-2026-07-29.md#manifestation-measurement).

## Focused application result

The evaluation also timed a single raytracer scene: 1920×1080, 128 samples per
pixel, depth 50, ten runs per configuration, with 129,600 lock acquisitions per
row. Mean frame times were 23,478.8 ms for `use_std`, 21,625.0 ms for
`parking_lot_deadlock`, and 23,754.5 ms for `deloxide`. On that workload,
`parking_lot_deadlock` was lower than Deloxide and the standard configuration;
Deloxide was 275.7 ms (1.2%) above `use_std` by the reported means. The ten
individual runs vary substantially, so those means should guide investigation,
not establish a product-wide ranking.

This is an important counterexample to a broad speed claim: baselines can win,
and a difference can be too noisy or too workload-specific to interpret. The
raw [raytracer CSV](../../performance/results/2026-07-29-raytracer.csv) is the
authoritative data for this one scene.

## Historical context and reproduction

The README at the evaluated commit described a small Criterion methodology and
the need to repeat movements beyond run-to-run noise, but it published no direct
numeric result comparable with this CSV. Separate historical evaluation output
used different provenance and aggregate-only values. It remains context, not a
current comparison; do not blend it into the table above.

To reproduce the documented evidence, start from the exact runtime commit and
environment capture in the evaluation record, leaving runtime source untouched:

```sh
# Focused default-path measurements
cargo bench --bench fast_path -- --noplot

# Regenerate the checked-in figures from the versioned CSVs
python3 scripts/generate_doc_charts.py
python3 scripts/generate_doc_charts.py --check
```

The record also contains the temporary evaluation-copy setup, direct
manifestation loop, raytracer build/timing commands, and the exact `Cargo paths`
override used to resolve the tested Deloxide checkout. Reproduce on the target
hardware with the target feature set, process limits, data set, and concurrency
profile. CPU frequency, thermals, OS scheduling, allocator behavior, lock mix,
and contention topology can change both the absolute values and their ordering.

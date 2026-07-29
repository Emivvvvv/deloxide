<p align="center">
  <img src="images/deloxide_logo_orange.png" alt="Deloxide" width="140">
</p>

<h1 align="center">Deloxide</h1>

<p align="center">
  Runtime deadlock detection and diagnosis for Rust, with a secondary C integration surface.
</p>

<p align="center">
  <a href="https://crates.io/crates/deloxide"><img alt="crates.io" src="https://img.shields.io/crates/v/deloxide.svg"></a>
  <a href="https://docs.rs/deloxide"><img alt="docs.rs" src="https://docs.rs/deloxide/badge.svg"></a>
  <a href="https://github.com/Emivvvvv/deloxide/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Emivvvvv/deloxide/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://crates.io/crates/deloxide"><img alt="downloads" src="https://img.shields.io/crates/d/deloxide.svg"></a>
  <a href="LICENSE-MIT"><img alt="MIT or Apache-2.0 license" src="https://img.shields.io/crates/l/deloxide.svg"></a>
</p>

<p align="center">
  <strong><a href="https://emivvvvv.github.io/deloxide/">Read the manual</a></strong>
  · <a href="https://docs.rs/deloxide">Rust API</a>
  · <a href="examples/diagnose_deadlock.rs">Runnable example</a>
  · <a href="https://github.com/Emivvvvv/deloxide/issues">Report a bug</a>
  · <a href="https://github.com/Emivvvvv/deloxide/issues/new?template=feature_request.yml">Request a feature</a>
</p>

---

## Turn a hang into an answer

An intermittent hang is hard to debug: add logging, attach a debugger, or rerun
the workload and the schedule changes. Deloxide is a runtime deadlock detector
and diagnosis toolkit. Its tracked synchronization primitives turn an unexplained
hang into a concrete thread-and-lock cycle.

It fits familiar guard-based Rust code, sends structured callback evidence about
the participating threads and waited locks, and can reconstruct tracked events in
an interactive visualization. Start with active diagnosis; add historical
lock-order analysis or stress testing only when the investigation calls for them.

![Deloxide visualization showing a tracked event timeline beside the thread-and-lock dependency graph that formed a deadlock](docs/src/assets/visualization.png)

*The viewer reconstructs the tracked execution leading to a report, so an
investigation can move from IDs in a callback back to the lock interactions that
formed the cycle.*

On the recorded Apple M1 Pro default-build Mutex microbenchmark, Deloxide measured
9.12 ns median versus the included `parking_lot` Mutex's 10.28 ns. That is a
blazingly fast result for this narrowly scoped fast path—not a universal
application speed claim; RwLock-read and handoff measurements carry explicit
noise warnings in the
[full evaluation](docs/performance/evaluation-2026-07-29.md).

## Try it on the failing path

Add the default active detector, then initialize it before creating the tracked
locks and worker threads:

```toml
[dependencies]
deloxide = "1.1"
```

```rust
use deloxide::{DeadlockInfo, DeadlockSource, Deloxide};

Deloxide::new()
    .callback(|report: DeadlockInfo| match report.source {
        DeadlockSource::WaitForGraph => {
            eprintln!("active cycle: {:?}", report.thread_cycle);
            eprintln!("waited locks: {:?}", report.thread_waiting_for_locks);
        }
        DeadlockSource::LockOrderViolation => {
            eprintln!("potential lock order: {:?}", report.lock_order_cycle);
        }
    })
    .start()
    .expect("initialize before the workload");
```

Use the tracked `Mutex` where the two paths meet. The smallest dangerous shape is
opposite order: each worker keeps its first lock, synchronizes, then requests the
other one.

```rust
// First worker: left, then right.
let _left = left_for_first.lock();
barrier_for_first.wait();
let _right = right_for_first.lock();

// Second worker: right, then left.
let _right = right_for_second.lock();
barrier_for_second.wait();
let _left = left_for_second.lock();
```

The canonical example reports the active finding in this form:

```text
ACTIVE DEADLOCK
source: WaitForGraph
thread cycle: [ThreadId(...), ThreadId(...)]
thread waiting for locks: [(ThreadId(...), LockId(...)), ...]
```

`WaitForGraph` means Deloxide validated a current blocked cycle among tracked
waits and incompatible owners. It is different from `LockOrderViolation`, which
is a **potential** historical ordering risk, not evidence that threads are blocked
now. Run the complete, compile-tested [two-lock diagnosis](examples/diagnose_deadlock.rs)
with `cargo run --example diagnose_deadlock`, then follow the manual's
[first diagnosis](https://emivvvvv.github.io/deloxide/getting-started.html). To
preserve the path to the report, enable
[logging and visualization](https://emivvvvv.github.io/deloxide/visualization.html)
and open the retained log with the viewer.

## Why Deloxide

- **Evidence at the synchronization boundary.** Tracked `Mutex`, `RwLock`, and
  `Condvar` operations feed the detector instead of asking a timeout to guess why
  a process stopped moving.
- **An active report you can act on.** `DeadlockInfo` includes the source,
  thread cycle, and waited-lock evidence. Keep its callback bounded and hand
  slower incident work to your own queue or supervisor.
- **Adoption that follows Rust code.** Guard-based wrappers and
  `deloxide::thread` lifecycle helpers make it practical to instrument a suspect
  operation incrementally.
- **Depth when the incident needs it.** Logs reconstruct a tracked execution;
  optional order analysis and stress modes help find a problem before or while it
  is reproduced.

Only synchronization that passes through those tracked wrappers is in scope.
Raw `std::sync`/`parking_lot` locks, third-party primitives, OS waits, channels,
I/O, and remote resources can leave a dependency outside the graph.

## Choose how deep to look

The default is the active wait-for graph. Optional Cargo features add distinct
questions, evidence, and costs; they are not runtime switches.

| Capability | Activate | Purpose and evidence | Main cost | Deep dive |
| --- | --- | --- | --- | --- |
| Active wait-for detection | Default: `deloxide = "1.1"` | Reports a current, validated `WaitForGraph` cycle among tracked waits and incompatible owners. | Supported-lock and contention tracking; measure it on the deployment workload. | [Choose a mode](https://emivvvvv.github.io/deloxide/choosing-a-mode.html) |
| Logging and visualization | `features = ["logging-and-visualization"]` | Records tracked events and reconstructs their timeline and graph; it accompanies a report without changing its certainty. | Queueing, serialization, file I/O, retained log data, and browser transfer. | [Logging and visualization](https://emivvvvv.github.io/deloxide/visualization.html) |
| Lock-order analysis | `features = ["lock-order-graph"]` | Finds a historical inconsistent acquisition order as a potential `LockOrderViolation`, useful in development or CI. | Order-graph storage and traversal. | [Find potential lock-order risks](https://emivvvvv.github.io/deloxide/diagnosis/lock-order.html) |
| Random stress testing | `features = ["stress-test"]` + `.with_random_stress()` | Broadly perturbs timing to make a suspected schedule manifest in focused tests; perturbation itself is not a deadlock finding. | Slower, less deterministic test execution. | [Stress-test a suspected race](https://emivvvvv.github.io/deloxide/diagnosis/stress-testing.html) |
| Component-based stress testing | `features = ["stress-test"]` + `.with_component_stress()` | Adds targeted delays guided by tracked acquisition relationships to improve a focused reproduction. | Relationship tracking plus test-time delays and disturbance. | [Stress-test a suspected race](https://emivvvvv.github.io/deloxide/diagnosis/stress-testing.html) |

Use one dependency declaration for the feature set you need:

```toml
[dependencies]
deloxide = { version = "1.1", features = [
  "logging-and-visualization",
  "lock-order-graph",
  "stress-test",
] }
```

The [Rust adoption guide](https://emivvvvv.github.io/deloxide/rust/adoption.html)
covers `Mutex`, `RwLock`, `Condvar`, guards, and incremental migration. The
[lifecycle guide](https://emivvvvv.github.io/deloxide/rust/lifecycle.html) covers
one process-wide detector, tracked threads, and bounded callback handoff. The
complete feature/configuration matrix is in the
[manual](https://emivvvvv.github.io/deloxide/rust/features.html).

## Performance you can inspect

The focused figures below are from Deloxide commit
[`baa9e89`](https://github.com/Emivvvvv/deloxide/commit/baa9e89ef87191d25832b4ecf567c5dd26b4a6ae), default features,
Criterion 0.7, and an Apple M1 Pro (16 GiB, Darwin/aarch64) using Rust 1.90.0-nightly.
They are a reproducible starting point, not a promise about a different workload.

| Focused Mutex operation | Median | 95% interval |
| --- | ---: | ---: |
| Deloxide, uncontended | 9.12 ns | 9.07–9.22 ns |
| `parking_lot`, uncontended | 10.28 ns | 9.95–10.50 ns |

![Focused 2026-07-29 Mutex latency medians for Deloxide and parking_lot](docs/src/assets/mutex-latency.svg)

The same evaluation's deliberately deadlocking two-lock schedule manifested in
170/1,000 default runs and 999/1,000 component-delay runs. That is schedule
evidence for this harness, not a production detection guarantee or throughput
comparison. The RwLock-read interval and two-thread handoff spread are noise
warnings; benchmark the selected feature set and contention profile in your own
application. See the [performance chapter](https://emivvvvv.github.io/deloxide/production/performance.html)
and the [complete evaluation record](docs/performance/evaluation-2026-07-29.md)
for raw CSVs, commands, environment, and the less flattering application result.

The optimized detector uses the physical `parking_lot` operation first on eligible
uncontended Mutex and exclusive RwLock paths. When there is no slow waiter and no
compiled order graph, it can avoid global detector-map and graph work; contention,
RwLock reads, and optional features take different paths. [Architecture and fast
path](https://emivvvvv.github.io/deloxide/internals/architecture.html) explains
the protocol and its boundaries.

## C support, kept secondary

Deloxide also produces a C library and the public
[`deloxide.h`](include/deloxide.h) header. Initialize it once, create tracked
objects, and register thread lifecycle when your C program owns thread creation:

```c
#include "deloxide.h"

if (deloxide_init(NULL, NULL) != 0) return 1;
void *mutex = deloxide_create_mutex();
if (mutex == NULL || deloxide_lock_mutex(mutex) != 0) return 1;
/* critical section */
deloxide_unlock_mutex(mutex);
deloxide_destroy_mutex(mutex);
```

This uses the same tracked primitive boundary, but Rust remains the primary
configuration path. The [C guide](https://emivvvvv.github.io/deloxide/c-guide.html)
covers build/link commands, callbacks, RwLock and Condvar operations, POSIX and
Windows thread registration, and exact lifecycle details.

## Boundaries and routes

- `WaitForGraph` is active, validated evidence for the tracked graph;
  `LockOrderViolation` is a potential historical ordering risk. Keep their
  response paths separate.
- Coverage is limited to tracked `Mutex`, `RwLock`, and `Condvar` operations.
  External synchronization and resources may be absent or only partially visible.
- Logging, order analysis, and stress testing have their own memory, I/O, graph,
  or execution-time costs. Application-level benchmarking is still required.
- Deloxide does not prove that every hang is a deadlock or that an arbitrary
  process is free of deadlocks. Read the complete
  [coverage and limitations](https://emivvvvv.github.io/deloxide/production/limitations.html)
  before relying on a report operationally.

- [User manual](https://emivvvvv.github.io/deloxide/)
- [Rust API on docs.rs](https://docs.rs/deloxide)
- [Runnable examples](examples)
- [C header](include/deloxide.h)
- [Performance evaluation](docs/performance/evaluation-2026-07-29.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Changelog](CHANGELOG.md)
- [Issues](https://github.com/Emivvvvv/deloxide/issues)
- [MIT License](LICENSE-MIT) and [Apache License 2.0](LICENSE-APACHE)

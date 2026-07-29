<p align="center">
  <img src="images/deloxide_logo_orange.png" alt="Deloxide" width="140">
</p>

<h1 align="center">Deloxide</h1>

<p align="center">
  Blazingly fast runtime deadlock detection for Rust.<br>
  Turn an unexplained hang into a concrete thread-and-lock cycle.
</p>

<p align="center">
  <a href="https://crates.io/crates/deloxide"><img src="https://img.shields.io/crates/v/deloxide.svg" alt="crates.io release"></a>
  <a href="https://docs.rs/deloxide"><img src="https://docs.rs/deloxide/badge.svg" alt="docs.rs"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.85%2B-black.svg" alt="Rust 1.85+"></a>
  <a href="https://github.com/Emivvvvv/deloxide/actions/workflows/ci.yml"><img src="https://github.com/Emivvvvv/deloxide/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/deloxide"><img src="https://img.shields.io/crates/d/deloxide.svg" alt="downloads"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green.svg" alt="MIT or Apache-2.0"></a>
</p>

<p align="center">
  <a href="https://emivvvvv.github.io/deloxide/"><strong>Read the user manual »</strong></a>
  <br />
  <a href="https://docs.rs/deloxide">API reference</a>
  ·
  <a href="https://github.com/Emivvvvv/deloxide/tree/main/examples">Examples</a>
  ·
  <a href="https://github.com/Emivvvvv/deloxide/issues/new?labels=bug">Report a bug</a>
  ·
  <a href="https://github.com/Emivvvvv/deloxide/issues/new?labels=enhancement">Request a feature</a>
</p>

---

Deloxide detects active deadlocks and reports the exact thread-and-lock cycle.
Replace synchronization on the relevant path with its tracked `Mutex`, `RwLock`,
and `Condvar` wrappers, then initialize the detector once.

## Why Deloxide

- **Instant active detection:** validates wait-for cycles when they occur, without
  waiting for a polling interval.
- **Custom callbacks:** run your own application logic when a deadlock is found,
  such as recording diagnostics, notifying a supervisor, or sending an alert.
- **Interactive visualization:** inspect the event timeline and thread-lock graph
  in a browser.
- **Lock-order analysis:** find acquisition-order risks before they become active
  deadlocks.
- **Stress testing:** use random or component-based scheduling disturbance to
  reproduce timing-sensitive failures.
- **Low-overhead default path:** keeps eligible uncontended operations away from
  global graph work.
- **Rust first, C supported:** tracked Mutexes, RwLocks, condition variables, and
  thread registration are available through both interfaces.

![Deloxide visualization showing the event timeline and thread-lock cycle](docs/src/assets/visualization.png)

## Comparison

The Rust ecosystem's existing approaches make a difficult trade-off. Static
analysis can be noisy on complex paths. Passive monitors preserve performance but
do not force rare schedules and report only when polled. Synchronous graph
debuggers detect immediately but can make every lock operation expensive.

Deloxide bridges that gap: synchronous active detection with an Optimistic Fast
Path, plus lock-order analysis, stress testing, structured callbacks, and
interactive visualization in one toolkit.

| Feature | STD | PL+DD | ND | **DX** |
| :--- | :---: | :---: | :---: | :---: |
| **Mutex overhead** | 0.88× | 1.00× | 1063.33× | **1.09×** |
| **Raytracing at 1080p** | 0.94× | 1.00× | 17.96× | **0.91× (faster)** |
| **Detection method** | None | Async (poll) | Synchronous | **Synchronous (instant)** |
| **Lock-order analysis** | No | No | No | **Yes** |
| **Stress testing** | No | No | No | **Yes** |
| **Visualization** | No | No | Text dump | **Interactive URL** |
| **False-positive rate in evaluated WFG controls** | N/A | Zero | Zero | **Zero** |

*STD = `std::sync`, PL+DD = `parking_lot` with `deadlock_detection`, ND =
`no_deadlocks`, DX = Deloxide. Results are from the full evaluation.*

See [Why Deloxide](https://emivvvvv.github.io/deloxide/comparison.html) for the
detection-model differences behind the table.

## Quick start

```toml
[dependencies]
deloxide = "1.1"
```

Initialize once before the tracked workload:

```rust
use deloxide::{DeadlockSource, Deloxide};

Deloxide::new()
    .callback(|report| match report.source {
        DeadlockSource::WaitForGraph => {
            eprintln!("active cycle: {:?}", report.thread_cycle);
            eprintln!("waited locks: {:?}", report.thread_waiting_for_locks);
        }
        DeadlockSource::LockOrderViolation => {
            eprintln!("potential lock order: {:?}", report.lock_order_cycle);
        }
    })
    .start()
    .expect("start Deloxide");
```

The callback is application-controlled. It can record the report, export
telemetry, notify an incident system, capture additional diagnostics, or signal a
supervisor. Keep slow work outside the callback by handing the report to an
application-owned queue.

Then use Deloxide locks where the competing paths meet. An opposite-order cycle
produces evidence like:

```text
source: WaitForGraph
thread_cycle: [ThreadId(2), ThreadId(3)]
thread_waiting_for_locks: [(ThreadId(2), LockId(7)), (ThreadId(3), LockId(4))]
```

Run the complete example:

```console
cargo run --example diagnose_deadlock
```

## Performance evaluation

The full evaluation tested isolated lock latency, heavily contended Mutex and
RwLock workloads, deterministic deadlock detection, timing-sensitive
manifestation, nine complex deadlock-free patterns, and a shared-state raytracer.

| Evaluated result                                 | PL+DD | ND | **Deloxide** |
|--------------------------------------------------| ---: | ---: | ---: |
| Mutex lock latency                               | 9.9 ns | 10,527 ns | **10.8 ns** |
| 1080p raytracing                                 | 18.32 s | 329.1 s | **16.67 s** |
| Average passive manifestation                    | 63.2% | 89.6% | 57.2% |
| Manifestation with stress testing                | N/A | N/A | **99.6%** |
| False deadlock reports across nine safe patterns | Zero | Zero | **Zero** |

The focused 1.1 microbenchmark measured an uncontended Deloxide Mutex at
**9.12 ns** and the same-harness `parking_lot` Mutex at **10.28 ns**. This short
run is encouraging, but it is too narrow to claim better general performance. It
does show that the latest correctness fixes introduced no material default
fast-path overhead.

The [performance chapter](https://emivvvvv.github.io/deloxide/production/performance.html)
contains the methodology, complete tables, limitations, and reproduction record.
The broader study is available as the
[Deloxide preprint](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=6389109).

## Features and modes

| Mode | Enable | Use it for |
| --- | --- | --- |
| Active wait-for detection | Default | Report a current validated cycle |
| Custom callback | Default | Run application-defined incident handling |
| Logging and visualization | `logging-and-visualization` | Reconstruct the tracked event timeline |
| Lock-order analysis | `lock-order-graph` | Find potential acquisition-order risks |
| Random stress | `stress-test` + `with_random_stress()` | Broad schedule perturbation |
| Component stress | `stress-test` + `with_component_stress()` | Targeted reproduction delays |

`WaitForGraph` is active evidence. `LockOrderViolation` is a potential historical
risk; it does not mean threads are blocked now.

## Rust first, C supported

Rust is the primary interface. Deloxide also builds a C library using
[`include/deloxide.h`](include/deloxide.h), with tracked mutexes, RwLocks,
condition variables, callbacks, and thread registration.

- [User manual](https://emivvvvv.github.io/deloxide/)
- [Rust API](https://docs.rs/deloxide)
- [Examples](examples)
- [C guide](https://emivvvvv.github.io/deloxide/c-guide.html)
- [Choosing Deloxide or another tool](https://emivvvvv.github.io/deloxide/comparison.html)
- [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)

## License

Deloxide is available under the [MIT License](LICENSE-MIT) or
[Apache License 2.0](LICENSE-APACHE).

<p align="center">
  <img src="images/deloxide_logo_orange.png" alt="Deloxide" width="140">
</p>

<h1 align="center">Deloxide</h1>

<p align="center">
  Blazingly fast runtime deadlock detection for Rust.<br>
  Turn an unexplained hang into a concrete thread-and-lock cycle.
</p>

<p align="center">
  <a href="https://crates.io/crates/deloxide"><img alt="crates.io" src="https://img.shields.io/crates/v/deloxide.svg"></a>
  <a href="https://docs.rs/deloxide"><img alt="docs.rs" src="https://docs.rs/deloxide/badge.svg"></a>
  <a href="https://github.com/Emivvvvv/deloxide/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Emivvvvv/deloxide/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://crates.io/crates/deloxide"><img alt="downloads" src="https://img.shields.io/crates/d/deloxide.svg"></a>
  <a href="LICENSE-MIT"><img alt="MIT or Apache-2.0 license" src="https://img.shields.io/crates/l/deloxide.svg"></a>
</p>

<p align="center">
  <strong><a href="https://emivvvvv.github.io/deloxide/">User manual</a></strong>
  · <a href="https://docs.rs/deloxide">Rust API</a>
  · <a href="examples/diagnose_deadlock.rs">Runnable example</a>
  · <a href="https://github.com/Emivvvvv/deloxide/issues">Issues</a>
</p>

---

## Deadlocks should leave evidence

A deadlock can keep a process alive while useful work has stopped. It is often
timing-sensitive: add a log line or attach a debugger and the failure disappears.

Deloxide instruments the synchronization boundary itself. Replace the locks on a
suspect path with tracked `Mutex`, `RwLock`, and `Condvar` wrappers. When threads
block in a cycle, Deloxide reports the participating threads and the lock each one
is waiting for.

![Deloxide visualization showing the event timeline and thread-lock cycle](docs/src/assets/visualization.png)

## Why switch to Deloxide?

The Rust ecosystem has good lock primitives and useful debugging tools. Deloxide
combines the parts normally spread across several of them:

| Capability | `std::sync` | `parking_lot` detector | `no_deadlocks` | **Deloxide** |
| --- | :---: | :---: | :---: | :---: |
| `Mutex`, `RwLock`, and `Condvar` | Yes | Yes | Yes | **Yes** |
| Active runtime cycle detection | No | When explicitly checked | Yes | **Yes, on the blocking path** |
| Structured callback report | No | No | No | **Yes** |
| Potential lock-order warnings | No | No | No | **Optional** |
| Schedule stress testing | No | No | No | **Random + targeted modes** |
| Interactive event visualization | No | No | No | **Optional** |
| Rust and C integration | Rust | Rust | Rust | **Rust + C** |

Use `std::sync` or plain `parking_lot` when you only need synchronization.
Use `parking_lot`'s experimental detector when periodic inspection is enough.
Use `no_deadlocks` as a synchronous debugging replacement. Choose Deloxide when
you want one path from rare-bug reproduction to an active callback, visualization,
and production diagnosis.

## From hang to answer

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

## Measured performance

Current results were recorded at commit
[`baa9e89`](https://github.com/Emivvvvv/deloxide/commit/baa9e89ef87191d25832b4ecf567c5dd26b4a6ae)
on an Apple M1 Pro with Rust 1.90.0-nightly.

| Focused operation | Median | 95% interval |
| --- | ---: | ---: |
| Deloxide Mutex, uncontended | **9.12 ns** | 9.07–9.22 ns |
| `parking_lot` Mutex, uncontended | 10.28 ns | 9.95–10.50 ns |
| Deloxide RwLock write, uncontended | **9.17 ns** | 9.08–9.21 ns |
| Deloxide RwLock read, uncontended | 58.07 ns | 54.06–62.78 ns |
| Deloxide Mutex, two-thread handoff | 37.89 µs | 37.12–39.03 µs |

![Focused Mutex latency comparison](docs/src/assets/mutex-latency.svg)

A 1920×1080 raytracer with 129,600 lock acquisitions per frame gives a more
application-shaped counterpoint:

| Configuration | Ten-run mean |
| --- | ---: |
| `parking_lot` | **21.63 s** |
| `std::sync` | 23.48 s |
| Deloxide | 23.75 s |

Deloxide was 1.2% above `std::sync` in that workload; `parking_lot` was faster.
The microbenchmark shows a very small default Mutex fast path, while the raytracer
shows why you should still benchmark your own contention pattern.

Stress modes can make rare schedules much easier to reproduce. In 1,000 paired
runs of the deliberate two-lock scenario:

| Default | Random stress | Aggressive stress | Component delays |
| ---: | ---: | ---: | ---: |
| 17.0% | 59.1% | 83.7% | **99.9%** |

These are manifestation rates, not throughput or universal detection guarantees.
See the [performance chapter](https://emivvvvv.github.io/deloxide/performance.html)
and [full evaluation record](docs/performance/evaluation-2026-07-29.md).

## Pick the tool you need

| Mode | Enable | Use it for |
| --- | --- | --- |
| Active wait-for detection | Default | Report a current validated cycle |
| Logging and visualization | `logging-and-visualization` | Reconstruct the tracked event timeline |
| Lock-order analysis | `lock-order-graph` | Find potential acquisition-order risks |
| Random stress | `stress-test` + `with_random_stress()` | Broad schedule perturbation |
| Component stress | `stress-test` + `with_component_stress()` | Targeted reproduction delays |

`WaitForGraph` is active evidence. `LockOrderViolation` is a potential historical
risk; it does not mean threads are blocked now.

## Boundaries

Deloxide sees synchronization performed through its wrappers. A dependency that
crosses raw locks, third-party primitives, channels, I/O, another process, or a
remote service can remain outside the graph.

Optional logging adds queueing and file I/O. Stress modes intentionally change
timing. Benchmark the feature set you plan to ship and keep callbacks short.

## Rust first, C supported

Rust is the primary interface. Deloxide also builds a C library using
[`include/deloxide.h`](include/deloxide.h), with tracked mutexes, RwLocks,
condition variables, callbacks, and thread registration.

- [Short user manual](https://emivvvvv.github.io/deloxide/)
- [Rust API](https://docs.rs/deloxide)
- [Examples](examples)
- [C guide](https://emivvvvv.github.io/deloxide/c-guide.html)
- [Choosing Deloxide or another tool](https://emivvvvv.github.io/deloxide/comparison.html)
- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)
- [Changelog](CHANGELOG.md)

## License

Deloxide is available under the [MIT License](LICENSE-MIT) or
[Apache License 2.0](LICENSE-APACHE).

<p align="center">
  <img src="images/deloxide_logo_orange.png" alt="Deloxide" width="180">
</p>

<h1 align="center">Deloxide</h1>

<p align="center">
  Runtime deadlock detection and diagnosis for Rust and C.<br>
  Keep the uncontended path small. Get a concrete thread-and-lock cycle when progress stops.
</p>

<p align="center">
  <a href="https://crates.io/crates/deloxide"><img alt="crates.io" src="https://img.shields.io/crates/v/deloxide.svg"></a>
  <a href="https://docs.rs/deloxide"><img alt="docs.rs" src="https://docs.rs/deloxide/badge.svg"></a>
  <a href="https://github.com/Emivvvvv/deloxide/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Emivvvvv/deloxide/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://crates.io/crates/deloxide"><img alt="downloads" src="https://img.shields.io/crates/d/deloxide.svg"></a>
  <a href="LICENSE-MIT"><img alt="license" src="https://img.shields.io/crates/l/deloxide.svg"></a>
</p>

<p align="center">
  <strong><a href="https://emivvvvv.github.io/deloxide/">Read the user manual</a></strong>
  · <a href="https://docs.rs/deloxide">Rust API</a>
  · <a href="examples">Examples</a>
  · <a href="https://github.com/Emivvvvv/deloxide/issues">Report a bug</a>
  · <a href="https://github.com/Emivvvvv/deloxide/issues/new?template=feature_request.yml">Request a feature</a>
</p>

---

## Why Deloxide

Deadlocks are often schedule-dependent: attach a debugger, add logging, or rerun the
workload and the failure disappears. Deloxide instruments the synchronization
boundary itself. When tracked threads form a blocked dependency cycle, the callback
receives the participating thread IDs and the locks each thread is waiting to
acquire.

- **Active runtime detection** using a direct thread-to-thread wait-for graph
- **Optimized uncontended path** for Mutex and RwLock writers
- **Mutex, RwLock, and Condvar** wrappers with guard-based Rust APIs
- **Rust and C integration** from one implementation
- **Optional lock-order analysis** for dangerous historical acquisition patterns
- **Optional stress scheduling** to improve manifestation during testing
- **Optional logging and visualization** for incident reconstruction

Deloxide distinguishes active wait-for cycles from potential lock-order violations,
so users can choose the evidence level appropriate to the environment.

## Choose the right mode

| Mode | What it answers | Recommended environment |
|---|---|---|
| Active wait-for graph | “Which tracked threads are blocked on each other now?” | Runtime and production diagnosis |
| Lock-order graph | “Have we observed an inconsistent acquisition order?” | Development and CI |
| Stress mode | “Can schedule variation make this bug manifest?” | Focused testing |
| Logging and visualization | “How did the tracked execution reach this state?” | Incident analysis |

The default build enables active detection without the optional logging pipeline.

## Where it fits

| Approach | Observes an active cycle | Explains participating locks | Production-oriented path | Finds potential ordering risk |
|---|---:|---:|---:|---:|
| Standard synchronization primitives | No | No | Yes | No |
| Static analysis | No | Sometimes | Build-time | Yes |
| Development-only lock-order detector | No | Yes | Usually no | Yes |
| **Deloxide** | **Yes** | **Yes** | **Yes, with measured overhead** | **Optional** |

Deloxide sees synchronization performed through its wrappers. A dependency crossing
an untracked raw lock, operating-system resource, or third-party primitive can remain
outside the graph.

## Quick start

```toml
[dependencies]
deloxide = "1.0"
```

```rust
use deloxide::{Deloxide, Mutex};

fn main() {
    Deloxide::new()
        .callback(|info| {
            eprintln!("source: {:?}", info.source);
            eprintln!("threads: {:?}", info.thread_cycle);
            eprintln!("waited locks: {:?}", info.thread_waiting_for_locks);
        })
        .start()
        .expect("detector initialization");

    let value = Mutex::new(41);
    *value.lock() += 1;
}
```

The repository includes compile-tested
[Mutex](examples/basic_mutex.rs), [RwLock](examples/rwlock.rs), and
[Condvar](examples/condvar.rs) examples.

### Optional capabilities

```toml
[dependencies]
deloxide = { version = "1.0", features = [
  "lock-order-graph",
  "stress-test",
  "logging-and-visualization",
] }
```

Feature costs and intended environments are documented in
[Choosing a Mode](https://emivvvvv.github.io/deloxide/choosing-a-mode.html).

## C integration

Deloxide also builds a C-compatible static/dynamic library:

```c
#include "deloxide.h"

int main(void) {
    if (deloxide_init(NULL, NULL) != 0) {
        return 1;
    }

    void *lock = deloxide_create_mutex();
    if (lock == NULL || deloxide_lock_mutex(lock) != 0) {
        return 1;
    }

    deloxide_unlock_mutex(lock);
    deloxide_destroy_mutex(lock);
    return 0;
}
```

See the [C Guide](https://emivvvvv.github.io/deloxide/c-guide.html) for lifecycle,
linking, callbacks, and platform notes.

## How the optimized detector works

The stable detector fact is that a thread is waiting for a lock in a particular
mode. Deloxide derives direct `Thread → Thread` edges from the lock's current
incompatible owners. If ownership transfers while the lock has waiters, affected
edges are refreshed rather than remaining tied to an old sampled owner.

Before a wait-for report is dispatched, every edge in the candidate cycle is
validated against current ownership. Shared RwLock reads are never treated as proof
that cycle participants are mutually exclusive.

The full protocol—including contention registration, handoff, graph traversal,
memory ordering, RwLock recursion, Condvar behavior, and complexity—is documented in
[How It Works](https://emivvvvv.github.io/deloxide/how-it-works.html).

## Performance

Performance changes are checked with a deliberately small Criterion harness:

- uncontended Mutex lock/unlock;
- uncontended RwLock read/write; and
- a two-thread Mutex handoff.

Candidate changes are compared with release commit `3b28ace` using the same toolchain,
feature set, profile, and machine. Results are repeated when the movement exceeds
normal run-to-run noise. Both percentages and absolute nanoseconds are reported.

See the [methodology](docs/performance/microbench-methodology.md). The full evaluation
suite is kept separate from these fast implementation checkpoints.

## Operational boundaries

- Only synchronization performed through tracked Deloxide primitives is visible.
- Active wait-for reports and potential lock-order reports have different certainty.
- Blocking read-to-write RwLock upgrades are self-deadlocks; release the read guard
  before acquiring a writer.
- Optional logging adds queue, serialization, and file-I/O costs.
- A callback runs asynchronously and should remain bounded.
- Application-level benchmarking with the intended feature set remains necessary.

These boundaries are part of using the product correctly, not footnotes. The
[production guide](https://emivvvvv.github.io/deloxide/production.html) provides
deployment and troubleshooting guidance.

## Explore

- [User manual](https://emivvvvv.github.io/deloxide/)
- [Detailed internals](https://emivvvvv.github.io/deloxide/how-it-works.html)
- [Rust API](https://docs.rs/deloxide)
- [C header](include/deloxide.h)
- [Examples](examples)
- [Performance methodology](docs/performance/microbench-methodology.md)

## Development

Focused checks:

```sh
cargo fmt --all -- --check
cargo clippy --lib --bins --examples --all-features -- -D warnings
cargo test --lib
cargo check --examples
scripts/check_docs.sh
```

Fast-path changes should also run the focused microbenchmarks. The complete
evaluation suite is not required for ordinary documentation or isolated bug fixes.

## License

Deloxide is available under the terms of the
[MIT License](LICENSE-MIT) or [Apache License 2.0](LICENSE-APACHE).

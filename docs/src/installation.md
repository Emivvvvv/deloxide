# Installation

Deloxide's default build includes active deadlock detection:

```toml
[dependencies]
deloxide = "1.1"
```

Then initialize it once, before creating the locks and threads you want to
observe:

```rust,no_run
# extern crate deloxide;
use deloxide::Deloxide;

Deloxide::new()
    .callback(|report| eprintln!("{report:#?}"))
    .start()
    .expect("start Deloxide");
```

Replace the synchronization on the suspicious path with Deloxide's `Mutex`,
`RwLock`, and `Condvar`. Their guards behave like familiar Rust lock guards.
Using `deloxide::thread` also records thread lifecycle information.

Optional Cargo features add deeper investigation tools:

```toml
[dependencies]
deloxide = { version = "1.1", features = [
  "logging-and-visualization",
  "lock-order-graph",
  "stress-test",
] }
```

Start with the default build. Add logging when you need a timeline, lock-order
analysis when you want to find risky acquisition patterns, and stress testing
when a bug rarely reproduces. [Choosing a mode](choosing-a-mode.md) explains
the difference.

Rust is the primary interface. C projects can build Deloxide as a library and
use [`include/deloxide.h`](../../include/deloxide.h); see
[C integration](c-guide.md).

For exact methods and types, use the [Rust API documentation](https://docs.rs/deloxide).

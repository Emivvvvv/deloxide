# Rust integration

Deloxide is designed for incremental adoption. Initialize it once, then replace
the locks on the path you want to diagnose.

| Standard type | Deloxide type | Main difference |
| --- | --- | --- |
| `std::sync::Mutex<T>` | `deloxide::Mutex<T>` | `lock()` returns the guard directly; poisoning is not used. |
| `std::sync::RwLock<T>` | `deloxide::RwLock<T>` | `read()` and `write()` return guards directly. |
| `std::sync::Condvar` | `deloxide::Condvar` | Wait methods mutate the supplied Deloxide mutex guard. |
| `std::thread` helpers | `deloxide::thread` | Adds tracked lifecycle events while preserving familiar spawn APIs. |

```rust,no_run
# extern crate deloxide;
use deloxide::{Deloxide, Mutex, thread};
use std::sync::Arc;

Deloxide::new()
    .callback(|report| eprintln!("{report:#?}"))
    .start()
    .expect("start Deloxide");

let counter = Arc::new(Mutex::new(0));
let worker_counter = Arc::clone(&counter);

let worker = thread::spawn(move || {
    *worker_counter.lock() += 1;
});

worker.join().unwrap();
```

`MutexGuard`, `RwLockReadGuard`, and `RwLockWriteGuard` release their locks when
dropped. Keep guard lifetimes short and avoid calling unknown code while holding
more than one lock.

## Callbacks and lifecycle

The callback runs on Deloxide's dispatcher thread. Keep it quick: write a small
record or use a nonblocking handoff to your telemetry/incident worker. Do not do
slow network work or acquire application locks inside it.

Deloxide configuration is process-wide. Initialize the intended configuration
once at startup. Repeated `start()` calls are not a clean reset and cannot
replace the first installed callback or logger; use a fresh process when tests
need isolated configurations.

The default callback panics when it receives a finding. Production applications
should usually install an explicit callback.

## Where to find exact APIs

This manual explains how to use Deloxide rather than repeating every signature.
Use [docs.rs](https://docs.rs/deloxide) for constructors, methods, trait
implementations, and feature-gated APIs. The repository also includes focused
[examples](../../examples).

# Your first diagnosis

This example deliberately creates the classic two-lock cycle.

## 1. Start the detector

```rust,no_run
# extern crate deloxide;
use deloxide::{DeadlockInfo, Deloxide};

Deloxide::new()
    .callback(|report: DeadlockInfo| {
        eprintln!("source: {:?}", report.source);
        eprintln!("threads: {:?}", report.thread_cycle);
        eprintln!("waited locks: {:?}", report.thread_waiting_for_locks);
    })
    .start()
    .expect("start Deloxide");
```

Initialize Deloxide before creating the tracked locks and worker threads.

## 2. Create opposite lock order

```rust,no_run
# extern crate deloxide;
# use deloxide::{Mutex, thread};
# use std::sync::{Arc, Barrier};
# let left = Arc::new(Mutex::new(()));
# let right = Arc::new(Mutex::new(()));
# let barrier = Arc::new(Barrier::new(2));
# let left_a = Arc::clone(&left);
# let right_a = Arc::clone(&right);
# let barrier_a = Arc::clone(&barrier);
# let left_b = Arc::clone(&left);
# let right_b = Arc::clone(&right);
# let barrier_b = Arc::clone(&barrier);
let first = thread::spawn(move || {
    let _left = left_a.lock();
    barrier_a.wait();
    let _right = right_a.lock();
});

let second = thread::spawn(move || {
    let _right = right_b.lock();
    barrier_b.wait();
    let _left = left_b.lock();
});
# let _ = (first, second);
```

The barrier makes both threads keep their first lock before requesting the
second. Deloxide sees the active cycle and calls the callback.

## 3. Read the result

```text
source: WaitForGraph
threads: [ThreadId(2), ThreadId(3)]
waited locks: [(ThreadId(2), LockId(2)), (ThreadId(3), LockId(1))]
```

Fix the program by choosing one lock order and using it on every path.

Run the complete example:

```console
cargo run --example diagnose_deadlock
```

Source: [`examples/diagnose_deadlock.rs`](../../examples/diagnose_deadlock.rs).
Continue with [Reading a report](diagnosis.md) for self-deadlocks, `RwLock`,
condition variables, and missing evidence.

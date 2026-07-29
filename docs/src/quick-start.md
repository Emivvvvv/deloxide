# Quick Start

Initialize Deloxide once, then use its synchronization wrappers:

```rust
use deloxide::{Deloxide, Mutex};

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
```

The complete compile-tested version is
[`examples/basic_mutex.rs`](https://github.com/Emivvvvv/deloxide/blob/main/examples/basic_mutex.rs).
Initialization is process-wide; configure the callback before starting worker
threads.

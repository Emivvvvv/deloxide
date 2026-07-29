# Manage Lifecycle and Callbacks

Initialize Deloxide once, at the process boundary, **before** instrumented
threads begin work and before the locks whose behavior you want to diagnose are
created. Configuration is process-wide; it is not a per-request, per-test-case,
or per-worker service.

```rust,no_run
# extern crate deloxide;
use deloxide::{DeadlockSource, Deloxide};

Deloxide::new()
    .callback(|report| match report.source {
        DeadlockSource::WaitForGraph => {
            eprintln!("active tracked deadlock: {:?}", report.thread_cycle);
        }
        DeadlockSource::LockOrderViolation => {
            eprintln!("potential lock-order cycle: {:?}", report.lock_order_cycle);
        }
    })
    .start()
    .expect("Deloxide must start before the workload");

// Construct tracked locks and start worker/request processing after this point.
```

`WaitForGraph` means an active, validated cycle among the detector's currently
tracked waits and incompatible owners. `LockOrderViolation`, available with the
optional order-graph feature, is a potential ordering risk rather than an active
deadlock. See [Reading a Deadlock Report](../diagnosis/reports.md) for the
triage workflow and the exact [`Deloxide`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html)
and [`DeadlockSource`](https://docs.rs/deloxide/1.1.0/deloxide/enum.DeadlockSource.html)
APIs.

## One detector, first configuration

Deloxide keeps a global detector for the process. Calling
[`Deloxide::start`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.start)
does not create an isolated detector or clear prior graph state. The callback and,
when logging is compiled, the global logger are one-time initializations: the
first successfully installed callback and logger are the ones used by later
reports. Treat the first startup configuration as authoritative.

Do not call `start()` again to change a callback, replace a log file, reset a
lock-order history, or create a clean detector for a new tenant. Later starts are
not a supported reconfiguration mechanism; in particular, they do not undo
previous detector state. There is deliberately no public shutdown, reset, or
reconfigure API in this guidance.

This matters in tests. Put cases requiring different Deloxide configurations in
separate test processes (for example, separate integration-test binaries or
separate `cargo test --test name` invocations), rather than parallel tests in one
process. A test that installs the default callback can affect every later test in
that binary.

The default callback panics with the report, but callback execution is isolated;
choose an explicit callback for application policy instead of assuming that
default behavior terminates the process.

## Use tracked thread entry points

[`deloxide::thread`](https://docs.rs/deloxide/1.1.0/deloxide/thread/index.html)
re-exports common `std::thread` items such as `JoinHandle`, `current`, `sleep`,
`park`, and `yield_now`. It provides tracked versions of:

- [`thread::spawn`](https://docs.rs/deloxide/1.1.0/deloxide/thread/fn.spawn.html),
- [`thread::Builder`](https://docs.rs/deloxide/1.1.0/deloxide/thread/struct.Builder.html)
  with `spawn`, and
- [`Builder::spawn_scoped`](https://docs.rs/deloxide/1.1.0/deloxide/thread/struct.Builder.html#method.spawn_scoped).

These helpers register thread creation and exit. On creation they retain the
parent's Deloxide thread ID; with logging enabled, that parent/child information
is emitted with the thread-spawn event so a log can relate a worker to its
creator.

```rust,no_run
# extern crate deloxide;
use deloxide::thread;

let named = thread::Builder::new()
    .name("indexer".to_owned())
    .spawn(|| 42)
    .expect("worker creation");
assert_eq!(named.join().expect("worker result"), 42);

let value = 0;
thread::scope(|scope| {
    // `scope.spawn` is std's scoped spawn; use the tracked Builder helper here.
    thread::Builder::new()
        .spawn_scoped(scope, || assert_eq!(value, 0))
        .expect("scoped worker creation")
        .join()
        .expect("scoped worker result");
});
```

[`thread::scope`](https://docs.rs/deloxide/1.1.0/deloxide/thread/fn.scope.html)
provides the standard scoped-thread boundary, but its `Scope` is the standard
library type. Therefore `scope.spawn(...)` is an ordinary scoped spawn; use
`thread::Builder::spawn_scoped(scope, ...)` when creation/exit tracking matters.

Ordinary `std::thread::spawn` does not make the detector blind to every operation
inside that thread: a standard thread that locks a Deloxide `Mutex`, `RwLock`, or
uses a compatible Deloxide `Condvar` still runs the wrapper code, so supported
lock waits and acquisitions can be observed. What it lacks is the tracked
thread's spawn/exit registration and parent/child log relationship. Ordinary
threads also do not make `std::sync` or `parking_lot` locks observable; migrate
those lock instances separately.

## Keep callbacks an alert handoff

Deloxide queues callbacks to one background dispatcher thread rather than running
them on the thread that detected the finding. A panic from one callback invocation
is caught, reported to stderr, and does not stop that dispatcher from handling a
later report. That isolation is useful, but it is not permission to do recovery
work in the callback: callbacks are serialized, and a callback can still block on
an application lock, do slow I/O, or delay every subsequent report.

Keep the handler bounded and nonblocking. Copy or move the
[`DeadlockInfo`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html)
into a bounded queue with `try_send`, count overload, and let a separate
supervisor persist, page, or capture diagnostics.

```rust,no_run
# extern crate deloxide;
use deloxide::{DeadlockInfo, Deloxide};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};

let (reports_tx, reports_rx) = mpsc::sync_channel::<DeadlockInfo>(64);
let dropped = Arc::new(AtomicU64::new(0));
let callback_dropped = Arc::clone(&dropped);

Deloxide::new()
    .callback(move |report| {
        if reports_tx.try_send(report).is_err() {
            callback_dropped.fetch_add(1, Ordering::Relaxed);
        }
    })
    .start()
    .expect("detector initialization");

let _supervisor_inputs = (reports_rx, dropped);
```

The process may exit, abort, or be terminated before a queued callback, a log
write, or the supervising task completes. Do not make process termination from a
callback your evidence-preservation strategy; persist or export what you need on
the normal incident path, and test that path independently.

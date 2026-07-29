# Adopt the Tracked Primitives

Deloxide sees synchronization only when the code uses its tracked wrappers. Start
by replacing the locks on the path you are investigating, initialize the detector
before that path runs, and expand from there. The wrappers keep the familiar
guard-based style while reporting supported lock activity to the detector.

```rust,no_run
# extern crate deloxide;
use deloxide::{Condvar, Deloxide, Mutex, RwLock};

Deloxide::new()
    .callback(|report| eprintln!("deadlock report: {report:?}"))
    .start()
    .expect("detector initialization");

let counter = Mutex::new(0);
*counter.lock() += 1;

let settings = RwLock::new(String::from("ready"));
assert_eq!(settings.read().as_str(), "ready");
settings.write().push_str(" for work");

let ready = Condvar::new();
let _ = ready;
```

For complete item documentation, see [`Mutex`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Mutex.html),
[`RwLock`](https://docs.rs/deloxide/1.1.0/deloxide/struct.RwLock.html),
[`Condvar`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Condvar.html), and
[`Deloxide::start`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.start).

## Replace imports, not the locking model

For selected code, replace either family of imports with Deloxide's types:

```text
// Before: use std::sync::{Condvar, Mutex, RwLock};
// Before: use parking_lot::{Condvar, Mutex, RwLock};
use deloxide::{Condvar, Mutex, RwLock};
```

This is an import diff rather than a Rust example. The runnable forms are the repository's
[`basic_mutex`](https://github.com/Emivvvvv/deloxide/blob/main/examples/basic_mutex.rs),
[`rwlock`](https://github.com/Emivvvvv/deloxide/blob/main/examples/rwlock.rs), and
[`condvar`](https://github.com/Emivvvvv/deloxide/blob/main/examples/condvar.rs)
examples.

`Mutex::lock`, `RwLock::read`, and `RwLock::write` return guards directly. A
guard dereferences to its protected value, and dropping it always releases the
physical lock. The wrapper also reports the release globally when active tracking
or logging requires it; uncontended fast paths avoid unnecessary global detector
work. Keep the usual narrow scopes and explicit `drop(guard)` where the release
point matters. The wrappers use `parking_lot` internally; they do not expose
`std::sync` poisoning or `LockResult`/`PoisonError`. In particular, remove
`.unwrap()` or poisoned-lock recovery that existed only to handle the standard
library result:

```rust,no_run
# extern crate deloxide;
use deloxide::Mutex;

let jobs = Mutex::new(Vec::<String>::new());
jobs.lock().push("index".to_owned()); // No LockResult to unwrap.
assert_eq!(jobs.lock().len(), 1);
```

That is a semantic migration, not merely a type alias: code that relies on
poisoning as an application health signal needs its own explicit failure state.

## Use each wrapper with its matching guard

`Mutex` is for exclusive access. `RwLock` has distinct read and write guards:
multiple reads may coexist, while a write is exclusive. A Deloxide `Condvar`
waits with a mutable **Deloxide `MutexGuard`**; do not mix it with a
`std::sync::Mutex` or `parking_lot::Mutex` guard. Its wait methods release the
associated mutex while waiting and reacquire it before returning.

```rust,no_run
# extern crate deloxide;
use deloxide::{Condvar, Mutex};
use std::sync::Arc;

let state = Arc::new((Mutex::new(false), Condvar::new()));
let (lock, wake) = &*state;
let mut started = lock.lock();
while !*started {
    // `wait` returns with `started` holding the same tracked mutex again.
    wake.wait(&mut started);
}
```

Use the normal predicate loop: wakeups are not a reason to assume the predicate
is true. The wrapper also offers [`Condvar::wait_timeout`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Condvar.html#method.wait_timeout),
[`wait_while`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Condvar.html#method.wait_while),
and [`wait_timeout_while`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Condvar.html#method.wait_timeout_while).
Unlike `std::sync::Condvar`, these methods mutate the supplied guard in place:
`wait_timeout` and `wait_timeout_while` return `bool` (`true` means the timeout
elapsed), rather than returning a guard/result pair.

There is no read-to-write upgrade method. Release a read guard before taking a
write guard; attempting a blocking write while retaining a read guard can
self-deadlock.

```rust,no_run
# extern crate deloxide;
use deloxide::RwLock;

let cache = RwLock::new(vec![1, 2]);
{
    let read = cache.read();
    assert_eq!(read.len(), 2);
} // The read guard is gone before the write attempt.
cache.write().push(3);
```

## Nonblocking probes

[`Mutex::try_lock`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Mutex.html#method.try_lock),
[`RwLock::try_read`](https://docs.rs/deloxide/1.1.0/deloxide/struct.RwLock.html#method.try_read),
and [`RwLock::try_write`](https://docs.rs/deloxide/1.1.0/deloxide/struct.RwLock.html#method.try_write)
are nonblocking. They return `Option<Guard>`: `Some` owns the acquired guard and
`None` means that attempt could not acquire the lock immediately. They do not
return a `TryLockError` and must not be treated as an eventual wait.

```rust,no_run
# extern crate deloxide;
use deloxide::{Mutex, RwLock};

let mutex = Mutex::new(1);
if let Some(mut value) = mutex.try_lock() {
    *value += 1;
}

let config = RwLock::new(10);
let snapshot = config.try_read().map(|value| *value);
if let Some(mut value) = config.try_write() {
    *value += 1;
}
assert!(snapshot.is_some());
```

## Roll out without overstating coverage

Begin with the locks shared by the suspected operations, convert every endpoint
of that dependency, and reproduce the scenario. Then migrate adjacent lock
families and worker entry points. A useful rollout order is:

1. Initialize Deloxide before the instrumented workload.
2. Convert the locks and condition variables in one coherent operation.
3. Convert the threads that create that operation's workers to
   [`deloxide::thread`](https://docs.rs/deloxide/1.1.0/deloxide/thread/index.html).
4. Exercise the path and use reports to guide the next boundary.

An untracked boundary is a visibility boundary. If a participant holds a
standard/`parking_lot` lock, waits on a different synchronization primitive, or
uses a condition variable paired with an untracked mutex, Deloxide cannot form
all of that dependency's edges. A report is evidence about the tracked
primitives, not proof that the rest of the process is free of deadlocks. Keep
the original primitives where migration is not yet safe, but document the gap
and avoid interpreting the mixed deployment as complete coverage.

For active findings, distinguish
[`WaitForGraph`](https://docs.rs/deloxide/1.1.0/deloxide/enum.DeadlockSource.html#variant.WaitForGraph)
(a current, validated cycle) from
[`LockOrderViolation`](https://docs.rs/deloxide/1.1.0/deloxide/enum.DeadlockSource.html#variant.LockOrderViolation)
(a potential historical order cycle). Continue with [lifecycle and callback
guidance](lifecycle.md) before enabling the detector in a long-running process.

# First Diagnosis

This tutorial turns a deliberate two-lock deadlock into a report you can read. The
same workflow applies to a real reproduction: initialize once, use tracked
primitives on the suspected path, and keep the callback small enough to preserve the
evidence rather than becoming another source of delay.

## 1. Add Deloxide

```toml
[dependencies]
deloxide = "1.1"
```

The default feature set is the active wait-for detector. It does not enable the
optional logger, lock-order graph, or stress scheduler.

## 2. Start the detector before the workload

The canonical example configures the detector and its callback before it creates
the locks or starts worker threads. The callback only prints the report and exits,
which keeps the teaching program bounded.

```rust,no_run,ignore
use deloxide::{DeadlockInfo, DeadlockSource, Deloxide, Mutex, thread};
use std::sync::{Arc, Barrier};

fn main() {
{{#include ../../examples/diagnose_deadlock.rs:setup}}
{{#include ../../examples/diagnose_deadlock.rs:cycle}}

    thread::park();
}
```

The `setup` and `cycle` excerpts above are taken from
[`examples/diagnose_deadlock.rs`](https://github.com/Emivvvvv/deloxide/blob/main/examples/diagnose_deadlock.rs),
which is the compile-tested source for this intentionally deadlocking example.
The `no_run` marker is important: running this exact program is supposed to block
until Deloxide reports the cycle. The canonical example is compile-checked by Cargo;
the rendered excerpt is ignored by `mdbook test` because the book test harness does
not link this crate's multi-artifact library target.

## 3. Reproduce the cycle

```console
cargo run --example diagnose_deadlock
```

The barrier makes each worker acquire its first lock before either attempts the
second. One thread holds `left` and waits for `right`; the other holds `right` and
waits for `left`. The detector sees the two directed waits and schedules the
callback. This example exits with status 0 because its callback deliberately calls
`std::process::exit(0)` after printing the active finding. That exit is a teaching
device, not normal recovery policy for an application.

## 4. Read the three fields first

The output contains the fields that anchor a first investigation:

- `source` tells you the evidence level. Here it is `WaitForGraph`: an active,
  validated cycle among currently tracked waits and incompatible owners. If an
  optional lock-order check reports `LockOrderViolation`, treat it as a potential
  acquisition-order risk instead; it does not say a live cycle was observed.
- `thread_cycle` is the ordered group of participating Deloxide thread IDs. Follow
  it around the cycle: each thread is waiting on a lock owned incompatibly by the
  next participant.
- `thread_waiting_for_locks` contains `(thread_id, lock_id)` pairs. Match every
  waiting thread to the lock it attempted, then map that lock back to the wrapper
  and acquisition site in your code. The pair identifies a wait; it is not a source
  location or a claim about untracked resources.

For field-by-field interpretation, see [Read a Deadlock Report](diagnosis/reports.md).
For the familiar two-mutex shape and other failure modes, see
[Recognize Common Patterns](diagnosis/patterns.md).

## 5. Fix the order in code

Deloxide reports evidence; it does not rewrite synchronization. For this example,
choose one order and enforce it everywhere: acquire `left` before `right` in both
workers (or centralize the operation so callers cannot choose conflicting orders).
Then rerun the original workload and its tests. Do not "fix" a report by suppressing
the callback or switching only one path back to an untracked lock—the result can
hide the dependency rather than remove it.

Once you have a report, you can preserve the event history with
[logging and visualization](visualization.md), probe a rare reproduction with
[stress testing](diagnosis/stress-testing.md), or check the API surface on
[docs.rs](https://docs.rs/deloxide).

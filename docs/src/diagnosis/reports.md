# Reading a Deadlock Report

The callback receives a [`DeadlockInfo`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html) value. Start with [`source`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html#structfield.source): it decides whether the report describes a blocked cycle now or an ordering risk observed earlier.

- [`DeadlockSource::WaitForGraph`](https://docs.rs/deloxide/1.1.0/deloxide/enum.DeadlockSource.html#variant.WaitForGraph) is an **active, validated wait-for cycle**. Treat it as an incident: the listed threads are blocked on incompatible owners in the detector's current snapshot.
- [`DeadlockSource::LockOrderViolation`](https://docs.rs/deloxide/1.1.0/deloxide/enum.DeadlockSource.html#variant.LockOrderViolation) is a **potential lock-order violation**. It says executions have established a circular ordering rule, not that those threads are currently stuck. Reproduce and decide whether the paths can overlap before escalating it as an outage.

## Field-by-field procedure

| Field | What it contains | How to use it |
| --- | --- | --- |
| [`source`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html#structfield.source) | The detector that emitted the finding. | Always inspect first; it determines the confidence and triage path. |
| [`thread_cycle`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html#structfield.thread_cycle) | Ordered thread IDs. For an active wait-for report, each thread waits on the next thread and the final thread waits on the first. For a lock-order report it is only the thread that completed the suspicious acquisition. | Use it to attach request IDs, worker names, and stack captures. It is primary evidence only for `WaitForGraph`. |
| [`thread_waiting_for_locks`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html#structfield.thread_waiting_for_locks) | `(thread_id, lock_id)` pairs for the attempted acquisition that led to the report. | For each cycle thread, locate that lock wrapper and confirm which guard is still live. This is the most direct route from IDs to source sites. |
| [`lock_order_cycle`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html#structfield.lock_order_cycle) | `None` for a wait-for report; the ordered lock cycle for a lock-order violation. | Source-specific evidence. Read `A, B, C, A` as “A was held before B, B before C, and C before A” across observed acquisitions. Do not read it as a set of currently blocked locks. |
| [`timestamp`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html#structfield.timestamp) | An ISO-8601 detection timestamp. | Correlate with request logs, deploys, and traces. It timestamps observation, not necessarily the first moment an application stopped making progress. |
| [`verification_request`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html#structfield.verification_request) | Optional `(lock_id, thread_id)` verification metadata. | Preserve it verbatim for tooling. Current normal report paths leave it `None`; do not require it to diagnose either source. |

The ordered cycle is deliberately directional. Given `thread_cycle: [101, 202]` and `thread_waiting_for_locks: [(101, 17), (202, 42)]`, investigate: thread 101 is waiting for 17, held incompatibly by 202; thread 202 is waiting for 42, held incompatibly by 101. The same list is not a claim that the IDs are sorted, or that every holder of a shared lock must be in the cycle.

## Example: active wait-for report

```text
source: WaitForGraph
thread_cycle: [101, 202]
thread_waiting_for_locks: [(101, 17), (202, 42)]
lock_order_cycle: None
timestamp: "2026-07-29T09:41:12.807Z"
verification_request: None
```

This is an active deadlock report. Capture the two thread stacks, then search the lock construction sites for IDs 17 and 42 in the optional event log. The likely shape is 101 holding 42 while requesting 17, and 202 holding 17 while requesting 42. Fix the overlapping critical sections or establish one acquisition order; see [two-Mutex inversion](patterns.md#two-mutex-inversion).

## Example: potential lock-order report

```text
source: LockOrderViolation
thread_cycle: [202]
thread_waiting_for_locks: [(202, 17)]
lock_order_cycle: Some([17, 42, 17])
timestamp: "2026-07-29T09:48:33.042Z"
verification_request: None
```

This is not proof that 202 is blocked. It means prior execution recorded 17 before 42, while this acquisition, holding 42 and requesting 17, closes the historical cycle. Inspect both paths, determine whether they run concurrently and share the same lock instances, then use the [lock-order workflow](lock-order.md). If the overlap is real, use [stress testing](stress-testing.md) to seek an active `WaitForGraph` report; absence of one in a finite run does not make the ordering safe.

## Keep the callback small

Register a handler with [`Deloxide::callback`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.callback), but treat it as an alert handoff, not a recovery transaction. Deloxide dispatches callbacks away from the detecting thread, yet a callback that waits on application locks, performs slow network I/O, or invokes a complex shutdown path can still delay later notifications or compound an incident. Copy or serialize `DeadlockInfo`, send it to a bounded incident queue or logger that does not use implicated locks, and return.

```rust,ignore
use deloxide::{DeadlockInfo, Deloxide};
use std::sync::mpsc;

let (reports_tx, reports_rx) = mpsc::channel::<DeadlockInfo>();
Deloxide::new()
    .callback(move |info| {
        let _ = reports_tx.send(info); // hand off; do not acquire application locks here
    })
    .start()
    .expect("detector initialization");

// A separate supervisory task can correlate, persist, or page on reports_rx.
let _ = reports_rx;
```

If logging is enabled, [`Deloxide::with_log`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.with_log) and [`showcase_this`](https://docs.rs/deloxide/1.1.0/deloxide/fn.showcase_this.html) can provide the event history. The structured callback payload remains the authoritative alert input; the visualization is supporting evidence.

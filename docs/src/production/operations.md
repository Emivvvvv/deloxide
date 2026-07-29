# Operate Deloxide in Production

Deloxide is most useful when its configuration, its coverage, and its incident
path are decided before an outage. Begin with the default active wait-for
detector, then add optional evidence features only after measuring their cost in
the workload that will run them. An active `WaitForGraph` report is evidence of a
validated cycle among supported, tracked synchronization; a
`LockOrderViolation` is a potential historical ordering risk and needs a
different response. See [Reading a Deadlock Report](../diagnosis/reports.md) for
the triage distinction.

## Roll out in stages

1. **Inventory the coverage boundary.** List every process, thread entry point,
   long-lived worker, and lock instance that matters to the service. Migrate
   those lock instances to Deloxide `Mutex`, `RwLock`, and `Condvar` wrappers.
   Prefer `deloxide::thread::spawn` or `deloxide::thread::Builder` where
   parent/child and exit events are useful. Record raw `std::sync`, raw
   `parking_lot`, third-party, and operating-system synchronization as explicit
   gaps; a cycle that crosses one is not fully visible to the detector.
2. **Canary the default build first.** Initialize one process-wide detector
   before creating tracked locks or accepting work. Install an explicit bounded
   callback, observe startup errors and report volume, and verify that normal
   lock behavior and service latency stay within the application's budget.
3. **Benchmark representative work before broad rollout.** Include production
   contention shapes, RwLock reader/writer ratios, request concurrency, CPU
   limits, and the exact Cargo feature set. The focused figures in
   [Measure Performance](performance.md) are a starting point, not capacity
   planning for another workload or machine.
4. **Add optional features one at a time.** Enable logging for a controlled
   incident capture, lock-order analysis in development or CI, and stress modes
   for focused reproductions. Measure each addition separately and again in the
   combined configuration if it will be deployed together.

`Deloxide::start()` configures global process state, not a worker-local detector.
The first installed callback and logger persist; later starts are partial
reconfiguration rather than a reliable reset. Use separate processes to compare
clean configurations or isolate test cases. The lifecycle chapter explains the
feature-specific repeated-start behavior in detail: [Manage Lifecycle and
Callbacks](../rust/lifecycle.md).

## Make the callback an alert handoff

Callbacks run on a single dispatcher outside the detector mutex. They are a poor
place to acquire an application lock, synchronously page an external service,
open a browser, or attempt an involved shutdown: any of those can delay later
reports or entangle an already blocked application. Transfer the report to a
bounded application-owned queue, count failed handoffs, and let a supervisor
perform slower enrichment, persistence, tracing, or paging.

```rust,no_run
# extern crate deloxide;
use deloxide::{DeadlockInfo, Deloxide};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};

let (reports_tx, reports_rx) = mpsc::sync_channel::<DeadlockInfo>(64);
let dropped_reports = Arc::new(AtomicU64::new(0));
let callback_drops = Arc::clone(&dropped_reports);

Deloxide::new()
    .callback(move |report| {
        if reports_tx.try_send(report).is_err() {
            callback_drops.fetch_add(1, Ordering::Relaxed);
        }
    })
    .start()
    .expect("initialize before instrumented work");

// A separate task owns reports_rx and may persist or page without an implicated lock.
let _incident_inputs = (reports_rx, dropped_reports);
```

This code deliberately drops a report when the queue is full or disconnected.
Choose queue capacity, overload telemetry, and supervisor durability for the
incident burst the application expects. A process can still abort or be killed
before the callback, logger, or supervisor finishes; test evidence preservation
on the normal incident path instead of treating a callback as a transaction.

## Capture, protect, and retain logs

The `logging-and-visualization` feature adds an asynchronous event logger. With
that feature compiled, `Deloxide::new().start()` selects `deloxide.log` unless
the builder uses `with_log`; the writer creates parent directories and truncates
an existing selected file. Give each process or incident a unique,
application-owned path that includes a PID, UUID, or another collision-proof
component. The built-in `{timestamp}` substitution has one-second precision, so
`logs/deloxide_{timestamp}.log` reduces collisions but is not sufficient to make
concurrent process paths unique. Apply permissions, encryption, storage limits,
retention, and access review as you would to any incident record. Do not let
multiple processes reuse one filename.

The current optional logger has an **unbounded ordinary-event queue**. The
[correctness hardening report](../../correctness-hardening-report.md) records
that choosing a bounded drop, block, or coalescing policy still needs a separate
saturation design and test. Accordingly, treat logging as incident evidence with
memory and I/O capacity to monitor, rather than as a lossless audit trail with a
fixed resource bound.

When an incident requires the timeline, flush and open the process's current log
from a controlled supervisor path with `showcase_this`, or use `showcase` for a
retained file. The viewer encodes the local log in a URL and opens
`https://deloxide.vercel.app/` in the default browser. Review the log for
identifiers or sensitive context before sending it to that browser destination;
this workflow does not provide a local-only guarantee. Keep the structured
callback report as the primary alert input and the visualization as supporting
reconstruction evidence. [Logging and Visualization](../visualization.md)
describes the file format and failure cases.

## Roll back by feature and preserve the evidence boundary

Cargo features are compile-time choices, so a rollback normally means deploying
a binary built without the optional feature, not flipping a runtime switch in a
running process.

| Observation during rollout | Narrow rollback | What remains |
| --- | --- | --- |
| Log volume, file I/O, or queue growth is unsuitable | Deploy a build without `logging-and-visualization`, or use `no_logging()` before the first successful logger installation. | The default active detector and callback remain available. |
| Historical order warnings create more work than the team can triage | Deploy without `lock-order-graph`, or use `no_lock_order_checking()` for the initial start. | Active `WaitForGraph` reporting remains; do not reinterpret its evidence as an order-graph result. |
| Test perturbation changes timing too much | Deploy without `stress-test` and remove stress-builder selection. | The normal active detector path remains. |
| Base detector cost or behavior fails the service's own acceptance criteria | Roll back the Deloxide integration or confine it to the reproduction environment. | Existing application observability must cover the incident instead. |

Do not use a second `start()` call as an in-process rollback. It does not clear
existing ownership and wait state coherently, cannot replace the first callback
or logger, and has feature-specific side effects.

## Deployment checklist

- [ ] The process initializes exactly one intended Deloxide configuration before
      instrumented work begins.
- [ ] The coverage inventory names tracked lock instances, thread entry points,
      and every known untracked synchronization boundary.
- [ ] The callback only performs bounded, nonblocking handoff; its queue loss or
      disconnection metric is monitored.
- [ ] The service has separate response playbooks for active `WaitForGraph` and
      potential `LockOrderViolation` reports.
- [ ] A representative benchmark covers the deployed features, contention
      pattern, resource limits, and roll-back threshold.
- [ ] Logging paths are unique per process/incident, writable, access-controlled,
      retained for an agreed period, and sized with the unbounded queue caveat in
      mind.
- [ ] The visualization export/privacy review is part of the incident procedure.
- [ ] Lock-order and stress features are enabled only in the environments where
      their historical-analysis or schedule-perturbation costs are intended.
- [ ] A fresh process/binary is available for each rollback configuration; no
      response depends on repeated `start()` calls.

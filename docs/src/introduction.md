# Introduction

Deloxide is a runtime deadlock detection and diagnosis toolkit for Rust, with a
secondary C interface. It turns a hanging tracked workload into a concrete
thread-and-lock cycle, then gives you tools to reproduce, understand, and prevent
the same failure.

The default detector follows waits between threads using Deloxide's `Mutex`,
`RwLock`, and `Condvar`. When the current waits form a cycle, the callback receives
the participating thread IDs and the lock each thread is trying to acquire.

```text
ThreadId(2) waits for LockId(7), owned by ThreadId(3)
ThreadId(3) waits for LockId(4), owned by ThreadId(2)
```

That is a `WaitForGraph` report: an active cycle observed among tracked
synchronization. Deloxide also provides:

- an optional lock-order graph that finds risky acquisition patterns before they
  become an active deadlock;
- random and component-based stress modes that make rare schedules easier to
  reproduce;
- structured callbacks for application telemetry and incident handling;
- asynchronous event logging and an interactive visualization; and
- C bindings for the same tracked primitives.

## One workflow from development to production

Deloxide is designed to remain useful through the whole investigation:

1. Replace the locks around the suspicious path.
2. Reproduce the hang and receive an active cycle.
3. Add visualization when the IDs alone are not enough.
4. Use lock-order analysis to find the inversion earlier.
5. Use stress modes when the schedule rarely manifests.
6. Keep the default detector in production when the measured cost fits the
   application.

The Optimistic Fast Path keeps eligible uncontended Mutex and exclusive RwLock
operations away from global graph work. The broader evaluation, methodology, and
comparisons are described in the
[Deloxide preprint](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=6389109)
and the [performance chapter](production/performance.md).

## The observation boundary

Deloxide sees synchronization performed through its wrappers. It cannot build a
complete cycle through raw locks, channels, I/O, another process, or a remote
service. That is why incremental adoption should cover every lock on the
suspected cycle, not only the line where the final thread happened to block.

This manual complements [docs.rs](https://docs.rs/deloxide). It explains the
workflow, feature choices, evidence, examples, C integration, and production
trade-offs. Use the API documentation for exact signatures and trait details.

Continue with [Installation](installation.md), then run
[Your first diagnosis](getting-started.md).

# Understand Coverage and Limitations

Deloxide reports what its wrappers can observe. It is designed to distinguish
stronger active wait evidence from earlier, potential ordering evidence; it is
not a general progress, race, or distributed-deadlock monitor.

## Two report sources, two confidence levels

`DeadlockSource::WaitForGraph` is an **active, validated wait-for cycle**. Before
dispatch, the detector checks every candidate edge against a current mode-aware
wait intent and current incompatible owners. If an edge is stale, it repairs the
affected waiters and validates any replacement candidate before creating the
report. For supported, tracked primitives represented in that detector state,
this is the incident-grade evidence to preserve and investigate.

`DeadlockSource::LockOrderViolation` comes from the optional historical
lock-order graph. It says observed acquisitions formed a `Lock -> Lock` order
cycle; it does not say threads are blocked at report time and does not pass
through the active wait-cycle validation path. Treat it as a potential risk:
inspect whether the same lock instances and paths can overlap, then reproduce
under a feasible schedule. [Read a Deadlock Report](../diagnosis/reports.md)
gives the response procedure for both sources.

## Coverage starts at the wrapper boundary

Deloxide follows synchronization performed through its `Mutex`, `RwLock`, and
`Condvar` wrappers, including the C API that reaches those wrappers. Ordinary
threads can still produce supported lock events when they use a Deloxide lock,
but tracked `deloxide::thread` entry points add spawn/exit registration and
parent/child logging context.

The detector cannot construct dependencies that never enter those wrappers. This
includes raw `std::sync` and `parking_lot` locks, third-party primitives, custom
atomics or spin locks, I/O waits, channels, processes, and remote resources. A
deadlock that crosses one of those boundaries may be partial or absent from the
wait-for graph. Keep a coverage inventory and use other observability for the
untracked portions; [Operate Deloxide in Production](operations.md) has a rollout
checklist.

## RwLock and Condvar semantics matter

Several threads may hold a shared `RwLock` read lock at once. Shared readers are
not mutually exclusive, so they are excluded from the common-lock proof used by
active-cycle validation. A read wait conflicts with the current writer; a write
wait can conflict with a writer or each counted reader. A thread performing a
blocking read-to-write upgrade remains an incompatible reader and can form a
validated self-cycle. There is no implicit upgrade operation.

`Condvar::wait` tracks the release, wait, and physical reacquisition of its
Deloxide `Mutex`. Notification does not make the notifying thread a synthetic
mutex owner. A timeout follows physical reacquisition and cleanup, but a pure
timeout can block while reacquiring the mutex without a corresponding synthetic
wait intent unless notification also selected the waiter. That is a coverage
boundary, not proof of an application failure. See [Tracked Primitive
Semantics](../internals/primitives.md) for the mode and timeout details.

## Timing, validation, and snapshot boundaries

The detector dispatches a report after validation and outside its detector mutex.
The timestamp records observation, not necessarily when the application first
stopped making progress. Callbacks are serialized on a dispatcher thread; a slow
callback can delay later notifications, and process termination can occur before
a callback, log write, or supervisor action completes. Use a bounded handoff and
capture incident context separately.

Validation protects against several stale-edge shapes: a wait retains its stable
lock-and-mode intent, affected edges are refreshed on ownership handoff, every
candidate edge is re-resolved against current incompatible owners, and graph
maintenance continues beyond the first candidate across affected owners and
waiters. Those controls reject a sampled cycle whose dependency no longer holds;
they do not establish a universal false-positive absence claim. The full protocol
and its known scope are in [Validate Active Cycles](../internals/validation.md).

There remains a formal snapshot boundary. Per-lock owner hints and slow-waiter
state use separate atomics, rather than one packed owner/contention state, so the
implementation does not provide a single linearizable owner/waiter snapshot. The
detector-held physical recheck, owner publication, and affected-waiter repair
closed the reproduced stale-handoff failures, but the [correctness hardening
report](../../correctness-hardening-report.md) retains packed state as deferred
work. Acquisition generations are also not used; a release and later
reacquisition by the same thread can be ABA-shaped from the detector's point of
view even though no deterministic failure requiring generations was reproduced.

## Non-goals

Deloxide does not aim to:

- prove the absence of deadlocks in an arbitrary process or deployment;
- detect general data races, atomic-ordering bugs, starvation, livelock, or every
  form of application hang;
- infer dependencies through untracked locks, channels, filesystem or network
  I/O, processes, or remote services;
- turn a historical lock-order cycle into proof of an active outage; or
- provide distributed deadlock detection across process or machine boundaries.

Use active WFG reports as scoped evidence for the tracked graph, treat order
reports as hypotheses to test, and keep the synchronization and operational
boundaries visible in incident reviews.

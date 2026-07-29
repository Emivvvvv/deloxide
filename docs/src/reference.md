# Reference

Deloxide observes synchronization performed through its tracked Rust wrappers or
C ABI objects. It reports either a cycle in the current wait-for state or, when
the optional Rust lock-order feature is enabled, a historical ordering risk. It
does not observe native or third-party primitives that bypass those interfaces.

## Terminology

| Term | Meaning |
| --- | --- |
| Tracked primitive | A Deloxide `Mutex`, `RwLock`, or `Condvar` (or its C ABI counterpart) whose operations feed the detector. |
| Active wait | A thread is currently trying to acquire a tracked lock held incompatibly by another tracked thread. |
| Wait-for graph | Directed current-state dependencies from a waiting thread to incompatible lock owner(s). |
| Active cycle | A closed, current wait-for cycle, reported as `WaitForGraph`. |
| Lock-order graph | Optional history of observed `held lock → requested lock` relationships. |
| Potential cycle | A circular lock-order pattern, reported as `LockOrderViolation`; it may never block at runtime. |
| Callback | The application handler that receives a `DeadlockInfo` report. It is an alert handoff, not a recovery transaction. |

## Features

| Feature | Default build | What it adds | C integration |
| --- | --- | --- | --- |
| Active wait-for detection | Enabled | Current ownership/wait tracking and validated active-cycle reports. | Available through tracked C primitives. |
| `logging-and-visualization` | Disabled | Structured event logging and browser visualization. | Build the library with the feature; initialize C with a log path and use the logging FFI only from that build. |
| `lock-order-graph` | Disabled | Potential circular acquisition-order reports. | Rust configuration only: the current C initializer disables lock-order checking. |
| `stress-test` | Disabled | Random or component-based scheduling disturbance for reproduction. | C stress controls must be called before initialization; they return `-1` when the feature is absent. |

The default configuration is therefore the active detector without optional logging,
historical lock-order checking, or stress disturbance. Feature selection changes
coverage and operational cost; it does not turn a potential report into proof of
an active deadlock.

## Deadlock report schema

### `DeadlockSource` certainty

| Source | What was observed | Certainty and response |
| --- | --- | --- |
| `WaitForGraph` | A cycle validated against the detector's current wait and ownership state. | Active deadlock among tracked operations. Capture the report and relevant thread stacks as an incident. |
| `LockOrderViolation` | A circular acquisition-order pattern observed across executions. | Potential deadlock only. Review whether the paths overlap and can run concurrently; use stress/reproduction to seek an active cycle. |

Never describe `LockOrderViolation` as an active deadlock. Conversely, an active
`WaitForGraph` report is not merely a lock-order warning.

### `DeadlockInfo` fields

| Field | Contents | Interpretation |
| --- | --- | --- |
| `source` | `WaitForGraph` or `LockOrderViolation`. | Read first; it determines the report's certainty. |
| `thread_cycle` | Ordered Deloxide thread IDs. | For an active report, each thread waits through the cycle to the next. A potential report can name only the reporting thread. |
| `thread_waiting_for_locks` | `(thread_id, lock_id)` pairs. | The attempted lock acquisitions that produced the report; correlate IDs with application context. |
| `lock_order_cycle` | Optional ordered lock-ID cycle. | Present for an ordering violation and absent for an active wait-for report; it is historical ordering evidence, not a list of blocked locks. |
| `timestamp` | ISO-8601 detection time. | Correlate with deploys, request logs, and stack captures. |
| `verification_request` | Optional `(lock_id, thread_id)` metadata. | Preserve for tooling; normal report paths leave it `None`. |

The Rust callback receives this structure directly. The C callback receives its
JSON serialization as a borrowed NUL-terminated string that must be copied, not
freed or retained; see the [C example](c-guide.md#smallest-complete-program).

## Defaults and lifecycle

| Setting | Default | Notes |
| --- | --- | --- |
| Initialization | One process-global detector | Rust `start()` and C `deloxide_init` are intended before tracked work; repeated C initialization returns `1`. |
| Active detection | Enabled | The normal mode and the source of `WaitForGraph` reports. |
| Lock-order checking | Off unless the `lock-order-graph` feature is compiled | Rust enables it by default when that feature is compiled; C initialization currently disables it. |
| Logging | Off in the default build | With `logging-and-visualization`, Rust's default log path is `deloxide.log`; C passes a path to `deloxide_init`, or `NULL` to disable logging. |
| Stress mode | Disabled | Configure before initialization and only in a `stress-test` build. |
| Callback | Optional | Keep it bounded and hand work to an application-owned queue or supervisor. |

There is no public process reset/shutdown lifecycle for initialization. Plan the
detector as process-lifetime state and destroy C primitives only after their users
have stopped.

## Compatibility and supported surfaces

Rust CI runs library tests on Ubuntu, macOS, and Windows, and checks all supported
feature combinations on Ubuntu. The C smoke job currently builds and runs
`c_examples/basic_mutex.c` on Ubuntu, using the static library and
`-pthread -ldl -lm`. The release workflow packages the C header plus a static C
library for Ubuntu, macOS, and Windows. This establishes artifact availability on
those platforms, but it is not a claim that the same C smoke execution runs on
all three. The POSIX tracked-thread helpers require `pthread`; Windows consumers
use the manual registration pattern in the [C Guide](c-guide.md#threads).

Rust is the primary, fully configured interface. C exposes the tracked primitive
lifecycle, callbacks, thread registration, optional logging calls, and stress
controls documented in the [header](https://github.com/Emivvvvv/deloxide/blob/main/include/deloxide.h);
its current initialization path does not expose optional lock-order checking.

## Logs and visualization

With `logging-and-visualization`, Deloxide writes line-delimited JSON records for
tracked thread, lock, and condition-variable events, plus a terminal deadlock
record. The browser viewer reconstructs the event sequence rather than reading a
saved graph snapshot. See [Logging and Visualization](visualization.md) for the
format, log-path behavior, flush/showcase calls, and the browser URL data-handling
warning. The structured callback report remains the primary alert input; the view
is supporting incident evidence.

## Public resources

- [Rust API on docs.rs](https://docs.rs/deloxide/1.1.0/deloxide/)
- [Public C header](https://github.com/Emivvvvv/deloxide/blob/main/include/deloxide.h)
- [Source repository and issues](https://github.com/Emivvvvv/deloxide)
- [Focused microbenchmark methodology](https://github.com/Emivvvvv/deloxide/blob/main/docs/performance/microbench-methodology.md)
- [Release-candidate evaluation record](https://github.com/Emivvvvv/deloxide/blob/main/docs/performance/evaluation-2026-07-29.md)
- [Releases](https://github.com/Emivvvvv/deloxide/releases)
- [Security policy](https://github.com/Emivvvvv/deloxide/blob/main/SECURITY.md)
- [Changelog](https://github.com/Emivvvvv/deloxide/blob/main/CHANGELOG.md)
- [Contributing guide](https://github.com/Emivvvvv/deloxide/blob/main/CONTRIBUTING.md)

## Detection-model comparison

| Approach | Evidence it uses | What a report establishes |
| --- | --- | --- |
| Deloxide active detector | Current waits and incompatible tracked owners | A validated active cycle among tracked operations. |
| Deloxide lock-order graph | Historical acquisition ordering | A potential cyclic order that needs concurrency review. |
| Lock timeout/watchdog | Elapsed time or lack of progress | A symptom requiring application-specific attribution. |
| Static lock-order review | Code/control-flow analysis | A design-time risk, subject to analysis coverage and runtime paths. |

These approaches answer different questions. Choose the one that matches the
evidence needed, and do not convert their different detection models into a
universal performance or correctness ranking.

# Choosing a Mode

Start with the default active detector when you need to explain a reproduced hang.
Add an optional mode only for a question it can answer. Most importantly, do not
merge two evidence levels: `WaitForGraph` is an active, validated cycle among
currently tracked waits and incompatible owners; `LockOrderViolation` is a potential
historical ordering risk.

| Mode | Question answered | Cargo feature and builder call | Evidence and environment | Primary cost | Unsafe interpretation to avoid |
|---|---|---|---|---|---|
| Active wait-for graph | Which tracked threads are blocked on each other now? | Default features; `Deloxide::new().callback(...).start()` | Active `WaitForGraph`; use for runtime diagnosis and, after measuring the intended workload, production. | Tracking of supported synchronization and contention state. | "Every hang, lock, or external dependency is covered." Only tracked primitives are visible. |
| Lock-order graph | Have we observed inconsistent acquisition orders? | `lock-order-graph`; `.with_lock_order_checking()` (enabled by default when the feature is compiled). | Potential `LockOrderViolation`; use in development and CI before an active cycle occurs. | Historical ordering graph storage and traversal. | "This report proves a deadlock happened." It identifies a dangerous pattern that may never block. |
| Random stress testing | Can random timing changes make this suspected bug manifest? | `stress-test`; `.with_random_stress()` | Schedule perturbation, not a finding by itself; use in focused tests. Any callback still has its own `source` evidence level. | Added delays, longer and less deterministic tests. | "A passing run proves the race is gone" or "a failure proves the stress scheduler found the root cause." |
| Component-based stress testing | Do observed lock-acquisition relationships point to delays likely to expose this bug? | `stress-test`; `.with_component_stress()` | Schedule perturbation guided by tracked acquisition patterns; use in focused test environments. | Added delays plus relationship tracking. | "The component heuristic explores every schedule" or "no manifestation means no deadlock." |
| Logging and visualization | How did tracked execution reach the report? | `logging-and-visualization`; `.with_log("logs/deadlock_{timestamp}.log")` (or the feature's default log path). | Event history for incident analysis; it can accompany active or potential reports but does not change their certainty. | Queueing, serialization, file I/O, retained log data, and browser transfer when opened. | "The timeline is complete program tracing" or "opening it keeps data local." It covers tracked events and the current viewer receives encoded log data in its URL. |

Use [Read a Deadlock Report](diagnosis/reports.md) after an active callback. Use
[Find Potential Lock-Order Risks](diagnosis/lock-order.md) for the lock-order
workflow, [Stress Test a Suspected Race](diagnosis/stress-testing.md) for both stress
strategies, and [Logging and Visualization](visualization.md) for the event-log
workflow. The feature and lifecycle details are collected in
[Select Features and Configuration](rust/features.md).

# Finding Inconsistent Lock Order

Lock-order checking is a development aid for discovering an ordering rule that could deadlock under a different schedule. It must never be presented as an active wait-for graph result: [`DeadlockSource::LockOrderViolation`](https://docs.rs/deloxide/1.1.0/deloxide/enum.DeadlockSource.html#variant.LockOrderViolation) is **potential**, whereas [`DeadlockSource::WaitForGraph`](https://docs.rs/deloxide/1.1.0/deloxide/enum.DeadlockSource.html#variant.WaitForGraph) is an active, validated cycle.

## Enable and control it

The builder methods exist only with the `lock-order-graph` Cargo feature:

```toml
[dependencies]
deloxide = { version = "1.1.0", features = ["lock-order-graph"] }
```

When that feature is compiled, [`Deloxide::new`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.new) enables order checking by default. Make the choice visible in development or CI configuration with [`with_lock_order_checking`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.with_lock_order_checking), and turn it off explicitly for a controlled comparison with [`no_lock_order_checking`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.no_lock_order_checking).

```rust,ignore
use deloxide::{DeadlockSource, Deloxide};

Deloxide::new()
    .with_lock_order_checking()
    .callback(|info| match info.source {
        DeadlockSource::WaitForGraph => {
            // Active, validated deadlock: preserve evidence and page/escalate.
        }
        DeadlockSource::LockOrderViolation => {
            // Potential ordering cycle: send to code-review/triage workflow.
        }
    })
    .start()
    .expect("detector initialization");

// For a baseline run with the feature compiled:
// Deloxide::new().no_lock_order_checking().start()?;
```

Without the Cargo feature, neither control is available and no historical order graph is maintained. The normal detector can still emit active `WaitForGraph` reports.

## How an edge becomes a finding

Whenever a thread holds A and then acquires B, the order graph records the directed relationship `A -> B`. It does not need to observe both locks blocked at once.

```text
earlier path:  hold A, then acquire B    records A ──► B
later path:    hold B, then acquire A    would add B ──► A

result:        A ──► B ──► A             potential lock-order cycle
```

The report contains `source: LockOrderViolation`, a one-thread contextual `thread_cycle`, the requested `(thread, lock)` pair, and `lock_order_cycle: Some(...)`. The historical graph deliberately survives the individual critical sections that created its edges. Consequently it may expose a dangerous inversion that never manifested during a run, but it cannot prove concurrent blockage. An active wait-for graph instead contains current thread-to-incompatible-owner dependencies and is independently validated before producing `WaitForGraph`.

## Development and CI workflow

1. Enable `lock-order-graph` in a development/CI feature set and install a callback that records the full `DeadlockInfo` payload.
2. Exercise distinct entry points, error/rollback paths, and shutdown paths—the places most likely to acquire the same resources in a different order.
3. Group potential findings by the normalized `lock_order_cycle`, then identify the acquisition sites for every edge. Treat numeric IDs as run-local evidence; use logs and symbols to name the resources.
4. Decide whether the paths can hold the same lock instances concurrently. If they cannot, document why and keep a regression test; if they can, impose a consistent acquisition order or remove the nested hold.
5. Run the focused scenario repeatedly, optionally with [stress mode](stress-testing.md), then run the normal test suite. A later active `WaitForGraph` report raises the issue from a potential warning to an incident-quality reproduction.

Keep a baseline run with `no_lock_order_checking()` when measuring the checking cost or isolating a report. Do not use that baseline to dismiss an already observed cycle.

## Triage rules

| Finding shape | Interpretation | Next action |
| --- | --- | --- |
| Repeated identical `lock_order_cycle` across test runs or code paths | Strong evidence of a stable inconsistent policy, still **potential** rather than active. | Assign an owner, map all edges to source, and fix or document a proven non-overlap invariant. |
| One-off cycle after a rare error/shutdown path | A potential path worth preserving before it disappears. | Save the payload/log, create a focused test, and determine whether the instances can overlap. |
| `WaitForGraph` also appears | An active, validated deadlock occurred; it is not “just” a lock-order warning. | Follow [report reading](reports.md), capture stacks, and fix immediately. |
| Only `LockOrderViolation` appears under stress | Stress found an order inversion, not a deterministic deadlock. | Reduce the scenario and continue scheduled testing; do not claim a production deadlock without active evidence. |

Potential findings are most useful before release because they turn scheduling-dependent defects into reviewable lock-order evidence. They complement, and never replace, the current-state wait-for detector.

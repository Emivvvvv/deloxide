# Common Hang Patterns

Deloxide tracks its [`Mutex`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Mutex.html), [`RwLock`](https://docs.rs/deloxide/1.1.0/deloxide/struct.RwLock.html), and [`Condvar`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Condvar.html) wrappers. A `WaitForGraph` finding is an active, validated thread cycle; a `LockOrderViolation` finding is a potential order cycle reconstructed from earlier acquisitions. The distinction matters in every pattern below.

| Pattern | What Deloxide observes | Report and repair direction |
| --- | --- | --- |
| [Two-Mutex inversion](#two-mutex-inversion) | Each thread owns one mutex and attempts the other. | Active `WaitForGraph` when the two waits coexist; lock-order checking may warn earlier. Establish one acquisition order or avoid holding both together. |
| [Three-lock cycle](#three-lock-cycle) | Three directed waits close a ring. | Active `WaitForGraph` has three ordered thread IDs; a lock-order cycle is only potential. Put all three resources in one consistent order. |
| [Same-thread Mutex re-entry](#same-thread-mutex-re-entry) | A thread attempts a non-reentrant mutex it already owns. | A one-thread active cycle can be reported. Drop/pass the guard before the nested call, or avoid re-entering the critical section. |
| [RwLock read-to-write self-deadlock](#rwlock-read-to-write-self-deadlock) | A writer request is incompatible with every reader, including the requesting reader. | An active report can include the same thread. Release the read guard before writing; use an explicit application-level upgrade protocol if required. |
| [Writer behind several readers](#writer-waiting-for-multiple-readers) | One write request has wait edges to all incompatible readers. | Waiting alone is not a cycle and produces no report. If a cycle closes through one reader, it is active `WaitForGraph`; shorten reader critical sections or redesign handoff. |
| [Condvar wait then reacquire](#condvar-wait-and-mutex-reacquisition) | A waiter releases its mutex, waits for notification, then gets a synthetic tracked mutex attempt when notified. | A callback only occurs if reacquisition closes an active wait cycle (or a potential order cycle when enabled). Ensure the predicate and ownership protocol make progress. |
| [Notification without a deadlock](#notification-without-a-deadlock) | `notify_one`/`notify_all` wakes a waiter; it may briefly contend for the mutex. | No callback unless a real wait cycle exists. Do not equate a notification event with deadlock evidence. |

## Two-Mutex inversion

```text
T1 holds A, waits for B ──► T2
T2 holds B, waits for A ──► T1
```

The active graph contains `T1 -> T2 -> T1`, so `thread_cycle` names both threads and `thread_waiting_for_locks` maps each to its second mutex. With `lock-order-graph`, seeing `A -> B` in one path and `B -> A` in another emits a potential `LockOrderViolation` even before the overlap happens. Make the second acquisition use the same global order as the first, or split the work so neither path holds both mutexes.

## Three-lock cycle

```text
T1 holds A, waits B ──► T2 holds B, waits C ──► T3 holds C, waits A ──► T1
```

An active report orders the three threads around this ring; map each one to the requested lock before changing code. Lock-order analysis can instead report `Some([A, B, C, A])`, which is a potential historical order cycle. Choose a common order for all three locks and retest the three paths together.

## Same-thread Mutex re-entry

```text
T1 holds M
T1 calls a helper that locks M again
T1 waits for T1
```

`Mutex` is not re-entrant. When the second acquisition blocks, Deloxide can see a self-edge and emit an active `WaitForGraph` report with a one-element `thread_cycle`; it is not a lock-order warning. Keep the first guard's lifetime explicit, pass access through the helper instead, or move the nested operation outside the guard.

## RwLock read-to-write self-deadlock

```text
T1 holds read(R)
T1 requests write(R)
write(R) waits for all readers, including T1
```

Write access is incompatible with every active reader. Therefore an active report may name the same thread as requester and owner; this is expected evidence of a read-to-write upgrade attempt, not a duplicate callback. Do not retain the read guard while calling [`RwLock::write`](https://docs.rs/deloxide/1.1.0/deloxide/struct.RwLock.html#method.write). Release it, revalidate the data after acquiring write access, or implement the needed upgrade protocol around your data model.

## Writer waiting for multiple readers

```text
          ┌──► R1 (reads S)
W requests write(S)
          └──► R2 (reads S)
```

The wait-for graph records an edge from W to each incompatible reader. That is contention, not necessarily a deadlock: if R1 and R2 eventually release, W progresses and no callback is correct. If one reader is itself waiting back through W's path, the closing edge yields an active `WaitForGraph` report. Inspect the ordered cycle rather than assuming every reader named by a lock is part of it.

## Condvar wait and mutex reacquisition

[`Condvar::wait`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Condvar.html#method.wait) releases the associated mutex while sleeping. On notification, Deloxide records the wake-up and immediately tracks the mutex that the waiter must reacquire. The notification itself is not an owner edge. A callback requires a cycle involving that reacquisition (active `WaitForGraph`) or, when enabled, a separate potential `LockOrderViolation`. Protect the condition with the mutex, test the predicate in a loop, and make the notifier update the predicate under that mutex.

## Notification without a deadlock

A notifier can call [`Condvar::notify_one`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Condvar.html#method.notify_one) while it still owns the mutex. The woken thread can then wait briefly to reacquire it. That normal sequence may appear in a log but is not a deadlock and should not emit an active report unless another dependency completes a cycle. First verify that the predicate update occurs before notification and that the waiter loops on the predicate; only then investigate a missing wake-up.

## Hangs that are not lock cycles

| Symptom | Why Deloxide may not classify it | First diagnostic move |
| --- | --- | --- |
| Starvation | A thread repeatedly loses scheduling or lock fairness without a closed ownership cycle. | Measure wait duration and contention; check scheduler and lock fairness assumptions. |
| Livelock | Threads run and retry but make no useful state change. | Trace state transitions/retry counters rather than waiting for a lock-cycle callback. |
| Missed notification | The required state change or notification did not reach a waiter. | Log the predicate, wait registration, and notifier ordering; use a bounded test timeout. |
| Blocking I/O | A thread is blocked in a syscall, RPC, database, or channel outside tracked lock ownership. | Capture stacks and I/O traces; bound the external operation. |

These are real production hangs, but a silent Deloxide callback is expected when no cycle exists among tracked wrappers. See [troubleshooting](troubleshooting.md) for the next check.

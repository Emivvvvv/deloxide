# Reading a report

Deloxide emits two kinds of findings. They answer different questions:

| Source | Meaning | What to do |
| --- | --- | --- |
| `WaitForGraph` | Threads are currently blocked in a validated cycle among tracked locks. | Treat it as an active incident. |
| `LockOrderViolation` | The program has acquired locks in an order that could form a cycle under another schedule. | Treat it as a potential risk and reproduce it. |

An active report contains the participating threads and the lock each thread is
waiting for:

```text
source: WaitForGraph
thread_cycle: [ThreadId(2), ThreadId(3)]
thread_waiting_for_locks: [
  (ThreadId(2), LockId(7)),
  (ThreadId(3), LockId(4)),
]
```

Read the pairs as a loop: thread 2 waits for lock 7, whose incompatible owner is
thread 3; thread 3 waits for lock 4, whose incompatible owner is thread 2.

## Common deadlocks

The most common shape is opposite lock order:

```text
thread A: lock users, then lock cache
thread B: lock cache, then lock users
```

Choose one order and use it everywhere. For example, always acquire `users`
before `cache`.

A thread can also deadlock with itself. Typical examples are locking the same
non-reentrant `Mutex` twice or keeping an `RwLock` read guard while asking for a
write guard. Drop the first guard before the second acquisition.

Condition-variable bugs often come from the protocol around the wait rather
than the `Condvar` itself. Check that the predicate is tested in a loop, the
shared state is changed while holding the matching mutex, and notification
happens after the state change.

## If no report appears

Deloxide can only see synchronization performed through its wrappers. A cycle
that crosses `std::sync`, raw `parking_lot`, a channel, I/O, another process, or
a third-party primitive may be incomplete or invisible.

For a reliable reproduction:

1. Put the suspected operation in a child process or test with a timeout.
2. Use barriers to make the competing paths reach the lock order together.
3. Capture the callback report and optional event log before terminating it.
4. Add stress mode only after the deterministic shape is understood.

The runnable [`diagnose_deadlock` example](../../examples/diagnose_deadlock.rs)
shows a complete two-lock report.

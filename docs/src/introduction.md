# What Deloxide Is—and Is Not

When a concurrent program stops making progress and ordinary logs only say that it
is still running, Deloxide helps answer a more useful question: **which tracked
threads are waiting on which tracked locks right now?** It is a runtime deadlock
detector for Rust applications, with a C interface for programs that need the same
tracked synchronization boundary.

Deloxide is built for the frustrating hang that depends on timing. A debugger,
additional logging, or a different machine can change the schedule enough to hide
the failure. Its default detector records waits between threads that use Deloxide's
`Mutex`, `RwLock`, and `Condvar` wrappers. If the current waits and incompatible
owners form a cycle, the callback receives an ordered `thread_cycle` and the lock
each participating thread is trying to acquire. That is active evidence:
`DeadlockSource::WaitForGraph` means Deloxide validated a blocked dependency cycle
in its current tracked state.

## The boundary is deliberate

Deloxide does not observe every way a process can stop. It sees synchronization
performed through its own wrappers and tracked thread helpers. A cycle that crosses
a raw `std::sync` lock, a lock hidden in another library, I/O, a channel, a process,
or a remote service may be incomplete or invisible to the detector. A report is a
strong statement about the tracked boundary, not a claim that Deloxide has explained
every hang in the program.

The default build provides active wait-for detection without the optional logging
pipeline. Optional features add three different kinds of help:

- `lock-order-graph` records historical acquisition order and can report
  `DeadlockSource::LockOrderViolation`. That is a **potential** ordering risk, not
  proof that threads are blocked in a live cycle.
- `stress-test` changes scheduling around tracked lock operations to make a
  suspected timing bug easier to manifest in controlled tests.
- `logging-and-visualization` writes tracked events for later reconstruction in the
  visualization.

Rust is the primary integration: configure `Deloxide`, then adopt the tracked Rust
primitives where you need evidence. The C API is a supported secondary surface for
the same implementation; it requires explicit lifecycle and ownership handling, so
start with the [C Guide](c-guide.md) when that is your entry point.

## Choose the smallest useful tool

Deloxide is not mandatory for every concurrent program. Standard locks may be all
you need when the critical sections are small, the acquisition order is clear, and
tests already cover the failure modes you care about. Static analysis can be the
better first tool when a bug is visible from code structure before the program runs.
Development-only ordering checks are useful when you want to catch an inconsistent
order before it blocks. Reach for the active wait-for detector when you need concrete
runtime participants and waited-lock pairs from a reproduced hang.

The [mode guide](choosing-a-mode.md) compares these choices. If you already have a
suspected two-lock hang, continue with the [first diagnosis](getting-started.md).

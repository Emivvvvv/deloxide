# What deadlocks are—and what Deloxide does

A deadlock happens when threads wait on one another in a cycle and none of them
can continue.

```text
thread A holds lock 1 and waits for lock 2
thread B holds lock 2 and waits for lock 1
```

The program is still running, but the affected work has stopped. Deadlocks are
especially frustrating because timing matters: adding a log line, attaching a
debugger, or rerunning the test can make the problem disappear.

## What Deloxide does

Deloxide replaces the synchronization on a suspicious path with tracked
`Mutex`, `RwLock`, and `Condvar` wrappers. When a thread must wait, Deloxide
records which lock it wants and which thread currently owns the incompatible
guard. If those waits form a cycle, your callback receives the threads and locks
involved.

```text
unexplained hang
      ↓
tracked lock waits
      ↓
ThreadId(2) → LockId(7) → ThreadId(3)
ThreadId(3) → LockId(4) → ThreadId(2)
```

That default `WaitForGraph` finding describes an active blocked cycle. Deloxide
can also:

- record events for an interactive visualization;
- warn about potentially dangerous lock-order inversions;
- perturb scheduling to help rare deadlocks reproduce;
- send structured findings through a callback; and
- expose the same tracked primitives to C programs.

Rust is the primary interface. Adoption can be incremental: start with the locks
around the suspected hang instead of rewriting the whole application.

## What it does not do

Deloxide only sees operations that pass through its wrappers. It cannot explain
a cycle hidden inside raw locks, channels, I/O, another process, or a remote
service. It is also not a data-race or distributed-deadlock detector.

Use Deloxide when you need a concrete answer to “which tracked threads are
waiting on which tracked locks?” Use ordinary locks when you do not need that
diagnosis, and keep normal tracing, timeouts, and thread dumps for hangs outside
the tracked boundary.

Next: [install Deloxide](installation.md), then run
[your first diagnosis](getting-started.md).

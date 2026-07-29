# Core Concepts

A **lock identity** names one tracked synchronization object. A **thread identity**
is stable for a tracked thread. When thread T cannot acquire lock L, Deloxide records
the acquisition mode and resolves L to its current incompatible owner threads.

The active graph contains direct thread dependencies:

```text
T1 -> T2
```

means T1 is blocked on a tracked lock currently owned incompatibly by T2. A directed
cycle means none of its participants can make progress by completing the acquisition
being tracked.

`DeadlockSource::WaitForGraph` and `DeadlockSource::LockOrderViolation` deliberately
have different meanings. The former is runtime state; the latter is a warning about
an observed ordering pattern.

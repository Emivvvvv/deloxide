# Tracked Primitive Semantics

Trust a Deloxide report only to the extent that the synchronization involved uses
these wrappers. The physical behavior comes from `parking_lot`; Deloxide adds
ownership and wait tracking around it.

## Mutex

A Mutex has one exclusive physical owner and one per-lock atomic owner hint.
Eligible uncontended acquisition first succeeds physically, then publishes the
current thread with a release store. When contention requires global tracking,
`complete_acquire` records the same owner in the detector, clears the acquiring
thread's wait intent, and refreshes waiters for that lock.

Guard drop clears the atomic hint before the embedded physical guard releases. If
the guard was globally tracked or a slow waiter is visible, the detector removes
the matching owner and held-lock state and refreshes affected waiters. Otherwise
the normal default-feature release avoids the global detector.

A Mutex wait is incompatible only with the current Mutex owner. The wrapper's
atomic hint exists so a contended thread can publish an owner that acquired on the
local fast path; it is not permission to infer an owner from a failed try or a
Condvar notifier.

## RwLock

RwLock ownership has two different representations:

- `writer_owner` is a per-lock atomic hint for the one possible writer;
- `rwlock_readers` is a detector map of thread IDs to positive recursion counts.

A read conflicts only with the current writer. A write conflicts with the current
writer and every thread in the reader map. Reads therefore always attempt the
physical lock while the global detector is held, even when uncontended. On each
successful read, the detector increments that thread's count. Releasing one of
several live read guards decrements the count; only the final release removes the
reader identity and its writer dependency.

This counting preserves the detector semantics of recursive or repeated reads by
one thread. It does not override `parking_lot` fairness or promise that an
additional read can never block when a writer is queued.

A thread that holds a read guard and calls blocking `write()` remains among the
write's incompatible readers. The derived self-edge is a one-thread upgrade
cycle, which is reportable after active validation. There is no implicit upgrade
operation. The nonblocking `try_write()` simply returns `None` when physical write
acquisition fails.

RwLock writes use the same eligible exclusive fast path and slow-waiter handshake
as Mutex. Successful write ownership is published after physical acquisition.
Read and write releases update only the mode-specific owner state and derived
edges described in [Wait-For Graph](wait-for-graph.md).

## Condvar

`Condvar::wait` and `wait_timeout` use this order in
`src/core/locks/condvar.rs`:

1. Hold a Mutex slow-waiter token for the entire wait and possible reacquisition.
2. Add `(thread, mutex)` to the Condvar's detector queue.
3. Clear the Mutex owner hint and report Mutex release to the detector.
4. Let `parking_lot::Condvar` atomically release the physical mutex and park.
5. On notification or timeout, return only after `parking_lot` has physically
   reacquired the mutex.
6. Publish the waiting thread as the actual Mutex owner, complete global Mutex
   acquisition, mark the existing guard globally tracked, and remove the exact
   Condvar wait record.
7. Drop the slow-waiter token after the sequence is complete.

The token in step 1 is essential: during the interval when another thread can own
the released Mutex, eligible fast-path owners observe contention and publish the
real owner globally. On notification, detector code may create a wait intent for
the woken thread only when an actual current Mutex owner is recorded. `notify_one`
and `notify_all` never make the notifier the Mutex owner.

Timeout follows the same physical reacquisition and cleanup sequence, but it does
not have the same detector coverage as notification. Only `notify_one` and
`notify_all` can install the synthetic Mutex wait intent described above. A pure
timeout can therefore block while physically reacquiring the Mutex without that
dependency appearing in the active WFG, unless a notification also selected the
waiter. The boolean result describes whether `parking_lot` timed out; either way,
the function returns with the Mutex guard reacquired.

## Nonblocking attempts leave no wait

Failure of `Mutex::try_lock`, `RwLock::try_read`, or `RwLock::try_write` returns
`None` without persisting a wait intent, adding a reverse waiter entry, or creating
a WFG dependency. `try_read` still holds the detector during the physical attempt
so a success can be counted atomically with detector state; a failure changes no
reader or wait state.

Successful nonblocking exclusive attempts publish an owner and may become globally
tracked when slow waiters exist or the lock-order feature is compiled. They remain
acquisitions, not waits.

## Ordering protects metadata, not user data

Mutex owners and RwLock writer hints are stored with release ordering and loaded
with acquire ordering. Slow-waiter registration and token drop use acquire-release
read-modify-write operations; waiter observation uses an acquire load. Together
with the global detector mutex, those orderings publish the metadata used by the
contention handshake.

The physical `parking_lot` guard still supplies synchronization for application
data. Do not infer that Deloxide's atomics extend the protected critical section
or make an untracked access safe.

The owner and slow-waiter fields remain separate. A packed owner/contention word
could provide a stronger single-state transition, but it would change the hottest
exclusive path and needs dedicated concurrency and performance evaluation. It is
explicitly deferred.

## Operation-shaped costs

| Operation shape | Detector work |
| --- | --- |
| Eligible uncontended Mutex/RwLock write | Constant local owner and contention atomics; no WFG allocation or traversal in the default build. |
| RwLock read | Global detector mutex plus expected hash-map updates, even when physically uncontended. |
| Contended wait | Slow-waiter atomics, detector-held physical recheck, intent/index updates, owner resolution, and zero or more graph path searches. |
| Ownership change | Mode-specific owner update and affected edge removal or refresh; cost depends on indexed waiters and incompatible owners. |
| Candidate validation | Every cycle edge plus mode-aware current-owner resolution; a write wait can enumerate all current readers. |

One novel direct edge can require a breadth-first search over the reachable WFG
component. A wait or refresh can process several owners and several waiters, so no
single constant or universal `O(V + E)` bound describes the entire operation.
Memory likewise follows live owner counts, reader recursion entries, intents,
waiter indexes, graph edges, and optional feature state. Measure the workload shape
you actually deploy.

# How It Works

This chapter describes the implementation under the hood. It is intended for
concurrency engineers, reviewers, and readers evaluating Deloxide's cost model.

## Components

Tracked wrappers own the physical `parking_lot` primitive and a stable lock ID. The
global detector owns:

- current Mutex owners;
- current RwLock writer and counted reader ownership;
- mode-aware thread wait intents;
- a reverse lock-to-waiters index;
- the direct thread-to-thread wait-for graph; and
- optional held-lock and lock-order state.

Callbacks run on a dispatcher thread so a detecting thread can remain physically
blocked without preventing report delivery.

## Optimistic operation

Mutex and RwLock writers first attempt the physical lock. An uncontended operation
publishes its owner in a per-lock atomic and avoids graph traversal and allocation.
A per-lock slow-waiter state tells acquisitions and releases whether an ownership
handoff must be synchronized with the detector.

## Contention registration

After a physical attempt fails:

1. the wrapper marks a slow waiter;
2. it enters the detector;
3. it rechecks the physical lock without blocking;
4. if the recheck succeeds, it records the contended acquisition immediately;
5. otherwise it stores `thread -> (lock, mode)`;
6. it resolves current incompatible owners into direct WFG edges; and
7. it leaves the detector and blocks on the physical primitive.

The detector-held recheck closes the most important sampling interval. An ownership
transition is either observed by the recheck or sees the slow-waiter state and
refreshes the affected edges.

## Ownership handoff

Suppose T2 waits for L while T1 owns it:

```text
wait intent: T2 -> (L, Mutex)
derived edge: T2 -> T1
```

If L transfers to T3, Deloxide does not retain T1 merely because it was sampled
earlier. It resolves the unchanged wait intent again:

```text
wait intent: T2 -> (L, Mutex)
derived edge: T2 -> T3
```

This keeps the paper-compatible direct thread graph while making the lock wait the
stable supporting fact.

## Modes and incompatible ownership

- Mutex wait: the current Mutex owner.
- RwLock read wait: the current writer.
- RwLock write wait: the current writer and every current reader.

RwLock readers are counted per thread, so dropping one recursive read guard does not
erase another. A blocking read-to-write upgrade includes the calling reader itself
and forms a one-thread cycle. `try_write` remains nonblocking and returns `None`.

## Cycle detection

Before adding `A -> B`, the WFG searches for a path from B back to A. The graph keeps
forward and reverse adjacency maps; reverse edges make thread cleanup proportional
to the affected neighbors instead of the whole graph.

A candidate is not dispatched immediately. Every edge is revalidated while the
detector is held:

1. the source still has a wait intent;
2. the target is a current incompatible owner for that intent; and
3. the acquisition has not completed or been cancelled.

If any edge is stale, the report is discarded and affected derived edges are
refreshed.

## False-positive controls

Immediate sampled-owner verification has been replaced by complete-cycle validation.
The common-lock filter cannot use a shared RwLock read as proof of mutual exclusion,
because multiple cycle participants can hold that read simultaneously. Condvar
notification also never invents mutex ownership for a notifier that does not hold
the mutex.

These controls reduce known stale-state reports; they are not a claim that every
possible integration or timing source is eliminated. Untracked primitives remain
outside the graph.

## Costs

Let V be tracked waiting threads and E be direct wait edges. A cycle search is
`O(V + E)` in the explored component. Ownership refresh is proportional to the
waiters for one lock and that lock's incompatible owners. These operations are
contention-only.

The Mutex fast path adds cheap contention-state observations around ownership
changes. Microbenchmarks report both the percentage and absolute nanosecond change;
see [Performance](performance.md). RwLock reads currently update counted reader state
through the detector because writers must know each incompatible reader identity.

## Memory ordering

Owner publication uses release ordering and readers use acquire ordering. Slow-waiter
registration uses atomic read-modify-write operations. The physical lock still
protects user data; these atomics protect detector metadata and the handshake around
ownership visibility.

The implementation avoids acquisition generations unless a deterministic
release/reacquisition regression demonstrates that current-owner validation is
insufficient. This keeps complexity out of the fast path until evidence requires it.

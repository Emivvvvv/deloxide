# Direct Wait-For Graph

The graph used for active deadlock detection remains a direct thread graph:

```text
waiting thread  ──►  current incompatible owner
```

Deloxide also keeps the fact that supports each derived edge:

```text
Thread  ──►  (Lock, Mode)
```

That mode-aware wait intent is not a second traversal graph. It lets the detector
re-resolve an edge when ownership changes and check whether every dependency in a
candidate cycle is still current.

## Resolve a wait into owners

`WaitMode` in `src/core/detector/wait.rs` determines which owners prevent the
physical operation:

| Wait intent | Incompatible owners | Consequence |
| --- | --- | --- |
| `Mutex` | The one current Mutex owner, if any. | At most one derived edge. |
| `RwRead` | The current RwLock writer, if any. | Other readers are compatible and create no edge. |
| `RwWrite` | The current writer plus every thread with a positive read count. | One waiter can have many outgoing edges; a reader upgrading in place includes itself. |

Reader ownership is counted per thread. Two read guards held by the same thread
represent one owner identity with count two, while two different reading threads
are two incompatible owners for a writer. Dropping one recursive read guard leaves
the dependency in place until that thread's final read guard is released.

## Register only after a physical recheck

The wrappers in `src/core/locks/mutex.rs` and `rwlock.rs` first observe a failed
physical attempt and register a slow-waiter token. The slow-path helpers in
`src/core/detector/mutex.rs` and `rwlock.rs` then hold the global detector while
trying the physical primitive again.

If this recheck acquires the primitive, the detector completes ownership without
creating a wait. Only another physical failure persists the wait intent and asks
the detector to resolve current incompatible owners. The owner hint can publish an
exclusive owner that previously used the local fast path; it must be the wrapper's
actual owner observation, never a guessed notifier or other convenient thread.

For each wait intent, `set_wait_intent` maintains two indexes:

- `thread_waits_for` maps the thread to its one current `(lock, mode)`;
- `lock_waiters` maps the lock back to every thread whose intent names it.

Replacing or clearing an intent also clears that thread's derived outgoing WFG
edges. The reverse lock-to-waiter index makes handoff repair local to the affected
lock instead of requiring a scan of all thread intents.

## Direct edges and candidate traversal

`WaitForGraph` in `src/core/graph/wait_for_graph.rs` stores forward and reverse
thread adjacency. Before storing a novel `A -> B` edge, it runs breadth-first
search from B to A:

```text
existing path:  B -> ... -> A
new dependency: A -> B
candidate:      B -> ... -> A -> B
```

The returned candidate vector is the existing path, without repeating its first
thread. The closing dependency is supported by A's current wait intent and owner
resolution; it is not inserted after the search reports a cycle. A self-dependent
RwLock upgrade is the one-node case.

Registering a write wait may propose edges to many readers. The detector retains
the first candidate it finds but continues processing the remaining owners.
Likewise, refresh retains the first candidate but still repairs every indexed
waiter. Candidate selection is therefore not a promise to enumerate or dispatch
every cycle in one transition.

The graph's reverse thread adjacency serves a different purpose from
`lock_waiters`: it removes incoming edges when tracked thread cleanup runs and
removes the matching reverse entry when an outgoing edge is cleared.

## Handoff refresh

Suppose T2 waits for Mutex L while T1 owns it:

```text
intent: T2 -> (L, Mutex)
edge:   T2 -> T1
```

If detector ownership later says T3 owns L, refreshing L clears T2's old outgoing
edges, resolves its unchanged intent again, and proposes `T2 -> T3`. This logic is
implemented by `refresh_waiters_for_lock` and is called from the ownership paths
that need affected-waiter maintenance.

The concrete RwLock behavior is mode-aware rather than a blanket refresh:

- a successful nonblocking read counts the reader and refreshes indexed waiters;
- the final read release removes edges from writers to that reader;
- write completion clears the writer's own intent and refreshes indexed waiters;
- write release removes edges to the released writer.

Mutex completion and release perform lock-wide refresh. Condvar notification can
register a woken thread's Mutex intent only when detector state has an actual
Mutex owner; it does not make the notifying thread the target.

## Why the abstraction remains paper-compatible

Traversal, cycle candidates, and active reports are all expressed as direct
`Thread -> Thread` dependencies. `Thread -> (Lock, Mode)` metadata is supporting
evidence for reconstruction and validation, not a replacement bipartite
thread/lock graph. This preserves the main direct-WFG abstraction while allowing
multiple readers, multiple waiters, and ownership handoff to be represented
accurately enough for current-state checking.

The graph is still a sampled detector model. The per-lock owner hint and
slow-waiter count are separate atomics, and the implementation has no acquisition
generation. Read [Validate Active Cycles](validation.md) before treating a returned
path as reportable evidence.

# Validation and False-Positive Controls

A path returned by graph traversal is a candidate, not yet a report. Deloxide
converts it to active wait-for evidence only through this detector-held sequence:

```text
candidate cycle
  -> inspect every edge
  -> compare wait intent with current incompatible owners
  -> repair stale edges
  -> retain a replacement current cycle when found
  -> dispatch only validated active evidence
```

The implementation is in `src/core/detector/wait.rs`; report extraction and
dispatch are in `deadlock_handling.rs`.

## Validate the complete cycle

For each source thread in the candidate, the validator takes the next thread
cyclically as the target. The edge is current only when:

1. the source still has a mode-aware wait intent;
2. that intent still names its current lock and mode; and
3. resolving current incompatible owners for the intent still includes the
   target.

Every edge must pass. A current last edge cannot rescue an older stale edge, and
the validator does not accept a candidate merely because the direct WFG still
contains old adjacency.

Mode matters during this comparison. A read wait is valid only against the
current writer. A write wait can be valid against the writer or any counted
reader, including the waiting thread itself during a blocking upgrade. Shared
readers never conflict with one another.

## Repair before giving up

When any candidate edge is stale, `validated_deadlock_info` collects the locks
named by the surviving candidate intents. For each distinct lock it refreshes all
threads in the reverse waiter index:

- clear that waiter's derived outgoing edges;
- resolve the waiter's current incompatible owners;
- rebuild all applicable dependencies; and
- remember the first replacement candidate without stopping graph maintenance.

If refresh finds a candidate, that replacement also receives complete-cycle
validation. Only then is `DeadlockInfo` created. A stale sampled cycle is therefore
discarded, while a different cycle exposed by current ownership can be retained.
The operation does not promise to enumerate every simultaneous cycle, and repair
is scoped to locks recoverable from the stale candidate's remaining intents.

## Controls for known false-positive shapes

### Shared RwLock reads are not mutual exclusion

The common-lock filter can discard an impossible sampled cycle when every
participant is said to hold the same mutually exclusive lock. It explicitly
removes any lock present in the RwLock reader map from that proof: several threads
can legitimately hold a shared read at the same time, so a common read cannot make
the cycle impossible.

### Stale owners are re-resolved

An unchanged intent is authoritative about what operation is blocked; its direct
target is not. Ownership handoff refresh derives the target again from the current
Mutex owner, RwLock writer, or counted readers. Complete validation repeats that
comparison for all candidate edges.

### Partial maintenance does not stop at the first cycle

Both wait registration across multiple owners and lock refresh across multiple
waiters retain the first candidate but continue updating the rest of the graph.
This prevents the first discovered cycle from leaving later owners or waiters with
unrepaired edges.

### Condvar notification does not invent an owner

Notification marks selected threads as woken and may derive a Mutex wait from the
detector's actual current Mutex owner. The notifier is only the source of the
notify event; it is never guessed to own the mutex. After physical reacquisition,
the waiting thread publishes itself and completes normal Mutex ownership before
Condvar cleanup.

## What validation does not prove

Validation is serialized with other detector mutations, but it does not freeze
the physical primitives. It checks the detector's current mode-aware ownership,
which is kept close to physical state by owner publication, slow-waiter
observation, and the detector-held recheck. It does not perform a new physical
lock probe for every edge during validation.

Owner and contention state are separate atomics, so there is no single
linearizable owner/waiter snapshot. The detector also has no acquisition
generation. A release and later reacquisition by the same thread can therefore
look like the same current owner—an ABA-shaped limitation—even though complete
current-owner validation has no reproduced deterministic failure requiring
generations. Packed state and generations remain deferred, not silently assumed.

Untracked synchronization, raw `parking_lot` or standard-library primitives, and
threads that bypass Deloxide are outside this evidence. Thread-exit cleanup
removes WFG adjacency but intentionally retains some Condvar and wait mappings;
process-wide state is not a transactional trace of every lifetime event.

## Keep lock-order certainty separate

The optional lock-order graph asks whether observed historical acquisitions form
a `Lock -> Lock` cycle. Such a finding is labeled
`DeadlockSource::LockOrderViolation`: it is a potential ordering risk and does not
pass through active wait-cycle validation. A default
`DeadlockSource::WaitForGraph` report, by contrast, is dispatched only after the
complete current dependency cycle validates.

Use that distinction when responding: preserve a WFG report as active evidence;
triage a lock-order cycle as a development warning that still needs a feasible
concurrent schedule.

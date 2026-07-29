# Correctness Hardening Report

**Candidate:** Deloxide 1.1.0 on branch `release-hardening`
**Release baseline:** commit `3b28ace`

## Implemented findings

| Finding | Regression evidence | Change | Focused verification |
|---|---|---|---|
| Stale owner after handoff | Detector handoff test retained `T2 -> old owner` | Mode-aware wait intents and per-lock edge refresh | Direct edge retargets to the current owner |
| Older stale edge accepted | Full-cycle diagnostic accepted a candidate with one obsolete edge | Validate every candidate edge against current incompatible ownership | Stale candidate is discarded and refreshed |
| Partial edge refresh | Cycle discovery returned before remaining owners or waiters were processed | Complete graph maintenance while retaining the first candidate cycle | Multi-owner and multi-waiter refresh tests |
| Replacement cycle discarded | Stale-cycle repair found a current cycle but ignored it | Return a validated cycle recovered during refresh | Stale candidate resolves to the current two-thread cycle |
| Fast RwLock writer invisible | Blocking reader could fail physical acquisition without a global writer | Slow-waiter handshake, detector-held recheck, atomic writer hint | Blocking-reader lifecycle test |
| RwLock upgrade spin/miss | Blocking read-to-write upgrade could spin or omit the caller | Removed adaptive spin; writers resolve all counted readers including self | One-thread upgrade cycle plus existing upgrade test |
| Failed `try_read` wait | Nonblocking failure could persist a dependency | Separate nonblocking read path | State remains free of intent, reverse waiter, and WFG edge |
| Recursive reads | One guard release erased ownership, LOG holds, and writer edges for another live guard | Per-thread read counts; remove LOG/WFG ownership only after the final guard | First release preserves count, hold, and writer edge |
| Shared-read filter | Common shared read suppressed a real candidate | Shared RwLock ownership is excluded from mutual-exclusion proof | Focused filter test |
| Condvar timeout cleanup | Completed wait remained in the Condvar queue | Exact `(thread, condvar, mutex)` cleanup | Queue/mappings are empty after end |
| Synthetic notifier owner | Notify outside the mutex invented a dependency | Notification uses only actual mutex ownership | Outside-notify test produces no dependency |
| Condvar fast-guard release | Reacquisition inserted a global owner without updating the guard's release state | Mark the existing guard globally tracked after reacquisition | Focused wait-timeout lifecycle test |
| C RwLock guard overwrite | Second read guard silently dropped the first | TLS maps keyed by RwLock pointer | Exported-function test holds two guards |
| Callback panic | A panic terminated the only dispatcher | Per-invocation unwind isolation | A second invocation executes after the first panics |

## Architecture impact

The active graph remains direct `Thread -> Thread`, matching the submitted design at
its main abstraction boundary. The supporting `Thread -> (Lock, Mode)` metadata is
authoritative for refreshing direct edges during ownership handoff. There is no
replacement with a bipartite graph and no acquisition generation in this release.

Uncontended operations still avoid graph traversal, allocation, and the global
detector. Mutex and RwLock writers inspect per-lock contention state around ownership
changes. Slow paths recheck the physical primitive while holding the detector before
persisting a wait.

These changes close the stale-edge and handoff failures reproduced by the focused
tests, but they do not claim a linearizable snapshot across the separate owner and
slow-waiter atomics. A packed owner/contention state would provide that stronger
formal boundary; it is intentionally deferred until its fast-path cost is measured
and its state transitions receive dedicated concurrency testing.

## Focused performance checkpoint

Short Criterion runs used 30 samples, a one-second warmup, and a two-second
measurement window on the same machine/toolchain. Results are medians from the
repeated comparison, not the complete evaluation.

| Case | `3b28ace` | Candidate | Approximate change |
|---|---:|---:|---:|
| Mutex uncontended | 8.962 ns | 9.04 ns | +0.08 ns / +0.9% |
| Mutex two-thread handoff | 33.409 µs | 34.673 µs | +3.8% |
| RwLock read uncontended | 50.517 ns | 48.224 ns | -4.5% |
| RwLock write uncontended | 9.147 ns | 9.300 ns | +1.7% |

The accepted Mutex layout stores one shared owner/contention-state reference in the
guard instead of two field references. A fresh pre-layout run measured 9.625 ns; two
consecutive post-layout runs measured 9.036 ns and 9.043 ns. Both contention checks
and their memory ordering remain unchanged.

Matched controls measured 8.791 ns for `parking_lot::Mutex::lock` and 8.589 ns for
`parking_lot::Mutex::try_lock`. A forced cold/non-inlined slow-path helper measured
18.058 ns and was fully reverted. The handoff movement remains small relative to
scheduler noise and was not statistically distinguished from the prior candidate.
The full evaluation is deliberately deferred.

## Deferred risks

- The optional logger still uses an unbounded ordinary-event queue. Selecting drop,
  block, or coalescing behavior changes diagnostic guarantees and needs a separate
  saturation design/test before implementation.
- Acquisition generations remain excluded because complete current-owner validation
  has not left a deterministic ABA failure.
- Owner and slow-waiter state remain separate atomics. No reproduced failure remains
  after physical recheck and handoff refresh, but a packed-state protocol is the
  remaining option for a formal single-snapshot handoff guarantee.
- Process-wide initialization still accepts only the first callback configuration;
  changing singleton lifecycle is a public behavior decision.

## Architecture figure specification

The existing `images/architecture.png` should be replaced before the 1.1.0 public
release. The replacement should show:

1. Rust/C wrappers and physical `parking_lot` primitives;
2. the uncontended path bypassing the global detector;
3. per-lock slow-waiter state and the detector-held physical recheck;
4. mode-aware wait intent feeding the direct thread WFG;
5. ownership-handoff edge refresh;
6. complete-cycle validation before callback dispatch; and
7. optional lock-order, stress, logger, and visualizer paths.

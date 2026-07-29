# Optimized Fast Path

The fast path attempts the physical lock first, publishes exclusive ownership, and
returns without graph work when no slow waiter is visible. A contended acquisition
or release synchronizes ownership with the detector and refreshes only that lock's
waiters. Optional logging, lock-order analysis, and stress features add their
documented feature-specific work.

# Rust Guide

Use `Mutex`, `RwLock`, and `Condvar` as guard-based synchronization wrappers.
`try_lock`, `try_read`, and `try_write` never block. Blocking RwLock upgrades are
reported as self-deadlocks; drop the read guard before taking a write guard.

Callbacks receive `DeadlockInfo` asynchronously. Keep callbacks bounded, avoid
assuming the detecting thread can make progress, and move expensive incident
handling to another service or queue.

Optional features are selected in Cargo and are not runtime switches.

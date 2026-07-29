# Choosing Deloxide or another tool

Choose based on the question you need answered, not one benchmark.

| Need | Usually choose |
| --- | --- |
| Fast synchronization with no deadlock diagnosis | `std::sync` or `parking_lot` |
| Periodically inspect a process for active lock cycles | `parking_lot` with `deadlock_detection` |
| Replace standard locks temporarily with a synchronous runtime debugger | `no_deadlocks` |
| Active reports, optional lock-order analysis, stress scheduling, callbacks, and visualization in one toolkit | Deloxide |
| Prove lock ordering or concurrency properties before running the program | Static analysis, model checking, or a formal lock-order policy |

## Feature comparison

| Capability | `std::sync` | `parking_lot` detector | `no_deadlocks` | **Deloxide** |
| --- | :---: | :---: | :---: | :---: |
| `Mutex`, `RwLock`, and `Condvar` wrappers | Yes | Yes | Yes | **Yes** |
| Active runtime cycle detection | No | Yes, when checked | Yes | **Yes, on the blocking path** |
| Structured callback report | No | No | No | **Yes** |
| Historical lock-order analysis | No | No | No | **Optional** |
| Built-in schedule stress modes | No | No | No | **Optional** |
| Interactive event visualization | No | No | No | **Optional** |
| Rust and C integration | Rust | Rust | Rust | **Rust + C** |
| Default uncontended Mutex median in Deloxide's recorded harness | Not measured in the current focused CSV | 10.28 ns | Not measured in the current focused CSV | **9.12 ns** |

`parking_lot` is an excellent primitive library and may be the right answer when
raw synchronization performance is the only requirement. Its experimental
deadlock detector is a separate API that applications normally call
periodically.

`no_deadlocks` provides a familiar debugging replacement and keeps lock state in
a global manager. Its own documentation recommends it primarily while debugging
before switching back to standard locks.

Deloxide is strongest when you want to keep one diagnosis surface from
reproduction through production: an active callback when threads block, optional
lock-order warnings before they block, stress modes for rare schedules, and a
visual event trail. Its trade-off is that synchronization must use Deloxide's
wrappers and optional features add work.

See [Performance and benchmarks](performance.md) for measured results and
[Production checklist and limits](operations.md) for the coverage boundary.

# Deloxide

Deloxide detects blocked-thread dependency cycles in programs that use its tracked
Rust or C synchronization primitives. It is designed for the awkward class of bugs
that disappear under a debugger and emerge only under a particular production
schedule.

The default active detector maintains a runtime wait-for graph. Optional features
add lock-order analysis, schedule stress, structured logging, and browser-based
visualization.

Deloxide does not instrument arbitrary `std` locks, operating-system handles, or
locks hidden inside third-party libraries. A cycle that crosses an untracked
primitive is outside its view. Lock-order reports describe potential risk; active
wait-for reports describe a validated cycle among currently tracked waits and owners.

The fastest route to a useful result is:

1. replace the locks involved in the suspected path;
2. install a callback;
3. reproduce the workload; and
4. use the reported thread/lock cycle to identify the acquisition order.

# Production checklist and limits

Deloxide can run in production, but roll it out like any other observability
tool: start small and measure the workload that matters to you.

## Rollout checklist

1. Initialize Deloxide before instrumented work begins.
2. Replace every lock in the suspected cycle; mixed tracked and untracked paths
   create blind spots.
3. Start with the default active detector.
4. Keep the callback bounded and hand slow work to another component.
5. Benchmark the exact feature set under representative contention.
6. Enable logging only when you need an event timeline.
7. Use lock-order and stress features mainly in development, CI, or focused
   reproduction environments.

## Logging

The logging feature writes events asynchronously. Choose a unique path containing
a PID, UUID, or another collision-proof value: existing files are truncated, and
the built-in `{timestamp}` placeholder has only one-second precision.

The ordinary-event logger queue is currently unbounded. Monitor memory and file
growth during a capture. The visualization encodes the log into a URL opened at
`https://deloxide.vercel.app/`; review sensitive data before using it.

## Coverage limits

Deloxide observes its own `Mutex`, `RwLock`, and `Condvar` wrappers. It does not
build dependencies through raw locks, custom atomics, channels, I/O, external
services, processes, or remote machines.

`WaitForGraph` is active evidence inside that tracked boundary.
`LockOrderViolation` is only a potential ordering risk.

Deloxide is not a general data-race, starvation, livelock, or distributed
deadlock detector. A missing report does not prove that the process is healthy.
Keep thread dumps, tracing, timeouts, and service-level monitoring alongside it.

If Deloxide's cost or behavior misses your acceptance target, first disable
optional features. If the default detector still does not fit, keep it in the
reproduction/test environment instead of forcing a broad deployment.

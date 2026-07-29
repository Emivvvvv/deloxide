# Select Features and Configuration

Deloxide's features are Cargo compile-time choices, not runtime switches. Start
with the default active wait-for detector, then add only the evidence or test
behavior needed for the environment. The public base API is always
[`Deloxide`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html),
[`Mutex`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Mutex.html),
[`RwLock`](https://docs.rs/deloxide/1.1.0/deloxide/struct.RwLock.html),
[`Condvar`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Condvar.html),
[`DeadlockInfo`](https://docs.rs/deloxide/1.1.0/deloxide/struct.DeadlockInfo.html),
and [`thread`](https://docs.rs/deloxide/1.1.0/deloxide/thread/index.html).

| Build | Additional API enabled | Runtime work added | Intended environment | Complete `Cargo.toml` dependency |
| --- | --- | --- | --- | --- |
| Default | Base API only | Active wait-for tracking for supported, tracked primitives; callbacks on findings. | Reproductions and measured normal deployments. | `deloxide = "1.1.0"` |
| `logging-and-visualization` | [`Deloxide::with_log`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.with_log), [`no_logging`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.no_logging), [`showcase`](https://docs.rs/deloxide/1.1.0/deloxide/fn.showcase.html), and [`showcase_this`](https://docs.rs/deloxide/1.1.0/deloxide/fn.showcase_this.html). | Event queue, serialization, asynchronous log writer, and file I/O. | Incident capture and local investigation. | `deloxide = { version = "1.1.0", features = ["logging-and-visualization"] }` |
| `lock-order-graph` | [`with_lock_order_checking`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.with_lock_order_checking) and [`no_lock_order_checking`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.no_lock_order_checking). | Historical lock-order edges and cycle checks in addition to active tracking. | Development and CI. | `deloxide = { version = "1.1.0", features = ["lock-order-graph"] }` |
| `stress-test` | [`StressConfig`](https://docs.rs/deloxide/1.1.0/deloxide/struct.StressConfig.html), [`StressMode`](https://docs.rs/deloxide/1.1.0/deloxide/enum.StressMode.html), [`with_random_stress`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.with_random_stress), [`with_component_stress`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.with_component_stress), and [`with_stress_config`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.with_stress_config). | Configured delays/preemption behavior around lock attempts; slower, less deterministic execution. | Focused tests and reproductions only. | `deloxide = { version = "1.1.0", features = ["stress-test"] }` |
| All optional features | All APIs above. | Logging, historical order tracking, and optional stress behavior when selected by the builder. | Comprehensive local/CI diagnosis, after measuring the combined cost. | `deloxide = { version = "1.1.0", features = ["logging-and-visualization", "lock-order-graph", "stress-test"] }` |

Use one dependency line in the application's `Cargo.toml`; the cells above are
complete alternatives, not lines to combine.

## Builder defaults follow compiled features

[`Deloxide::new`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.new)
always supplies a callback that panics with the report unless you replace it with
[`callback`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.callback).
When `logging-and-visualization` is compiled, it enables logging by default with
the path `deloxide.log`; change that path with `with_log`, or disable logging for
the initial configuration with `no_logging`. When `lock-order-graph` is compiled,
lock-order checking is enabled by default; make the initial policy explicit with
`with_lock_order_checking`, or use `no_lock_order_checking` for a controlled
baseline. Stress compilation alone does not add delays: select random or
component stress with its corresponding builder method.

```rust,no_run
# extern crate deloxide;
#[cfg(feature = "logging-and-visualization")]
{
    use deloxide::Deloxide;

    Deloxide::new()
        .with_log("logs/deloxide_{timestamp}.log")
        .callback(|report| eprintln!("{report:?}"))
        .start()
        .expect("logging detector initialization");
}
# #[cfg(not(feature = "logging-and-visualization"))]
# {
#     let _ = "this configuration needs the logging-and-visualization feature";
# }
```

```rust,no_run
# extern crate deloxide;
#[cfg(feature = "lock-order-graph")]
{
    use deloxide::{DeadlockSource, Deloxide};

    Deloxide::new()
        .with_lock_order_checking()
        .callback(|report| match report.source {
            DeadlockSource::WaitForGraph => eprintln!("active cycle"),
            DeadlockSource::LockOrderViolation => eprintln!("potential order cycle"),
        })
        .start()
        .expect("order-checking detector initialization");
}
# #[cfg(not(feature = "lock-order-graph"))]
# {
#     let _ = "this configuration needs the lock-order-graph feature";
# }
```

Both builder calls are feature-gated at compile time. Do not put them behind only
a runtime `if`: a binary built without the feature has no such methods.

## Choose the evidence level deliberately

The `lock-order-graph` feature can report
[`DeadlockSource::LockOrderViolation`](https://docs.rs/deloxide/1.1.0/deloxide/enum.DeadlockSource.html#variant.LockOrderViolation)
when observed acquisitions close a historical order cycle. It is useful early in
development, but it is not evidence that threads are blocked now. The base
wait-for detector's
[`DeadlockSource::WaitForGraph`](https://docs.rs/deloxide/1.1.0/deloxide/enum.DeadlockSource.html#variant.WaitForGraph)
is the active, validated-cycle finding. Keep those response paths distinct even
when all features are compiled.

Stress mode changes scheduling to make a suspected bug easier to reproduce; it
does not turn a potential order warning into a confirmed deadlock. Logging adds
history for the supported events it receives, not a complete trace of every
thread and primitive in the process. See [Choosing a Mode](../choosing-a-mode.md)
for operational trade-offs, [Finding Inconsistent Lock Order](../diagnosis/lock-order.md)
for potential findings, and [Stress Test a Suspected Race](../diagnosis/stress-testing.md)
for test-only stress workflows.

Cargo features are fixed when the application is built. Choose one runtime
builder configuration and start it before instrumented work. Repeated `start()`
calls are accepted, but their effects are asymmetric: the first successfully
installed callback and global logger win; an enabled lock-order graph is created
or replaced, while a later disabled setting does not remove an existing graph;
and stress mode/configuration is overwritten on each start. Existing ownership
and wait state is not reset coherently with those changes. Repeated starts are
therefore partial, unsupported reconfiguration, not a reliable reset or toggle.
Use separate processes for clean configurations; see [Manage Lifecycle and
Callbacks](lifecycle.md) for the exact behavior.

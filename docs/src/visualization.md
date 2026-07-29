# Logging and Visualization

An active report tells you which tracked threads and locks formed a cycle. Logging
adds the sequence that led there: thread lifecycle events, lock attempts,
acquisitions and releases, condition-variable events, and a terminal deadlock
record. Use it when an incident needs reconstruction, not as a substitute for a
small active report callback.

## Enable the logging pipeline

```toml
[dependencies]
deloxide = { version = "1.1", features = ["logging-and-visualization"] }
```

With that feature, `Deloxide::new().start()` writes to `deloxide.log` by default.
Choose a path explicitly when each run needs its own retained artifact:

```rust,ignore
use deloxide::Deloxide;

Deloxide::new()
    .with_log("logs/deadlock_{timestamp}.log")
    .start()
    .expect("detector initialization");
```

`{timestamp}` is replaced when the logger starts. Deloxide creates missing parent
directories, opens the selected file for writing, and truncates an existing file at
that path. Treat the path as application-owned incident storage: pick a location
with appropriate permissions and retention, and do not have multiple processes
reuse one filename.

## What is in the file

Tracked wrappers send event records to Deloxide's asynchronous logger. Its writer
thread serializes line-delimited JSON to the local log file; a detected deadlock is
written as a terminal record. Sequence numbers and timestamps let the viewer order
the tracked activity. The browser view reconstructs the graph from those events,
rather than reading a saved internal graph snapshot.

```text
tracked thread and lock events → logger channel → local log file
deadlock report                 → terminal log record
local log file                  → compact URL payload → browser viewer
```

The final arrow matters for privacy. Current `showcase` code reads the local file,
compresses and encodes it into a URL parameter, then opens
`https://deloxide.vercel.app/` in the default browser. It does **not** provide a
local-only or no-upload guarantee. Treat a log as data that will be carried to that
site in the browser request; review it for identifiers or other sensitive context
before opening it outside your approved environment.

## Open a visualization

Use `showcase` when you have a retained file, or `showcase_this` when the current
process owns the active logger. `showcase_this` first flushes pending records and
then obtains the active path; `showcase` simply reads the path you supply.
See the API entries for
[`Deloxide::with_log`](https://docs.rs/deloxide/1.1.0/deloxide/struct.Deloxide.html#method.with_log),
[`showcase`](https://docs.rs/deloxide/1.1.0/deloxide/fn.showcase.html), and
[`showcase_this`](https://docs.rs/deloxide/1.1.0/deloxide/fn.showcase_this.html) for
their exact signatures and error contracts.

```rust,ignore
use deloxide::{showcase, showcase_this};

showcase("logs/deadlock_20260729_120000.log")
    .expect("open retained log in the browser");
showcase_this().expect("flush and open the current log");
```

The command-line wrapper accepts the same path:

```console
cargo run --features logging-and-visualization --bin deloxide -- \
  logs/deadlock_20260729_120000.log
```

All three operations can fail when the log cannot be read or parsed, when a current
logger was never initialized, or when the system cannot open a browser. Avoid
launching a browser from a hot callback. A callback runs on Deloxide's dispatcher
thread, so keep it bounded—record the report or signal your incident handler, then
open the retained log from a controlled path.

## Read the view back into code

![Visualization of a Deloxide timeline and dependency graph showing thread and lock activity](assets/visualization.png)

Use the timeline to find the acquisition and wait events immediately before the
terminal report. Use the dependency graph to follow each waiting thread to the lock
and incompatible owner that complete the cycle. Then return to the code that owns
those wrappers and compare the acquisition order on every path. Deloxide IDs help
you correlate the report and the log, but they are not source locations; attach
application names or request context around the wrappers if you need a direct bridge
to a request or code path.

If the file is absent, confirm the feature is enabled, the process can create the
chosen directory, and initialization succeeded before tracked work began. If it is
incomplete, use `showcase_this` to flush the active logger before reading it, and
avoid terminating the process before the writer can handle its queued records. If
the viewer cannot open, retain the local file and inspect the browser-launch error;
the detector can still deliver its callback report without the viewer. More symptom
checks are in [Troubleshoot Missing Evidence](diagnosis/troubleshooting.md).

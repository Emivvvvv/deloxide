# Logging and visualization

The callback tells you which threads and locks formed a cycle. Logging adds the
events that led there.

Enable it with:

```toml
[dependencies]
deloxide = { version = "1.1", features = ["logging-and-visualization"] }
```

Choose a log path at startup:

```rust,no_run
# extern crate deloxide;
use deloxide::Deloxide;

Deloxide::new()
    .with_log("logs/deloxide_{timestamp}.log")
    .callback(|report| eprintln!("{report:#?}"))
    .start()
    .expect("start Deloxide");
```

The file records thread lifecycle, lock attempts, acquisitions, releases,
condition-variable events, and the final finding.

![Deloxide timeline and thread-lock graph](assets/visualization.png)

Open a retained log with:

```rust,no_run
# extern crate deloxide;
deloxide::showcase("logs/deloxide_20260729_120000.log")
    .expect("open visualization");
```

Or call `showcase_this()` to flush and open the current process's active log.

The viewer encodes the log into a URL and opens
`https://deloxide.vercel.app/`. Review the log for sensitive identifiers before
using it. The logger queue is currently unbounded, so monitor memory and file
growth during long captures.

Use visualization for reconstruction, not as a replacement for the small
structured callback report.

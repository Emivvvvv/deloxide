# Evaluation record — 2026-07-29

## Purpose and claim boundary

This record revalidates the documentation's focused performance and deadlock
manifestation evidence against Deloxide commit
`baa9e89ef87191d25832b4ecf567c5dd26b4a6ae` (the Condvar-reacquisition fix).
It records measurements from one machine and one toolchain; it does not claim a
universal speedup, a stable ranking on other hardware, or absence of false
positives outside the finite controls described below.

The separate evaluation checkout was commit
`c97225d1c2332fcec84e96b5bf81ca535bea6c48`. It was copied to a temporary
directory. Cargo resolved Deloxide to
`/Users/emivvvvv/Documents/GitHub/deloxide/.worktrees/docs-redme-book-redesign`
(`cargo tree -i deloxide` reported version 1.1.0 from that path); the original
`/Users/emivvvvv/Documents/GitHub/deloxide-evaluation` checkout was not changed.

## Environment

The complete capture is in
[`results/2026-07-29-environment.txt`](results/2026-07-29-environment.txt): Apple
M1 Pro, 16 GiB RAM, Darwin 24.6.0/aarch64, Rust 1.90.0-nightly
(`bdaba05a9`, 2025-06-27), and Cargo 1.90.0-nightly (`409fed7d`). Criterion was
0.7.0. The benchmark worktree had only parallel documentation/example/script
dirtiness; the README width edit was unrelated to benchmark code, and no runtime
source changed for these runs.

## Focused Criterion results

[`results/2026-07-29-fast-path.csv`](results/2026-07-29-fast-path.csv) contains
the five raw summaries using this schema:

```text
case,implementation,median,unit,confidence_low,confidence_high,samples,features,commit
```

Command:

```sh
cargo bench --bench fast_path -- --noplot
```

The harness used the default feature set (`default=[]`), 30 samples, a 1-second
warmup, and a 2-second measurement window. It reports 95% bootstrap confidence
intervals. The uncontended Mutex median was 9.12 ns (9.07–9.22 ns), parking_lot
was 10.28 ns (9.95–10.50 ns), the RwLock read median was 58.07 ns
(54.06–62.78 ns), the RwLock write median was 9.17 ns (9.08–9.21 ns), and the
two-thread handoff median was 37.89 µs (37.12–39.03 µs). The broad RwLock-read
interval and handoff spread are noise warnings, not evidence for fine-grained
rankings.

## Manifestation measurement

[`results/2026-07-29-manifestation.csv`](results/2026-07-29-manifestation.csv)
uses:

```text
scenario,mode,detected,runs,rate_percent,commit,features
```

The intended command was first attempted exactly as specified:

```sh
./run_tests.sh detection -n 1000 -q \
  -f deloxide,deloxide_random_default,deloxide_aggressive,deloxide_component_based_delays
```

The supplied script runs `cargo run` for every sample. After 43 of 20,000 rows
in roughly 105 seconds, that wrapper projected to hours, so the temporary-copy
run was stopped. The same four feature-specific release binaries were then built
once, copied aside, and run directly with the script's per-run `HEISENBUG_SEED`
formula; a 2,000-run, 100-per-combination pilot completed in 20 seconds. The
full direct equivalent then completed all 20,000 rows (five scenarios × four
modes × 1,000 iterations), followed by `python3 analysis/analyze_detection_rate.py`.
This changes only repeated Cargo invocation, not the binary, features, scenario,
working directory, CSV format, or seed mechanism.

`detected` counts only the active wait-for detector result in the scenario CSV.
No `LockOrderViolation` mode or finding is included. Rates describe how often
these deliberately deadlocking schedules manifested on this machine; stress
modes are testing tools, not production-default performance claims.

## Correctness controls

The temporary copy completed:

```sh
./run_tests.sh guaranteed -n 10 -q -f deloxide
```

The five guaranteed scenarios produced 10/10 active detections each, with no
timeouts. The requested false-positive wrapper,
`./run_tests.sh false-positive -n 100 -q -f deloxide`, was stopped after its
first six completed samples because it again invoked Cargo per sample and one
case alone took 25.0688 seconds. Running
`python3 analysis/verify_false_positives.py` over those six completed, distinct
traditional false-positive cases reported zero flags. That is a small,
incomplete finite observation—not a general no-false-positive guarantee.

## Application-level comparison

[`results/2026-07-29-raytracer.csv`](results/2026-07-29-raytracer.csv) contains
one row per run:

```text
configuration,resolution,run,frame_ms,locks,peak_rss_mb,commit,features
```

The temporary raytracer checkout generated exactly one 1920×1080 scene at 128
samples per pixel and depth 50, then built and directly timed ten runs each for
`use_std`, `parking_lot_deadlock`, and `deloxide`. Frame times were 23,478.8 ms,
21,625.0 ms, and 23,754.5 ms respectively (arithmetic means of the ten recorded
runs); all rows record 129,600 lock acquisitions. This deliberately excludes
`no_deadlocks` and stress modes, so it compares the intended application
configurations without treating one application result as a universal speedup.

The old README at the evaluated Deloxide commit described a methodology but did
not publish a directly comparable numeric result. Historical evaluation output
in the separate repository used different provenance and aggregate-only values,
so it is retained as historical context rather than compared as a current
claim. The current raw CSV is the authoritative source for this documentation.

## Regeneration

The deterministic SVG assets are generated from the fast-path and manifestation
CSVs with:

```sh
python3 scripts/generate_doc_charts.py
python3 scripts/generate_doc_charts.py --check
```

The check mode regenerates in memory and fails if either committed SVG differs.

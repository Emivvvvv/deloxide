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
The disposable checkout and dependency resolution were created exactly with:

```sh
deloxide_worktree=/Users/emivvvvv/Documents/GitHub/deloxide/.worktrees/docs-redme-book-redesign
evaluation_tmp="$(mktemp -d /tmp/deloxide-evaluation-2026-07-29.XXXXXX)"
cp -R /Users/emivvvvv/Documents/GitHub/deloxide-evaluation/. "$evaluation_tmp/"
ln -s "$deloxide_worktree" "$evaluation_tmp/deloxide"
cd "$evaluation_tmp/deloxide-deadlock-tests"
cargo tree \
  --config "paths = [\"$deloxide_worktree\"]" \
  --no-default-features --features deloxide -i deloxide
```

The deadlock-suite manifest's `deloxide = { path = "../deloxide" }` dependency
therefore resolved through the temporary symlink. The explicit Cargo `paths`
override was also passed for crates.io-style resolution, including every
raytracer Cargo command below.

## Environment

The complete capture is in
[`results/2026-07-29-environment.txt`](results/2026-07-29-environment.txt): Apple
M1 Pro, 16 GiB RAM, Darwin 24.6.0/aarch64, Rust 1.90.0-nightly
(`bdaba05a9`, 2025-06-27), and Cargo 1.90.0-nightly (`409fed7d`). Criterion was
0.7.0. The measured runtime sources were exactly
`baa9e89ef87191d25832b4ecf567c5dd26b4a6ae`. The only `Cargo.toml` difference
was the four-line `[[example]]` target for `diagnose_deadlock`, later committed
unchanged as `5db2db52889ff35f8d944f58b48196fe9b8b7864`; it changed no dependency,
feature, benchmark target, profile, or runtime source. The untracked
`scripts/tests/` files and parallel README width edit were documentation
validation work and were not compiled into the benchmark. Reproduction means
checking out `baa9e89`, applying only that `[[example]]` block to `Cargo.toml`,
and leaving all runtime sources untouched. The environment capture records the
exact block and pre-benchmark `git status`.

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
run was stopped. Four feature builds were then performed once. From each build,
the five selected scenario executables were copied immediately, producing 20
distinct executable paths—not four binaries:

```sh
cd "$evaluation_tmp/deloxide-deadlock-tests"
mkdir -p /tmp/deloxide-direct-bins
modes=(
  deloxide
  deloxide_random_default
  deloxide_aggressive
  deloxide_component_based_delays
)
scenarios=(
  two_lock
  three_lock_cycle
  dining_philosophers
  rwlock_deadlock
  five_lock_cycle
)

for mode_name in "${modes[@]}"; do
  cargo build --release --no-default-features \
    --features "$mode_name" --bins --quiet
  mkdir -p "/tmp/deloxide-direct-bins/$mode_name"
  for scenario_name in "${scenarios[@]}"; do
    cp "target/release/$scenario_name" \
      "/tmp/deloxide-direct-bins/$mode_name/$scenario_name"
  done
done
```

The run remained in
`$evaluation_tmp/deloxide-deadlock-tests`, because each executable appends its
row to `deadlock_tests/<scenario>_<mode>.csv` relative to that directory. The
exact direct loop, including the macOS fallback seed formula used by
`run_tests.sh`, was:

```sh
find deadlock_tests -type f -name '*.csv' -delete
for iteration_number in $(seq 1 1000); do
  for scenario_name in "${scenarios[@]}"; do
    for mode_name in "${modes[@]}"; do
      HEISENBUG_SEED="$(($(date +%s) * 1000000 + RANDOM * 1000 + RANDOM))" \
        "/tmp/deloxide-direct-bins/$mode_name/$scenario_name" \
        >> /tmp/deloxide-direct-manifestation-1000.log 2>&1 || true
    done
  done
done
find deadlock_tests -type f -name '*.csv' -exec wc -l {} + | tail -n 1
cd analysis
python3 analyze_detection_rate.py
```

The final count was 20,000 rows. The `|| true` matches `run_tests.sh`, whose
detector callback intentionally panics after recording a detection. A discarded
2,000-run, 100-per-combination pilot completed in 20 seconds before the final
run. The normalized documentation CSV was collected from the final files with:

```python
import csv
from pathlib import Path

root = Path("../deadlock_tests")
commit = "baa9e89ef87191d25832b4ecf567c5dd26b4a6ae"
print("scenario,mode,detected,runs,rate_percent,commit,features")
for scenario in [
    "two_lock", "three_lock_cycle", "dining_philosophers",
    "rwlock_deadlock", "five_lock_cycle",
]:
    for mode in [
        "deloxide", "deloxide_random_default", "deloxide_aggressive",
        "deloxide_component_based_delays",
    ]:
        rows = list(csv.reader((root / f"{scenario}_{mode}.csv").open()))
        detected = sum(row[0].strip().lower() == "true" for row in rows)
        rate = detected / len(rows) * 100
        print(
            f"{scenario},{mode},{detected},{len(rows)},{rate:.1f},"
            f"{commit},{mode}"
        )
```

Prebuilding changes only repeated Cargo invocation, not the release executable,
features, scenario, working directory, CSV format, or seed mechanism.

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
`use_std`, `parking_lot_deadlock`, and `deloxide`. The exact scene and dependency
commands were:

```sh
cd "$evaluation_tmp/rust-raytracer-modified/raytracer"
config_arg="paths = [\"$deloxide_worktree\"]"
cargo tree --config "$config_arg" --no-default-features \
  --features deloxide -i deloxide
cargo run --config "$config_arg" --release --bin generate_scene -- \
  1920 1080 128 50 > /tmp/deloxide-raytracer-1920x1080.json
```

The direct build, timing, and telemetry collection loop was:

```sh
: > /tmp/deloxide-raytracer-2026-07-29.log
for config_name in use_std parking_lot_deadlock deloxide; do
  cargo build --config "$config_arg" --release --no-default-features \
    --features "$config_name" --quiet \
    >> /tmp/deloxide-raytracer-2026-07-29.log 2>&1
  for run_number in $(seq 1 10); do
    run_output=$(
      /usr/bin/time -l ./target/release/raytracer \
        /tmp/deloxide-raytracer-1920x1080.json \
        "/tmp/deloxide-raytracer-${config_name}-${run_number}.png" 2>&1
    )
    frame_ms=$(printf '%s\n' "$run_output" |
      awk '/Frame time:/ {sub(/.*Frame time: /, ""); sub(/ms.*/, ""); print; exit}')
    locks=$(printf '%s\n' "$run_output" |
      awk '/Total Lock Acquisitions:/ {
        sub(/.*Total Lock Acquisitions: /, ""); print; exit
      }')
    rss_bytes=$(printf '%s\n' "$run_output" |
      awk '/maximum resident set size/ {print $1; exit}')
    printf 'RESULT,%s,%s,%s,%s,%s\n' \
      "$config_name" "$run_number" "$frame_ms" "$locks" "$rss_bytes" |
      tee -a /tmp/deloxide-raytracer-2026-07-29.log
  done
done
```

`frame_ms` and `locks` came from the application output; peak RSS came from
macOS `/usr/bin/time -l` in bytes and was divided by 1,048,576 for the CSV's
MiB-valued `peak_rss_mb` column. The raw `RESULT` records were converted with:

```sh
awk -F, '
  BEGIN {
    print "configuration,resolution,run,frame_ms,locks,peak_rss_mb,commit,features"
  }
  /^RESULT/ {
    printf "%s,1920x1080,%s,%s,%s,%.2f,%s,%s\n",
      $2, $3, $4, $5, $6 / 1048576,
      "baa9e89ef87191d25832b4ecf567c5dd26b4a6ae", $2
  }
' /tmp/deloxide-raytracer-2026-07-29.log
```

Frame-time means were 23,478.8 ms, 21,625.0 ms, and 23,754.5 ms respectively;
all rows record 129,600 lock acquisitions. This deliberately excludes
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

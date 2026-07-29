# Contributing

Use a recent Rust toolchain with edition 2024 support.

```sh
cargo fmt --all -- --check
cargo clippy --lib --bins --examples --all-features -- -D warnings
cargo test --lib
cargo check --examples
scripts/check_docs.sh
```

Add one focused regression for a concurrency fix and verify that it fails for the
expected reason before changing production code. Prefer barriers/channels to sleeps
and isolate intentional deadlocks behind a process watchdog.

Run the focused Criterion cases for changes to lock acquisition or release. The full
evaluation suite is not required for unrelated changes.

Keep commits small and use compact subjects such as `fix(rwlock)`, `docs(readme)`,
`book(internals)`, and `ci(checks)`.

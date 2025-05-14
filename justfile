# ─────────────────────────────────────────────
# Default tasks
# ─────────────────────────────────────────────
default: test

# Run all tests
test:
    cd deloxide-tests && cargo test

# Unit tests only
test-unit:
    cd deloxide-tests && cargo test unit_tests

# Scenario tests only
test-scenarios:
    cd deloxide-tests && cargo test scenarios


# ─────────────────────────────────────────────
# Benchmark groups
# ─────────────────────────────────────────────
# Run the whole suite from scratch:
#   1) wipe previous Criterion data
#   2) baseline          (default)
#   3) detector          (default)
bench-all:
    cd deloxide-tests && rm -rf target/criterion
    just bench-baseline-all
    just bench-detector-all


# ── 1. Baseline only ─────────────────────────
bench-baseline-all:
    cd deloxide-tests && \
    cargo bench --no-default-features --bench single_mutex_baseline && \
    cargo bench --no-default-features --bench ordered_locking_baseline && \
    cargo bench --no-default-features --bench reader_writer_baseline  && \
    cargo bench --no-default-features --bench hierarchical_locking_baseline  && \
    cargo bench --no-default-features --bench producer_consumer_baseline


# ── 2. Detector (no log) ─────────────────────
bench-detector-all:
    cd deloxide-tests && \
    cargo bench --no-default-features --features detector --bench single_mutex_detector  && \
    cargo bench --no-default-features --features detector --bench ordered_locking_detector  && \
    cargo bench --no-default-features --features detector --bench reader_writer_detector  && \
    cargo bench --no-default-features --features detector --bench hierarchical_locking_detector  && \
    cargo bench --no-default-features --features detector --bench producer_consumer_detector

# ─────────────────────────────────────────────
# Misc utilities
# ─────────────────────────────────────────────
clean:
    cargo clean
    cd deloxide-tests && cargo clean

build:
    cargo build --release
    cd deloxide-tests && cargo build --release

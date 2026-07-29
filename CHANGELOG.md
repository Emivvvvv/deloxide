# Changelog

All notable user-visible changes are documented here.

## [1.1.0] - Unreleased

### Added

- A complete mdBook user manual and focused microbenchmark harness.
- Mode-aware wait metadata and ownership-handoff edge refresh.

### Changed

- Active cycles validate every edge against current incompatible ownership.
- RwLock reader ownership is counted per thread.
- Blocking read-to-write upgrades are reported as same-thread deadlocks.

### Fixed

- Stale direct wait edges during ownership handoff.
- Fast-writer visibility, nonblocking read cleanup, and shared-read filtering.
- Condvar timeout cleanup and notification outside a mutex.
- Multiple simultaneous C RwLock guards on one thread.
- Callback dispatcher termination after a callback panic.

## [1.0.0]

- Initial Rust and C release with active wait-for detection, optional lock-order
  analysis, stress scheduling, logging, and visualization.

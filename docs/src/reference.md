# Reference and Compatibility

- [Rust API on docs.rs](https://docs.rs/deloxide)
- [C header](https://github.com/Emivvvvv/deloxide/blob/main/include/deloxide.h)
- [Source repository](https://github.com/Emivvvvv/deloxide)
- [Issue tracker](https://github.com/Emivvvvv/deloxide/issues)

The public Rust and C signatures remain compatible across the correctness-hardening
release. Behavioral fixes may reject stale cycles, report blocking RwLock upgrades
earlier, and return an error for an unmatched C RwLock unlock.

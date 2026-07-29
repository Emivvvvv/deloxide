# Installation

For active detection with the optimized default build:

```toml
[dependencies]
deloxide = "1.0"
```

Optional Cargo features are independent:

```toml
deloxide = { version = "1.0", features = [
  "logging-and-visualization",
  "lock-order-graph",
  "stress-test",
] }
```

The crate requires the Rust edition and toolchain supported by the published crate
metadata. C consumers build the Rust library and include `include/deloxide.h`; see
the [C Guide](c-guide.md).

# C Guide

Build the Rust library, include `include/deloxide.h`, and link the produced Deloxide
library plus platform threading dependencies.

```sh
cargo build --release
cc -Iinclude program.c -Ltarget/release -ldeloxide -pthread
```

Every pointer returned by a `deloxide_create_*` function must be destroyed exactly
once after all users and guards are gone. Lock and unlock calls must be paired on the
same thread. One thread may hold guards for multiple distinct RwLocks; storage is
keyed by the lock pointer.

The callback receives serialized deadlock information. Copy data needed after the
callback returns.

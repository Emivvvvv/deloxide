# C integration

Rust is Deloxide's primary interface, but the same tracked detector and
primitives are available through `include/deloxide.h`.

Build the library with:

```console
cargo build --release --features c-api
```

Add `logging-and-visualization`, `lock-order-graph`, or `stress-test` only when
the C application needs those capabilities.

## Smallest complete program

```c
#include "deloxide.h"
#include <stdio.h>

static void on_deadlock(const char *json) {
    fprintf(stderr, "Deloxide: %s\n", json);
}

int main(void) {
    if (deloxide_init(NULL, on_deadlock) != 0) {
        return 1;
    }

    void *mutex = deloxide_create_mutex();
    if (mutex == NULL || deloxide_lock_mutex(mutex) != 0) {
        return 1;
    }

    /* protected work */

    deloxide_unlock_mutex(mutex);
    deloxide_destroy_mutex(mutex);
    return 0;
}
```

Pass a log path as the first `deloxide_init` argument when the library was built
with `logging-and-visualization`. Without that feature, a non-null log path
returns `-3`. A null callback uses the library's default behavior.

## Available primitives

The header provides opaque handles for:

- mutex creation, lock, try-lock, unlock, and destruction;
- RwLock read/write acquisition, try operations, release, and destruction; and
- condition-variable wait, timed wait, notification, and destruction.

Every function returns a documented status code or pointer. Check it—C cannot
use Rust's type system to prevent a bad handle, mismatched guard, or invalid
lifecycle.

## Threads

Calls through Deloxide locks are observed regardless of how the native thread
was created. Register thread start/exit when you also want lifecycle and parent
information in logs. The header contains POSIX and Windows helpers plus manual
registration functions for another threading runtime.

## Linking

Link against the produced `deloxide` static or dynamic library and the normal
platform libraries required by Rust output. The exact filenames and flags vary
by target, so treat the generated artifact and
[`include/deloxide.h`](../../include/deloxide.h) as the source of truth.

Keep the callback short, initialize once before creating tracked objects, and
destroy locks only after all users have stopped.

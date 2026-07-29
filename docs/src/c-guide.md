# C guide

Rust is Deloxide's primary interface, but C applications can use the same
detector and tracked synchronization through `include/deloxide.h`.

## Build and link

Build the library and C API:

```console
cargo build --release --features c-api
```

Add other features when needed:

```console
cargo build --release --features \
  c-api,logging-and-visualization,lock-order-graph,stress-test
```

Include `include/deloxide.h` and link the produced static or dynamic `deloxide`
library. Exact filenames and platform libraries depend on the target. The
repository's [`c_examples/basic_mutex.c`](../../c_examples/basic_mutex.c) is the
smallest buildable example.

## Initialization and callback

Initialize once before creating tracked objects:

```c
#include "deloxide.h"
#include <stdio.h>

static void on_deadlock(const char *json) {
    fprintf(stderr, "Deloxide report: %s\n", json);
}

int main(void) {
    int rc = deloxide_init(NULL, on_deadlock);
    if (rc != 0) {
        fprintf(stderr, "deloxide_init failed: %d\n", rc);
        return 1;
    }

    /* create locks and threads */
    return 0;
}
```

The callback receives a borrowed NUL-terminated JSON string. Copy it if another
thread must retain it; do not free it or keep the pointer after the callback
returns. Keep callback work bounded.

Initialization returns `0` on success and `1` if it has already run. Invalid log
paths and logger failures use negative codes. Passing a non-null log path without
the logging feature returns `-3`; the public header currently omits that code.

## Mutex

```c
void *mutex = deloxide_create_mutex();
if (mutex == NULL) return 1;

if (deloxide_lock_mutex(mutex) != 0) return 1;
/* protected work */
if (deloxide_unlock_mutex(mutex) != 0) return 1;

deloxide_destroy_mutex(mutex);
```

`LOCK_MUTEX(mutex)` and `UNLOCK_MUTEX(mutex)` provide checked convenience macros
that terminate on failure. Destroy a mutex only after every thread has stopped
using it.

## RwLock

```c
void *state = deloxide_create_rwlock();
if (state == NULL) return 1;

if (deloxide_rw_lock_read(state) != 0) return 1;
/* read shared state */
if (deloxide_rw_unlock_read(state) != 0) return 1;

if (deloxide_rw_lock_write(state) != 0) return 1;
/* update shared state */
if (deloxide_rw_unlock_write(state) != 0) return 1;

deloxide_destroy_rwlock(state);
```

The `RWLOCK_READ`, `RWUNLOCK_READ`, `RWLOCK_WRITE`, and `RWUNLOCK_WRITE` macros
are the shorter checked form. A thread may hold read guards for different RwLocks,
but it must release each matching guard correctly.

## Condition variables

A Deloxide condition variable waits with a Deloxide mutex:

```c
void *mutex = deloxide_create_mutex();
void *ready = deloxide_create_condvar();

if (deloxide_lock_mutex(mutex) != 0) return 1;
while (!predicate_is_ready()) {
    int rc = deloxide_condvar_wait(ready, mutex);
    if (rc != 0) return 1;
}
if (deloxide_unlock_mutex(mutex) != 0) return 1;

deloxide_destroy_condvar(ready);
deloxide_destroy_mutex(mutex);
```

`deloxide_condvar_wait_timeout` returns `1` when the timeout expires and `0` when
notified. Negative values indicate invalid handles, a mutex not held by the
caller, or another wait failure. Notify with
`deloxide_condvar_notify_one` or `deloxide_condvar_notify_all`.

## Tracked threads

Any native thread using a Deloxide lock contributes synchronization events.
Register lifecycle events when logs should also show the thread relationship:

```c
uintptr_t tid = deloxide_get_thread_id();
deloxide_register_thread_spawn(tid, parent_tid);

/* thread work */

deloxide_register_thread_exit(tid);
```

On POSIX, `DEFINE_TRACKED_THREAD(worker)` and
`CREATE_TRACKED_THREAD(thread, worker, arg)` wrap this protocol around
`pthread_create`. Those macros are not available on Windows; call the manual
registration functions from the Windows thread entry point.

## Logging, visualization, and stress

With `logging-and-visualization`, pass a log path to `deloxide_init`, flush it
with `deloxide_flush_logs`, and open it with `deloxide_showcase` or
`deloxide_showcase_current`.

With `stress-test`, C can enable random scheduling delays with
`deloxide_enable_random_stress`, enable targeted component delays with
`deloxide_enable_component_stress`, and return to normal scheduling with
`deloxide_disable_stress`.

The C header is the exact API reference. This chapter focuses on correct
lifecycle and common usage rather than duplicating every status-code comment.

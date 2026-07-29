# C Integration Guide

Deloxide is designed around its Rust wrappers. Use this C ABI when an existing C
program needs the same *tracked* mutex, reader/writer lock, and condition-variable
operations; it is not a replacement for the Rust integration guide. The
[public header](https://github.com/Emivvvvv/deloxide/blob/main/include/deloxide.h)
is the authority for every exported signature and return code.

## Build and link

The crate produces both a static library and a platform dynamic library:

| Platform | Static output | Dynamic output |
| --- | --- | --- |
| Linux | `target/release/libdeloxide.a` | `target/release/libdeloxide.so` |
| macOS | `target/release/libdeloxide.a` | `target/release/libdeloxide.dylib` |
| Windows | `target/release/deloxide.lib` | `target/release/deloxide.dll` |

Build first:

```sh
cargo build --release
```

The Linux command is the C smoke-test command in CI. It links the static library
explicitly, so the system libraries must follow it:

```sh
cc -Wall -Wextra -Werror -Iinclude c_examples/basic_mutex.c \
  target/release/libdeloxide.a -pthread -ldl -lm -o target/c-basic-mutex
target/c-basic-mutex
```

On macOS, link the dynamic library exactly as follows and make it discoverable for
the run:

```sh
cc -Wall -Wextra -Werror -Iinclude c_examples/basic_mutex.c \
  -Ltarget/release -ldeloxide -pthread -o target/c-basic-mutex
DYLD_LIBRARY_PATH=target/release target/c-basic-mutex
```

For a packaged release, install `include/deloxide.h` with the matching library
from that release. Do not mix a header from one release with a library from another.

## Initialize once, then keep callbacks small

Call `deloxide_init` before creating or using a tracked primitive. Its documented
results are `0` for initialization, `1` when another caller already initialized
the process, `-1` for a non-UTF-8 log path, and `-2` when logger initialization
fails. A current build also returns `-3` if a non-NULL log path requests optional
logging from a library built without `logging-and-visualization`. Treat any
negative result as a failed setup; a program that tolerates an already-initialized
process may accept `1` deliberately.

```c
int rc = deloxide_init(NULL, deadlock_callback);
if (rc != 0 && rc != 1) {
    return 1;
}
```

Pass `NULL` for `log_path` to disable logging. A non-NULL path requires a library
built with the `logging-and-visualization` Cargo feature. The header callback is
called with a NUL-terminated JSON string. It is a borrowed pointer: do not free it
or retain it after the callback returns. Copy the bytes during the callback if
another thread or a later operation needs the report. Keep the callback bounded;
queue or record the copied report instead of acquiring application locks or doing
slow I/O there.

The serialized report follows the Rust `DeadlockInfo` schema: `source`,
`thread_cycle`, `thread_waiting_for_locks`, optional `lock_order_cycle`,
`timestamp`, and `verification_request`. In normal C use, `source` is
`WaitForGraph`: an active, validated cycle. `LockOrderViolation` is a potential
ordering cycle, not proof that threads are currently blocked; the C initializer
does not enable that optional checker. See [Reference](reference.md#deadlock-report-schema)
for the field meanings.

## Lifecycle and error handling

Every `deloxide_create_*` call returns an opaque pointer or `NULL`. Check it,
operate only through the matching `deloxide_*` functions, and destroy it exactly
once after all threads have stopped using it. A destroy call invalidates the
pointer. A practical shutdown order is:

1. Stop creating work and notify any condition-variable waiters.
2. Join worker threads; each must release every held tracked lock first.
3. Destroy condition variables, RwLocks, and mutexes after their last use.

Use the direct functions when cleanup matters: their return values let the caller
take a controlled error path. The convenience macros described below print an
error and call `exit(1)`. Lock and unlock a given primitive on the same thread;
release an RwLock read guard before trying to acquire its writer mode rather than
attempting an upgrade.

### Mutex

Create one tracked mutex for each lock relationship Deloxide should observe.
Check both allocation and operation results, and unlock before destruction:

```c
void *mutex = deloxide_create_mutex();
if (mutex == NULL) {
    return 1;
}
if (deloxide_lock_mutex(mutex) != 0) {
    deloxide_destroy_mutex(mutex);
    return 1;
}

/* Critical section. */

if (deloxide_unlock_mutex(mutex) != 0) {
    /* The pointer remains application-owned; do not continue using it. */
    return 1;
}
deloxide_destroy_mutex(mutex);
```

### RwLock

Use the read and write pairs that match the acquisition. Each thread can hold
distinct tracked RwLocks concurrently, but must not acquire the same mode twice
without releasing it.

```c
void *rw = deloxide_create_rwlock();
if (rw == NULL) {
    return 1;
}
if (deloxide_rw_lock_read(rw) != 0) {
    deloxide_destroy_rwlock(rw);
    return 1;
}

/* Read the separately managed shared data. */

if (deloxide_rw_unlock_read(rw) != 0) {
    return 1;
}
deloxide_destroy_rwlock(rw);
```

Use `deloxide_rw_lock_write` and `deloxide_rw_unlock_write` for an exclusive
section. Do not destroy the RwLock while any reader or writer can still hold it.

### Condvar

A tracked condition variable is paired with a tracked mutex. Lock the mutex
before waiting; `deloxide_condvar_wait` atomically releases it while waiting and
reacquires it before returning. Test the application predicate in a loop, exactly
as with a native condition variable.

```c
while (!ready) {
    int wait_rc = deloxide_condvar_wait(condvar, mutex);
    if (wait_rc != 0) {
        /* mutex may need application-specific recovery or cleanup here */
        return 1;
    }
}
```

`deloxide_condvar_wait_timeout` returns `0` after notification and `1` on timeout;
negative values identify bad pointers, a mutex not held by the caller, or a failed
wait. A notifying thread calls `deloxide_condvar_notify_one` or
`deloxide_condvar_notify_all`, checks its return code, and destruction waits until
no thread can wait or notify through that pointer.

## Threads and POSIX helpers

`deloxide_get_thread_id()` returns Deloxide's unique `uintptr_t` identity for the
current thread; it is not a portable native-thread ID. For a thread created outside
the helper, capture the parent's Deloxide ID at creation time. In the child entry
function, obtain its ID, call `deloxide_register_thread_spawn(child, parent)`, and
call `deloxide_register_thread_exit(child)` immediately before every normal exit.
Use `0` as the parent ID when no parent relationship is available.

On POSIX, the header offers helpers around `pthread_create`:

```c
static void *worker(void *arg) {
    void *mutex = arg;
    LOCK_MUTEX(mutex);
    /* work */
    UNLOCK_MUTEX(mutex);
    return NULL;
}

DEFINE_TRACKED_THREAD(worker)

pthread_t thread;
CREATE_TRACKED_THREAD(thread, worker, mutex);
pthread_join(thread, NULL);
```

`DEFINE_TRACKED_THREAD` registers the child spawn and exit, while
`CREATE_TRACKED_THREAD` captures the parent ID and allocates its wrapper argument.
They are convenience macros: they do not expose allocation or `pthread_create`
failures to the caller, and the lock macros terminate the process on an operation
error. Use the direct functions and explicit registration when the application
needs recovery or its own thread abstraction.

On Windows the POSIX macros expand to no helpers. Use your native thread API and
the manual pattern instead: capture `parent = deloxide_get_thread_id()` before
creation, then in the new entry point call `child = deloxide_get_thread_id()`,
`deloxide_register_thread_spawn(child, parent)`, perform tracked operations, and
finally call `deloxide_register_thread_exit(child)`. The C API itself uses opaque
pointers and `uintptr_t`; only the `pthread_*` convenience layer is unavailable.

## Complete smoke example

This is the complete, intentionally small
[`c_examples/basic_mutex.c`](https://github.com/Emivvvvv/deloxide/blob/main/c_examples/basic_mutex.c)
used by the Linux C smoke job:

```c
#include "deloxide.h"

int main(void) {
    int initialized = deloxide_init(NULL, NULL);
    if (initialized != 0 && initialized != 1) {
        return 1;
    }

    void *mutex = deloxide_create_mutex();
    if (mutex == NULL || deloxide_lock_mutex(mutex) != 0) {
        return 2;
    }
    if (deloxide_unlock_mutex(mutex) != 0) {
        return 3;
    }
    deloxide_destroy_mutex(mutex);
    return 0;
}
```

For all remaining functions, macro definitions, and exact ABI types, read
[`include/deloxide.h`](https://github.com/Emivvvvv/deloxide/blob/main/include/deloxide.h).

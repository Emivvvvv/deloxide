# <img src='images/deloxide_logo_orange.png' height='25'> Deloxide - Cross-Language Deadlock Detector

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![License: Coffeeware](https://img.shields.io/badge/License-Coffeeware-brown.svg)](LICENSE)

Deloxide is a cross-language deadlock detection library with visualization support. It tracks mutex and reader-writer lock operations in multi-threaded applications to detect, report, and visualize potential deadlocks in real-time.

## Features

- **Real-time deadlock detection** - Detects deadlocks as they happen
- **Cross-language support** - Core implementation in Rust with C bindings
- **Thread & lock tracking** - Monitors relationships between threads and sync primitives (Mutex, RwLock, Condvar)
- **Visualization** - Web-based visualization of thread-lock relationships (see [example](https://deloxide.vercel.app/?logs=H4sIAAAAAAAC_03NvQrCMBSG4ZNEHKqDiOIipYMWi1ROo_2hm4uDo3hlkVyAk-DgjejmZNE7cLVdPcWkOD688H3qCT0NrHVdF_77M57cmAbOjaYnkmgb-S-mOYDRLCKxgdUGSEOrLYk7VudaHasLSbhGAdataxWDFs1DsGNaNCvlkVqzUt5JwjOqnP-_qk-NjX5yVQEHxYVSnCvBHp5EGYeYhZHcyyhfJrnExSpNsySdI-aIX6996tkRAQAA
  ) here)
- **Low overhead** - Designed to be lightweight for use in production systems
- **Easy integration** - Simple API for both Rust and C
- **Stress testing** - Optional feature to increase deadlock manifestation during testing

> [!NOTE]
> Cross-platform support: Rust API works on Windows, macOS, and Linux. The C API is POSIX-first and ships with pthread-based convenience macros for macOS/Linux; on Windows those macros are disabled (see below) but the core C functions are fully usable.

## Project Architecture

### How Deloxide Works

1. **Initialization**: The application initializes Deloxide with optional logging and callback settings.

2. **Resource Creation**: When threads, mutexes, and reader-writer locks are created, they're registered with the deadlock detector.

3. **Lock Operations**: When a thread attempts to acquire a lock:
   - The attempt is recorded by the detector
   - If the lock is already held by another thread, a "wait-for" edge is added
   - The detector checks for cycles in the "wait-for" graph
   - If a cycle is found, a deadlock is reported

4. **Deadlock Detection**: When a deadlock is detected, the callback is invoked with detailed information, including which threads are involved and which locks they're waiting for.

5. **Visualization**: The `showcase` function can be called (automatically in the callback or manually) to visualize the thread-lock interactions in a web browser.

### Core Components

1. **Deadlock Detection Engine**
   - Maintains a "wait-for" graph of thread dependencies
   - Detects cycles in the graph to identify potential deadlocks
   - Reports detected deadlocks through a configurable callback

2. **Resource Tracking**
   - Tracks threads and locks as resources with lifecycles
   - Manages parent-child relationships between threads
   - Automatically cleans up resources when threads exit

3. **Logging and Visualization**
   - Records thread-lock interactions to a log file
   - Processes logs for visualization in a web browser
   - Provides automatic visualization when deadlocks are detected

4. **Cross-Language Support**
   - Rust API with `Mutex`, `RwLock`, `Condvar`, and `thread` module
   - C API through FFI bindings in `deloxide.h`
   - Simple macros for C to handle common operations

5. **Stress Testing** (Optional with stress-testing feature)
   - Strategically delays threads to increase deadlock probability
   - Multiple strategies for different testing scenarios
   - Available as an opt-in feature for testing environments

## Quick Start

### Rust

Deloxide provides drop-in replacements for standard synchronization primitives with deadlock detection capabilities. All primitives wrap parking_lot implementations and add unique identifiers for tracking and visualization.

#### deloxide::thread

A drop-in replacement for `std::thread` that automatically tracks thread lifecycle events. All `std::thread` functions and types are available with added deadlock detection:

```rust
// All std::thread items are re-exported
pub use std::thread::{
    AccessError, JoinHandle, LocalKey, Result, Scope, 
    ScopedJoinHandle, Thread, ThreadId, available_parallelism, 
    current, panicking, park, park_timeout, sleep, yield_now,
};

// Custom spawn function with tracking
pub fn spawn<F, T>(f: F) -> JoinHandle<T> 
    where F: FnOnce() -> T + Send + 'static, T: Send + 'static;

// Custom Builder with tracking
pub struct Builder { /* ... */ }
```

Using tracked threads is identical to using `std::thread`:

```rust
use deloxide::thread;

// Spawn a tracked thread - exactly like std::thread::spawn
let handle = thread::spawn(|| {
    println!("Hello from tracked thread!");
    42
});

// All std::thread functions work
thread::yield_now();
thread::sleep(Duration::from_millis(100));
let current = thread::current();

// Builder pattern supported
let handle = thread::Builder::new()
    .name("worker".to_string())
    .stack_size(32 * 1024)
    .spawn(|| { /* ... */ })
    .unwrap();

// Join works the same way
let result = handle.join().unwrap();
assert_eq!(result, 42);
```

It automatically registers thread spawn/exit events for deadlock detection, visualization, and debugging purposes.

#### Deloxide::Mutex

A drop-in replacement for `std::sync::Mutex` (based on `parking_lot::Mutex`) with tracking:

```rust
pub struct Mutex<T> {
    id: LockId,
    inner: ParkingLotMutex<T>,
    creator_thread_id: ThreadId,
}

impl<T> Mutex<T> {
    pub fn new(data: T) -> Self;
    pub fn lock(&self) -> MutexGuard<'_, T>;
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>>;
    pub fn into_inner(self) -> T where T: Sized;
    pub fn get_mut(&mut self) -> &mut T;
    pub fn id(&self) -> LockId;
}

impl<T: Default> Default for Mutex<T> { /* ... */ }
impl<T> From<T> for Mutex<T> { /* ... */ }
```

All `std::sync::Mutex` methods are supported (except poisoning-related ones, as parking_lot doesn't use poisoning).

#### Deloxide::RwLock

A drop-in replacement for `std::sync::RwLock` (based on `parking_lot::RwLock`) with tracking:

```rust
pub struct RwLock<T> {
    id: LockId,
    inner: ParkingLotRwLock<T>,
    creator_thread_id: ThreadId,
}

impl<T> RwLock<T> {
    pub fn new(data: T) -> Self;
    pub fn read(&self) -> RwLockReadGuard<'_, T>;
    pub fn write(&self) -> RwLockWriteGuard<'_, T>;
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>>;
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>>;
    pub fn into_inner(self) -> T where T: Sized;
    pub fn get_mut(&mut self) -> &mut T;
    pub fn id(&self) -> LockId;
}

impl<T: Default> Default for RwLock<T> { /* ... */ }
impl<T> From<T> for RwLock<T> { /* ... */ }
```

All `std::sync::RwLock` methods are supported (except poisoning-related ones).

#### Deloxide::Condvar

A drop-in replacement for `std::sync::Condvar` (based on `parking_lot::Condvar`) with tracking:

```rust
pub struct Condvar {
    id: CondvarId,
    inner: ParkingLotCondvar,
}

impl Condvar {
    pub fn new() -> Self;
    pub fn wait<T>(&self, guard: &mut MutexGuard<'_, T>);
    pub fn wait_while<T, F>(&self, guard: &mut MutexGuard<'_, T>, condition: F)
        where F: FnMut(&mut T) -> bool;
    pub fn wait_timeout<T>(&self, guard: &mut MutexGuard<'_, T>, timeout: Duration) -> bool;
    pub fn wait_timeout_while<T, F>(&self, guard: &mut MutexGuard<'_, T>, 
        timeout: Duration, condition: F) -> bool
        where F: FnMut(&mut T) -> bool;
    pub fn notify_one(&self);
    pub fn notify_all(&self);
    pub fn id(&self) -> CondvarId;
}

impl Default for Condvar { /* ... */ }
```

All `std::sync::Condvar` methods are supported.

#### Complete Usage Example

Here's a comprehensive example demonstrating all Deloxide primitives in a single scenario:

```rust
use deloxide::{Deloxide, Mutex, RwLock, Condvar, thread};
use std::sync::Arc;
use std::time::Duration;

fn main() {
    // Initialize the detector with logging and visualization
    Deloxide::new()
        .with_log("deadlock_{timestamp}.json")
        .callback(|info| {
            eprintln!("Deadlock detected! Threads: {:?}", info.thread_cycle);
            deloxide::showcase_this().expect("Failed to launch visualization");
        })
        .start()
        .expect("Failed to initialize detector");

    // Create synchronization primitives
    let counter = Arc::new(Mutex::new(0));
    let shared_data = Arc::new(RwLock::new(vec![1, 2, 3, 4, 5]));
    let condition_pair = Arc::new((Mutex::new(false), Condvar::new()));

    // Example 1: Mutex operations with potential deadlock
    let counter_clone1 = Arc::clone(&counter);
    let counter_clone2 = Arc::clone(&counter);
    let mutex_b = Arc::new(Mutex::new("Resource B"));
    let mutex_b_clone = Arc::clone(&mutex_b);

    // Thread 1: Lock counter, then mutex_b (deadlock scenario)
    thread::spawn(move || {
        let _count = counter_clone1.lock();
        thread::sleep(Duration::from_millis(100));
        let _b = mutex_b.lock();
    });

    // Thread 2: Lock mutex_b, then counter (deadlock scenario) 
    thread::spawn(move || {
        let _b = mutex_b_clone.lock();
        thread::sleep(Duration::from_millis(100));
        let _count = counter_clone2.lock();
    });

    // Example 2: RwLock with multiple readers and upgrade deadlock
    let shared_clone1 = Arc::clone(&shared_data);
    let shared_clone2 = Arc::clone(&shared_data);

    // Multiple reader threads
    for i in 0..3 {
        let shared_clone = Arc::clone(&shared_data);
        thread::spawn(move || {
            let data = shared_clone.read();
            println!("Reader {}: {:?}", i, *data);
            thread::sleep(Duration::from_millis(50));
        });
    }

    // Writer thread attempting upgrade (potential deadlock)
    thread::spawn(move || {
        let _read_guard = shared_clone1.read();
        println!("Writer acquired read lock, attempting upgrade...");
        thread::sleep(Duration::from_millis(25));
        let _write_guard = shared_clone2.write(); // This will deadlock!
        println!("Writer acquired write lock");
    });

    // Example 3: Condvar usage with wait_while
    let pair_clone = Arc::clone(&condition_pair);
    
    // Waiter thread using convenient wait_while method
    thread::spawn(move || {
        let (mutex, condvar) = (&pair_clone.0, &pair_clone.1);
        let mut ready = mutex.lock();
        // wait_while is more convenient than a manual loop!
        condvar.wait_while(&mut ready, |ready| !*ready);
        println!("Condition met, thread proceeding");
    });

    // Notifier thread
    let pair_clone2 = Arc::clone(&condition_pair);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let (mutex, condvar) = (&pair_clone2.0, &pair_clone2.1);
        let mut ready = mutex.lock();
        *ready = true;
        condvar.notify_one();
        println!("Condition signaled");
    });

    // Let threads run and potentially detect deadlocks
    thread::sleep(Duration::from_secs(2));
    println!("Program completed");
}
```

### C

The C API provides a complete interface to Deloxide through `include/deloxide.h`. It uses opaque pointers and helper macros to simplify integration with existing C codebases.

#### Core C API Functions

```c
// Initialization
int deloxide_init(const char* log_path, void (*callback)(const char* json_info));
int deloxide_is_deadlock_detected();
void deloxide_reset_deadlock_flag();
int deloxide_is_logging_enabled();

// Mutex operations
void* deloxide_create_mutex(void);
void* deloxide_create_mutex_with_creator(uintptr_t creator_thread_id);
void deloxide_destroy_mutex(void* mutex);
int deloxide_lock_mutex(void* mutex);
int deloxide_unlock_mutex(void* mutex);
uintptr_t deloxide_get_mutex_creator(void* mutex);

// RwLock operations  
void* deloxide_create_rwlock(void);
void* deloxide_create_rwlock_with_creator(uintptr_t creator_thread_id);
void deloxide_destroy_rwlock(void* rwlock);
int deloxide_rw_lock_read(void* rwlock);
int deloxide_rw_unlock_read(void* rwlock);
int deloxide_rw_lock_write(void* rwlock);
int deloxide_rw_unlock_write(void* rwlock);
uintptr_t deloxide_get_rwlock_creator(void* rwlock);

// Condvar operations
void* deloxide_create_condvar(void);
void* deloxide_create_condvar_with_creator(uintptr_t creator_thread_id);
void deloxide_destroy_condvar(void* condvar);
int deloxide_condvar_wait(void* condvar, void* mutex);
int deloxide_condvar_wait_timeout(void* condvar, void* mutex, unsigned long timeout_ms);
int deloxide_condvar_notify_one(void* condvar);
int deloxide_condvar_notify_all(void* condvar);

// Thread tracking
int deloxide_register_thread_spawn(uintptr_t thread_id, uintptr_t parent_id);
int deloxide_register_thread_exit(uintptr_t thread_id);
uintptr_t deloxide_get_thread_id();

// Logging and visualization
int deloxide_flush_logs();
int deloxide_showcase(const char* log_path);
int deloxide_showcase_current();
```

#### Helper Macros

Deloxide provides convenient macros for easier usage:

```c
// Thread tracking macros
DEFINE_TRACKED_THREAD(fn_name)     // Define a tracked thread wrapper
CREATE_TRACKED_THREAD(thread, fn, arg)  // Create and start tracked thread

// Mutex macros
LOCK_MUTEX(mutex)                  // Lock with automatic tracking
UNLOCK_MUTEX(mutex)                // Unlock with automatic tracking

// RwLock macros
RWLOCK_READ(rwlock)                // Acquire read lock
RWLOCK_WRITE(rwlock)               // Acquire write lock  
RWUNLOCK_READ(rwlock)              // Release read lock
RWUNLOCK_WRITE(rwlock)             // Release write lock

// Condvar macros
CONDVAR_WAIT(condvar, mutex)       // Wait on condition variable
CONDVAR_NOTIFY_ONE(condvar)        // Signal one waiting thread
CONDVAR_NOTIFY_ALL(condvar)        // Signal all waiting threads
```

#### Complete C Usage Example

Here's a comprehensive example demonstrating all C API features in one program:

```c
#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include <unistd.h>
#include "deloxide.h"

// Global synchronization primitives
void* counter_mutex;
void* shared_rwlock;
void* condition_mutex;
void* condition_var;
int shared_counter = 0;
int condition_ready = 0;

void deadlock_callback(const char* json_info) {
    printf("=== DEADLOCK DETECTED ===\n%s\n", json_info);
    deloxide_showcase_current();
}

// Example 1: Mutex deadlock scenario
void* mutex_worker1(void* arg) {
    void** mutexes = (void**)arg;
    void* mutex_a = mutexes[0];
    void* mutex_b = mutexes[1];
    
    printf("Thread 1: Locking mutex A\n");
    LOCK_MUTEX(mutex_a);
    usleep(100000);  // 100ms delay
    
    printf("Thread 1: Trying to lock mutex B\n");
    LOCK_MUTEX(mutex_b);  // Potential deadlock here
    
    printf("Thread 1: Got both locks, doing work\n");
    UNLOCK_MUTEX(mutex_b);
    UNLOCK_MUTEX(mutex_a);
    return NULL;
}

void* mutex_worker2(void* arg) {
    void** mutexes = (void**)arg;
    void* mutex_a = mutexes[0];
    void* mutex_b = mutexes[1];
    
    printf("Thread 2: Locking mutex B\n");
    LOCK_MUTEX(mutex_b);
    usleep(100000);  // 100ms delay
    
    printf("Thread 2: Trying to lock mutex A\n");
    LOCK_MUTEX(mutex_a);  // Potential deadlock here
    
    printf("Thread 2: Got both locks, doing work\n");
    UNLOCK_MUTEX(mutex_a);
    UNLOCK_MUTEX(mutex_b);
    return NULL;
}

// Example 2: RwLock usage
void* reader_worker(void* arg) {
    int reader_id = *(int*)arg;
    
    printf("Reader %d: Acquiring read lock\n", reader_id);
    RWLOCK_READ(shared_rwlock);
    
    printf("Reader %d: Reading shared data: %d\n", reader_id, shared_counter);
    usleep(50000);  // 50ms
    
    RWUNLOCK_READ(shared_rwlock);
    printf("Reader %d: Released read lock\n", reader_id);
    return NULL;
}

void* writer_worker(void* arg) {
    printf("Writer: Acquiring read lock first\n");
    RWLOCK_READ(shared_rwlock);
    
    printf("Writer: Attempting to upgrade to write lock\n");
    usleep(25000);  // 25ms
    RWLOCK_WRITE(shared_rwlock);  // This will deadlock!
    
    printf("Writer: Writing to shared data\n");
    shared_counter++;
    
    RWUNLOCK_WRITE(shared_rwlock);
    return NULL;
}

// Example 3: Condvar usage
void* condvar_waiter(void* arg) {
    printf("Waiter: Waiting for condition\n");
    LOCK_MUTEX(condition_mutex);
    
    while (!condition_ready) {
        CONDVAR_WAIT(condition_var, condition_mutex);
    }
    
    printf("Waiter: Condition met, proceeding\n");
    UNLOCK_MUTEX(condition_mutex);
    return NULL;
}

void* condvar_notifier(void* arg) {
    usleep(200000);  // 200ms delay
    
    printf("Notifier: Setting condition and signaling\n");
    LOCK_MUTEX(condition_mutex);
    condition_ready = 1;
    CONDVAR_NOTIFY_ONE(condition_var);
    UNLOCK_MUTEX(condition_mutex);
    return NULL;
}

// Define tracked thread wrappers
DEFINE_TRACKED_THREAD(mutex_worker1)
DEFINE_TRACKED_THREAD(mutex_worker2)
DEFINE_TRACKED_THREAD(reader_worker)
DEFINE_TRACKED_THREAD(writer_worker)
DEFINE_TRACKED_THREAD(condvar_waiter)
DEFINE_TRACKED_THREAD(condvar_notifier)

int main() {
    printf("Initializing Deloxide with deadlock detection\n");
    deloxide_init("c_deadlock_test.json", deadlock_callback);
    
    // Create synchronization primitives
    void* mutex_a = deloxide_create_mutex();
    void* mutex_b = deloxide_create_mutex();
    counter_mutex = deloxide_create_mutex();
    shared_rwlock = deloxide_create_rwlock();
    condition_mutex = deloxide_create_mutex();
    condition_var = deloxide_create_condvar();
    
    // Example 1: Mutex deadlock test
    printf("\n=== Testing Mutex Deadlock Scenario ===\n");
    void* mutex_args1[2] = {mutex_a, mutex_b};
    void* mutex_args2[2] = {mutex_a, mutex_b};
    
    pthread_t mutex_threads[2];
    CREATE_TRACKED_THREAD(mutex_threads[0], mutex_worker1, mutex_args1);
    CREATE_TRACKED_THREAD(mutex_threads[1], mutex_worker2, mutex_args2);
    
    // Example 2: RwLock upgrade deadlock test
    printf("\n=== Testing RwLock Upgrade Deadlock ===\n");
    pthread_t reader_threads[3];
    int reader_ids[3] = {1, 2, 3};
    
    for (int i = 0; i < 3; i++) {
        CREATE_TRACKED_THREAD(reader_threads[i], reader_worker, &reader_ids[i]);
    }
    
    pthread_t writer_thread;
    CREATE_TRACKED_THREAD(writer_thread, writer_worker, NULL);
    
    // Example 3: Condvar test (should work without deadlock)
    printf("\n=== Testing Condvar Synchronization ===\n");
    pthread_t condvar_threads[2];
    CREATE_TRACKED_THREAD(condvar_threads[0], condvar_waiter, NULL);
    CREATE_TRACKED_THREAD(condvar_threads[1], condvar_notifier, NULL);
    
    // Let all threads run and potentially detect deadlocks
    printf("\nWaiting for threads to complete or deadlock...\n");
    sleep(3);
    
    printf("Program completed\n");
    return 0;
}
```

#### C API Portability Notes

- **Linux/macOS**: Full pthread support, all features available
- **Windows**: Requires pthread-compatible library. Refer to [C API portability notes]

## Stress Testing

Deloxide includes an optional stress testing feature to increase the probability of deadlock manifestation during testing. This feature helps expose potential deadlocks by strategically delaying threads at critical points.

### Enabling Stress Testing

#### In Rust:

Enable the feature in your `Cargo.toml`:

```toml
[dependencies]
deloxide = { version = "0.3", features = ["stress-test"] }
```

Then use the stress testing API:

```rust
// With random preemption strategy
Deloxide::new()
    .with_log("deadlock.log")
    .with_random_stress()
    .callback(|info| {
        eprintln!("Deadlock detected! Cycle: {:?}", info.thread_cycle);
    })
    .start()
    .expect("Failed to initialize detector");

// Or with component-based strategy and custom configuration
use deloxide::StressConfig;

Deloxide::new()
    .with_log("deadlock.log")
    .with_component_stress()
    .with_stress_config(StressConfig {
        preemption_probability: 0.8,
        min_delay_ms: 5,
        max_delay_ms: 20,
        preempt_after_release: true,
    })
    .start()
    .expect("Failed to initialize detector");
```

#### In C:

Build Deloxide with the stress-test feature enabled, then:

```c
// Enable random preemption stress testing (70% probability, 1-10ms delays)
deloxide_enable_random_stress(0.7, 1, 10);

// Or enable component-based stress testing
deloxide_enable_component_stress(5, 15);

// Initialize detector
deloxide_init("deadlock.log", deadlock_callback);
```

### Stress Testing Modes

- **Random Preemption**: Randomly delays threads before lock acquisitions with configurable probability
- **Component-Based**: Analyzes lock acquisition patterns and intelligently targets delays to increase deadlock probability

> [!NOTE]
> Condvar wake-ups (notify_one/notify_all) trigger a synthesized mutex attempt for the woken thread to model the required mutex re-acquisition. Stress injection occurs on this synthetic mutex attempt (and on normal lock attempts), not directly on the condvar wait/notify operations.

## Building and Installation

### Rust

Deloxide is available on crates.io. You can add it as a dependency in your `Cargo.toml`:

```toml
[dependencies]
deloxide = "0.3"
```

With stress testing:

```toml
[dependencies]
deloxide = { version = "0.3", features = ["stress-test"] }
```

Or install the CLI tool to showcase deadlock logs directly:

```bash
cargo install deloxide
deloxide my_deadlock.log  # Opens visualization in browser
```

For development builds:

```bash
# Standard build
cargo build --release

# With stress testing feature
cargo build --release --features stress-test
```

### C

For C programs, you'll need to compile the Rust library and link against it:

```bash
# Build the Rust library
cargo build --release

# With stress testing feature
cargo build --release --features stress-test

# Compile your C program with Deloxide
gcc -Iinclude your_program.c -Ltarget/release -ldeloxide -lpthread -o your_program
```

A Makefile is included in the repository to simplify building and testing with C programs.
It handles building the Rust library and compiling the C test programs automatically.

### C API portability notes

- Thread ID size across FFI
  - The C header uses `uintptr_t` for all thread IDs; the Rust side uses `usize`. This ensures correct sizes on LP64 (Linux/macOS) and LLP64 (Windows).

- pthread-based helpers are POSIX-only
  - The convenience macros `DEFINE_TRACKED_THREAD` and `CREATE_TRACKED_THREAD` depend on `pthread.h` and are available only on non-Windows platforms.
  - On Windows, these macros are disabled at compile time. You can still use the full C API by manually registering thread lifecycle events.

- Manual thread registration (Windows or custom runtimes)
  1. Create your thread using your platform's API.
  2. In the thread entry, call `deloxide_register_thread_spawn(child_tid, parent_tid)` once. On the thread, get IDs from `deloxide_get_thread_id()`.
  3. Before the thread returns, call `deloxide_register_thread_exit(current_tid)`.

  Minimal example sketch (pseudo-C):

  ```c
  // In parent, capture parent thread id
  uintptr_t parent_tid = deloxide_get_thread_id();
  // Create thread with OS API (e.g., _beginthreadex / CreateThread)
  // In child thread entry:
  uintptr_t child_tid = deloxide_get_thread_id();
  deloxide_register_thread_spawn(child_tid, parent_tid);
  // ... user work ...
  deloxide_register_thread_exit(child_tid);
  ```

## Visualization

Deloxide includes a web-based visualization tool. After detecting a deadlock, use the showcase feature to view it in your browser:

```rust
// In Rust
deloxide::showcase("deadlock_log.log").expect("Failed to launch visualization");

// Or for the currently active log
deloxide::showcase_this().expect("Failed to launch visualization");
```

```c
// In C
deloxide_showcase("deadlock_log.log");

// Or for the currently active log
deloxide_showcase_current();
```

You can also automatically launch the visualization when a deadlock is detected by calling the showcase function in your deadlock callback.

Additionally, you can manually upload a log file to visualize deadlocks through the web interface:

[Deloxide Showcase](https://deloxide.vercel.app/)

## Documentation

For more detailed documentation:

- Crates.io: `https://crates.io/crates/deloxide`
- Rust Docs: `https://docs.rs/deloxide`
- C API: See `include/deloxide.h` and `https://docs.rs/deloxide/latest/deloxide/ffi/index.html`

## Performance & Evaluation

This section outlines the performance, deadlock detection capabilities, and robustness of `Deloxide` v0.3. We compare it against standard Rust mutexes (`std::sync::Mutex`), `parking_lot::Mutex` (with its `deadlock_detection` feature), and the `no_deadlocks` library.

All benchmarks were run on a base M1 MacBook Pro with Rust 1.86.0-nightly (v0.3.0).

### 1. Performance Overhead

We evaluated overhead using both low-level microbenchmarks and application-level macrobenchmarks.

#### Microbenchmark Overhead

These tests measure the raw performance of creating locks and performing single, uncontended lock/unlock cycles.

**Mutex Performance:**

| Tested Setup | Lock/Unlock Time |
| :--- | :--- |
| **Std** | **8.5 ± 0.06 ns** |
| **ParkingLot** | 9.7 ± 0.11 ns |
| **NoDeadlocks** | 9.7 ± 0.09 µs |
| **Deloxide (Default)** | 68.8 ± 0.59 ns |
| `Deloxide+LockOrder` | 70.2 ± 0.74 ns |
| `Deloxide+StressRand` | 3.9 ± 1.19 ms |
| `Deloxide+StressComp` | 235.3 ± 3.73 ns |

**RwLock Performance:**

| Tested Setup | Read Lock/Unlock | Write Lock/Unlock |
| :--- | :--- | :--- |
| **Std** | 13.5 ± 0.21 ns | 9.6 ± 0.10 ns |
| **ParkingLot** | 16.0 ± 0.16 ns | 12.8 ± 0.72 ns |
| **NoDeadlocks** | 10.6 ± 0.07 µs | 10.6 ± 0.09 µs |
| **Deloxide (Default)** | 102.3 ± 1.06 ns | 73.3 ± 0.55 ns |
| `Deloxide+LockOrder` | 103.1 ± 1.17 ns | 73.8 ± 0.50 ns |
| `Deloxide+StressRand` | 3.9 ± 1.33 ms | 4.0 ± 1.18 ms |
| `Deloxide+StressComp` | 103.9 ± 0.79 ns | 238.7 ± 3.83 ns |

*(Lower is better)*

**Analysis:**
- `Deloxide` adds ~60-90ns overhead per lock operation compared to std/parking_lot (still sub-microsecond)
- Lock order checking adds negligible overhead (~1-2ns)
- Stress testing modes intentionally add delays for bug detection (not intended for production)
- `NoDeadlocks` has 1000x higher overhead than Deloxide for basic operations

#### Application-Level Overhead

**Producer-Consumer Benchmark** (High contention scenario with multiple producers/consumers accessing a shared queue):

| Configuration | 4x4 Threads | 16x16 Threads | 64x64 Threads |
| :--- | :--- | :--- | :--- |
| **Std** | 306.2 ± 2.57 µs | 942.1 ± 92.47 µs | 4.2 ± 0.02 ms |
| **ParkingLot** | 222.7 ± 9.47 µs | 1264.7 ± 36.94 µs | 8.8 ± 0.46 ms |
| **Deloxide** | 1553.8 ± 26.35 µs | 19.5 ± 0.88 ms | 308.7 ± 73.88 ms |
| `Deloxide+LockOrder` | 18.6 ± 0.64 ms | 359.5 ± 27.24 ms | - |
| `Deloxide+StressComp` | 120.0 ± 1.66 ms | 474.1 ± 56.33 ms | - |
| **NoDeadlocks** | 16.4 ± 12.97 s | - | - |

**RwLock Concurrent Reads Benchmark** (Multiple readers accessing shared data):

| Configuration | 4 Threads | 16 Threads | 64 Threads |
| :--- | :--- | :--- | :--- |
| **Std** | 264.2 ± 2.75 µs | 6.7 ± 0.43 ms | 29.0 ± 2.31 ms |
| **ParkingLot** | 298.3 ± 3.73 µs | 3.0 ± 0.04 ms | 14.1 ± 0.26 ms |
| **Deloxide** | 575.3 ± 6.88 µs | 3.1 ± 0.04 ms | 31.7 ± 1.35 ms |
| `Deloxide+LockOrder` | 578.4 ± 7.16 µs | 3.0 ± 0.05 ms | 30.9 ± 4.42 ms |
| `Deloxide+StressComp` | 613.0 ± 35.61 µs | 3.2 ± 0.11 ms | 38.7 ± 6.06 ms |
| **NoDeadlocks** | 21.6 ± 0.05 ms | - | - |

**Analysis:**
- Under high contention (producer-consumer), Deloxide is 5-20x slower than std, but still completes in milliseconds
- For read-heavy workloads (concurrent reads), overhead is much lower (2-3x)
- Lock order checking adds minimal overhead in real applications
- NoDeadlocks is 10-1000x slower than Deloxide, making it impractical for many scenarios

### 2. Deadlock Detection Capability

The primary goal of `Deloxide` is to find deadlocks quickly and reliably. We focus on detecting **Heisenbugs**—elusive deadlocks that only manifest under specific, rare thread interleavings and often disappear when you try to debug them. These bugs are notoriously difficult to reproduce and find in testing.

We tested 140 different configurations across 14 deadlock scenarios. For fairness and reproducibility, all tests used **fixed random seeds**, ensuring each detector faced identical thread scheduling conditions. This allows for direct comparison of detection capabilities.

#### Detection Rate Summary

**Heisenbug Deadlock Scenarios** (1000 runs each with fixed seed):

| Scenario | Deloxide | +LockOrder | +Random | +Random+LO | +Aggressive | +Agg+LO | +Component | +Comp+LO | ParkingLot | NoDeadlocks |
|:---------|:--------:|:----------:|:-------:|:----------:|:-----------:|:-------:|:----------:|:--------:|:----------:|:-----------:|
| **Two Lock** | 25.6% | **100.0%** | 66.3% | 99.8% | 74.7% | **100.0%** | 28.4% | **100.0%** | 31.5% | 63.6% |
| **Two Lock (2t)** | 39.2% | **100.0%** | 84.6% | **100.0%** | 91.4% | **100.0%** | 48.2% | **100.0%** | 56.4% | 99.7% |
| **Two Lock (4t)** | 76.0% | **100.0%** | 97.9% | **100.0%** | 99.7% | **100.0%** | 79.4% | **100.0%** | 76.1% | 74.5% |
| **Two Lock (8t)** | 93.6% | **100.0%** | 99.9% | **100.0%** | **100.0%** | **100.0%** | 99.1% | **100.0%** | 91.6% | 94.8% |
| **Two Lock (16t)** | 99.5% | **100.0%** | **100.0%** | **100.0%** | **100.0%** | **100.0%** | **100.0%** | **100.0%** | 99.0% | 99.9% |
| **Three Lock Cycle** | 89.1% | **100.0%** | 99.6% | **100.0%** | 99.8% | **100.0%** | **100.0%** | **100.0%** | 79.1% | 98.6% |
| **Five Lock Cycle** | **100.0%** | **100.0%** | **100.0%** | **100.0%** | **100.0%** | **100.0%** | **100.0%** | **100.0%** | **100.0%** | **100.0%** |
| **RwLock Deadlock** | 32.4% | **100.0%** | 80.5% | **100.0%** | 91.0% | **100.0%** | 33.6% | 99.9% | 59.0% | 99.9% |
| **Dining Philosophers** | 66.5% | 66.9% | 93.6% | 92.6% | 95.6% | 95.4% | 75.7% | 79.5% | 52.9% | 75.3% |

**Guaranteed Deadlock Scenarios** (250 runs each):

All configurations achieved **100% detection rate** across all guaranteed deadlock scenarios, including:
- Guaranteed Two Lock
- Guaranteed Three Lock  
- Guaranteed RwLock Deadlock
- Guaranteed Dining Philosophers
- Guaranteed Condvar Deadlock

#### Detection Speed

Average time to detect deadlock (milliseconds):

| Scenario | Deloxide | +LockOrder | +Random | +Agg | +Component | ParkingLot | NoDeadlocks |
|:---------|:--------:|:----------:|:-------:|:----:|:----------:|:----------:|:-----------:|
| **Two Lock** | 1.69 | **1.54** | 18.26 | 9.81 | 15.96 | 1.57 | 1100.37 |
| **Two Lock (2t)** | 1.55 | **1.53** | 17.01 | 9.23 | 16.08 | 1.54 | 1092.56 |
| **Two Lock (4t)** | 0.77 | **0.04** | 3.74 | 1.29 | 9.38 | 0.42 | 1400.35 |
| **Two Lock (8t)** | 0.42 | **0.11** | 0.86 | 0.19 | 7.25 | 0.37 | 1943.13 |
| **Two Lock (16t)** | 0.37 | **0.25** | 0.35 | 0.30 | 0.79 | 0.36 | 2619.03 |
| **Three Lock** | 1.55 | **1.53** | 23.64 | 12.91 | 19.90 | 1.59 | 1325.65 |
| **Five Lock** | 1.59 | **1.57** | 37.85 | 20.46 | 22.06 | 1.64 | 3093.64 |
| **RwLock Deadlock** | 1.61 | **1.55** | 16.80 | 9.27 | 15.78 | 1.56 | 1088.95 |
| **Dining Philosophers** | 1.61 | **1.57** | 37.77 | 19.84 | 20.68 | 1.65 | 3252.39 |

**Key Findings:**
- **Lock Order Graph detection is fastest**: Detects deadlocks in ~0.04-1.6ms on average (100-10000x faster than NoDeadlocks)
- **ParkingLot is fast but misses bugs**: Similar speed to Deloxide (~1.5ms) but much lower detection rates (31-79%)
- **Stress testing trades speed for detection rate**: Random/Aggressive modes take 10-40ms but catch 95-100% of bugs
- **NoDeadlocks is 50-2000x slower**: Takes 1-3 seconds to detect what Deloxide finds in microseconds/milliseconds
- **Scaling improves detection speed**: With more threads (16t), even basic Deloxide detects in 0.37ms
- **Guaranteed deadlocks detected instantly**: All detectors find these in <2ms

### 3. False Positive Analysis

A deadlock detector must be reliable. We tested 90 configurations of deadlock-free code (10 runs each) across 9 different scenarios to ensure no false alarms.

**False Positive Test Results:**

| Test Category | Configurations Tested | False Positives (Wait-For) | Known FP (Lock Order) |
|:--------------|:---------------------:|:--------------------------:|:---------------------:|
| **Traditional FP Tests** | 70 | **0** | 0 |
| **Lock Order FP Tests** | 20 | **0** | 8 |
| **Total** | 90 | **0** | 8 |

**Test Scenarios:**
1. **Gate Guarded**: Threads lock A→B or B→A, but use a gate to prevent circular waits
2. **Four Hierarchical**: Locks always acquired in consistent order (A→B→C→D)
3. **Conditional Locking**: Lock acquisition depends on runtime conditions
4. **Lock-Free Intervals**: Threads release all locks between critical sections
5. **Producer-Consumer**: Proper condvar-based synchronization
6. **Read-Dominated**: Heavy read-lock usage with occasional writes
7. **Thread-Local Hierarchy**: Each thread has its own lock hierarchy
8. **Complex Lock Order**: Multiple valid lock orders that don't create cycles
9. **Lock Order Inversion**: Apparent inversions that are actually safe

**False Positive Test Execution Times** (average across 10 runs):

| Test Scenario | Deloxide | +LockOrder | +StressComp | ParkingLot | NoDeadlocks |
|:--------------|:--------:|:----------:|:-----------:|:----------:|:-----------:|
| **Gate Guarded** | 0.52s | 0.52s | 0.51s | 0.47s | 0.58s |
| **Four Hierarchical** | 0.66s | 0.66s | 0.69s | 0.63s | 11.73s |
| **Conditional Locking** | 25.55s | 25.56s | 26.83s | 24.12s | 654.32s |
| **Lock-Free Interval** | 1.11s | 1.11s | 1.12s | 0.96s | 1.11s |
| **Producer-Consumer** | 0.70s | 0.71s | 0.72s | 0.59s | 14.09s |
| **Read-Dominated** | 2.07s | 2.07s | 5.21s | 1.79s | 19.30s |
| **Thread-Local Hierarchy** | 25.69s | 25.73s | 28.41s | 22.68s | 318.44s |
| **Complex Lock Order** | 0.06s | 0.07s | 0.11s | 0.06s | 0.06s |
| **Lock Order Inversion** | 0.06s | 0.06s | 0.07s | 0.05s | 0.06s |

**Analysis:**
- **Zero unexpected false positives**: Wait-for graph detection is 100% accurate across all scenarios
- **Lock order graph limitations**: 8 known false positives in scenarios with complex but safe lock ordering patterns
  - These are inherent limitations of static lock order analysis
  - Wait-for graph detection correctly identifies these as safe
- **All detectors passed**: Deloxide, ParkingLot, and NoDeadlocks all showed zero false positives on traditional tests
- **Performance on deadlock-free code**: Deloxide performs similarly to ParkingLot, while NoDeadlocks is 1-27x slower on complex scenarios

### 4. Summary & Comparison

#### Detector Comparison

| Feature | std::sync | parking_lot | Deloxide | Deloxide +stress | no_deadlocks |
|:--------|:---------:|:-----------:|:--------:|:----------------:|:------------:|
| **Performance** |
| Microbenchmark Overhead | 1x | 1.1x | 7-10x | 1000x+ | 1000x+ |
| Real Application Overhead | 1x | 0.7-1.2x | 1-2x | 100-500x | 10-1000x |
| Production Ready | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Detection** |
| Heisenbug Detection Rate | ❌ | 31-79% | 26-100% | 95-100% | 64-100% |
| Detection Speed | N/A | ~1.5ms | 0.04-1.6ms | 10-40ms | 1-3 seconds |
| False Positives | N/A | 0% | 0% | 0% | 0% |
| **Features** |
| Lock Order Graph | ❌ | ❌ | ✅ | ✅ | ❌ |
| Stress Testing | ❌ | ❌ | ✅ | ✅ | ❌ |
| Visualization | ❌ | ❌ | ✅ | ✅ | ❌ |
| Condvar Detection | ❌ | ❌ | ✅ | ✅ | ✅ |
| Cross-Language (C API) | ❌ | ❌ | ✅ | ✅ | ❌ |

**Note:** All parking_lot results use the deadlock_detection feature enabled.

#### Quick Decision Guide

**Deloxide's Sweet Spot:**
- **Performance:** 1-2x overhead in real applications (comparable to parking_lot), despite 7-10x in microbenchmarks
- **Detection:** 95-100% Heisenbug detection with stress testing, 50-2000x faster than no_deadlocks
- **Features:** Only detector with visualization, lock order graph, stress testing, and full Condvar support

**When to Choose Each:**

| Detector | Best For | Key Advantage | Main Limitation |
|:---------|:---------|:--------------|:----------------|
| **std::sync** | Maximum performance, no detection needed | Fastest (baseline) | No deadlock detection |
| **parking_lot** | Basic detection with minimal overhead | Fast + 31-79% detection | Misses many Heisenbugs, no Condvar detection |
| **Deloxide** | Development, testing, production monitoring | 95-100% detection + visualization + 1-2x overhead | 7-10x microbenchmark overhead |
| **Deloxide +stress** | CI/CD, hunting elusive bugs | 95-100% detection guaranteed | High overhead (testing only) |
| **no_deadlocks** | When speed doesn't matter | High detection rates | 1-3 second detection time |

#### Recommendation by Use Case

| Your Scenario | Choose This |
|:--------------|:------------|
| **Production (performance-critical)** | std::sync or parking_lot without detection |
| **Production (moderate performance)** | Deloxide or parking_lot (1-2x overhead acceptable) |
| **Development & debugging** | Deloxide (visualization + better detection) |
| **CI/CD & testing** | Deloxide +stress (95-100% detection) |
| **Hunting Heisenbugs** | Deloxide +aggressive or +component stress |
| **Need visualization or C API** | Deloxide (unique features) |

**Bottom Line:** Deloxide offers the best balance—catching 95-100% of Heisenbugs with only 1-2x real-world overhead, 50-2000x faster detection than alternatives, plus unique features like visualization and cross-language support. Use it for development, testing, and production monitoring. Only skip it if you need absolute maximum performance or are certain your code is deadlock-free.
- ✅ Production monitoring (without stress testing)
- ✅ Debugging hard-to-reproduce deadlocks
- ⚠️ Not recommended for ultra-low-latency systems with heavy lock contention

This project is my graduation project so I will share the full test suite repo after my defense.

## License

```
/*
 *      ( (
 *       ) )
 *    ........
 *    |      |]  ☕
 *    \      /
 *     `----'
 *
 * "THE COFFEEWARE LICENSE" (Revision 1, Deloxide Edition):
 * (Inspired by the original Beerware License by Poul-Henning Kamp)
 *
 * Emirhan Tala and Ulaş Can Demirbağ wrote this file. As long as you retain
 * this notice, you can do whatever you want with this stuff — run it, fork it,
 * deploy it, tattoo it, or summon it in a thread ritual. We don't care.
 *
 * Just remember: we make no guarantees, provide no warranties, and accept no
 * responsibility for anything that happens. This software may or may not work,
 * may or may not cause your system to spontaneously combust into deadlocks,
 * and may or may not summon a sentient debugger from the void. But we accept
 * coffee! If we ever meet someday and you think this code helped you can buy 
 * us a coffee in return. Or not. No pressure. But coffee is nice. We love it!
 * ----------------------------------------------------------------------------
 */
```

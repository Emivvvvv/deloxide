# <img src='./deloxide_logo_orange.png' height='25'> Deloxide - Cross-Language Deadlock Detector

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![License: Coffeeware](https://img.shields.io/badge/License-Coffeeware-brown.svg)](LICENSE)

Deloxide is a cross-language deadlock detection library with visualization support. It tracks mutex operations in multi-threaded applications to detect, report, and visualize potential deadlocks in real-time.

## Features

- **Real-time deadlock detection** - Detects deadlocks as they happen
- **Cross-language support** - Core implementation in Rust with C bindings
- **Thread & lock tracking** - Monitors relationships between threads and locks
- **Visualization** - Web-based visualization of thread-lock relationships
- **Low overhead** - Designed to be lightweight for use in production systems
- **Easy integration** - Simple API for both Rust and C/C++
- **Stress testing** - Optional feature to increase deadlock manifestation during testing

## Project Architecture

### How Deloxide Works

1. **Initialization**: The application initializes Deloxide with optional logging and callback settings.

2. **Resource Creation**: When threads and mutexes are created, they're registered with the deadlock detector.

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
   - Rust API with `TrackedMutex` and `TrackedThread` types
   - C API through FFI bindings in `deloxide.h`
   - Simple macros for C to handle common operations

5. **Stress Testing** (Optional with stress-testing feature)
   - Strategically delays threads to increase deadlock probability
   - Multiple strategies for different testing scenarios
   - Available as an opt-in feature for testing environments

## Quick Start

### Rust

```rust
use deloxide::{Deloxide, TrackedMutex, TrackedThread};
use std::sync::Arc;
use std::time::Duration;
use std::thread;

fn main() {
    // Initialize the detector with a deadlock callback
    Deloxide::new()
        .with_log("deadlock.log")
        .callback(|info| {
            eprintln!("Deadlock detected! Cycle: {:?}", info.thread_cycle);
            // Automatically show visualization in browser
            deloxide::showcase_this().expect("Failed to launch visualization");
        })
        .start()
        .expect("Failed to initialize detector");

    // Create two mutexes
    let mutex_a = Arc::new(TrackedMutex::new("Resource A"));
    let mutex_b = Arc::new(TrackedMutex::new("Resource B"));

    // Create deadlock between two threads
    let mutex_a_clone = Arc::clone(&mutex_a);
    let mutex_b_clone = Arc::clone(&mutex_b);

    let _t1 = TrackedThread::spawn(move || {
        let _a = mutex_a.lock().unwrap();
        thread::sleep(Duration::from_millis(100));
        let _b = mutex_b.lock().unwrap();
    });

    let _t2 = TrackedThread::spawn(move || {
        let _b = mutex_b_clone.lock().unwrap();
        thread::sleep(Duration::from_millis(100));
        let _a = mutex_a_clone.lock().unwrap();
    });

    thread::sleep(Duration::from_secs(1));
}
```

### C

find `deloxide.h` in `include/deloxide.h`

```c
#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include "deloxide.h"

void deadlock_callback(const char* json_info) {
    printf("Deadlock detected! Details:\n%s\n", json_info);
    // Automatically show visualization in browser
    deloxide_showcase_current();
}

void* worker1(void* arg) {
    void* mutex_a = ((void**)arg)[0];
    void* mutex_b = ((void**)arg)[1];
    
    LOCK(mutex_a);
    printf("Thread 1 acquired lock A\n");
    usleep(100000);  // 100 ms
    
    LOCK(mutex_b);
    printf("Thread 1 acquired lock B\n");
    
    return NULL;
}

void* worker2(void* arg) {
    void* mutex_a = ((void**)arg)[0];
    void* mutex_b = ((void**)arg)[1];
    
    LOCK(mutex_b);
    printf("Thread 2 acquired lock B\n");
    usleep(100000);  // 100 ms
    
    LOCK(mutex_a);
    printf("Thread 2 acquired lock A\n");
    
    return NULL;
}

DEFINE_TRACKED_THREAD(worker1)
DEFINE_TRACKED_THREAD(worker2)

int main() {
    // Initialize with deadlock callback
    deloxide_init("deadlock.log", deadlock_callback);
    
    // Create mutexes
    void* mutex_a = deloxide_create_mutex();
    void* mutex_b = deloxide_create_mutex();
    
    // Set up thread arguments
    void* thread1_args[2] = {mutex_a, mutex_b};
    void* thread2_args[2] = {mutex_a, mutex_b};
    
    // Create threads
    pthread_t t1, t2;
    CREATE_TRACKED_THREAD(t1, worker1, thread1_args);
    CREATE_TRACKED_THREAD(t2, worker2, thread2_args);
    
    sleep(1);
    
    return 0;
}
```

## Stress Testing

Deloxide includes an optional stress testing feature to increase the probability of deadlock manifestation during testing. This feature helps expose potential deadlocks by strategically delaying threads at critical points.

### Enabling Stress Testing

#### In Rust:

Enable the feature in your `Cargo.toml`:

```toml
[dependencies]
deloxide = { version = "0.1.0", features = ["stress-test"] }
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

## Building and Installation

### Rust

Deloxide is available on crates.io. You can add it as a dependency in your `Cargo.toml`:

```toml
[dependencies]
deloxide = "0.1.0"
```

With stress testing:

```toml
[dependencies]
deloxide = { version = "0.1.0", features = ["stress-test"] }
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

## Benchmarks

To evaluate the performance overhead of Deloxide, a series of benchmarks were conducted. These tests compare the performance of standard `parking_lot::Mutex` (Baseline) against Deloxide's `TrackedMutex` with deadlock detection enabled (Detector) and `TrackedMutex` with both detection and asynchronous file logging enabled (Detector Log).

**Test Environment:** All benchmarks were run on a base MacBook M1 Pro with an 8-core CPU (6 performance cores, 2 efficiency cores).

**Key Performance Metrics:**
* **Baseline Time:** Execution time using standard `parking_lot::Mutex`.
* **Detector Time/Ratio:** Execution time and overhead factor using Deloxide's `TrackedMutex` (detection only).
* **Detector Log Time/Ratio:** Execution time and overhead factor using Deloxide's `TrackedMutex` (detection and asynchronous file logging).

---

### 1. Single Mutex Operations

* Scenario: Basic mutex generation, lock, and unlock operations on a single `TrackedMutex` in a tight loop.*
* Benching: The fundamental, raw overhead of a single `TrackedMutex` generation & lock/release cycle.*

| Metric         | Time (ns)     | Overhead Factor |
|----------------|---------------|-----------------|
| Baseline       | 8.2 ± 0.06    | 1.0x            |
| Detector       | 59.5 ± 0.65   | ~7.27x          |
| Detector (Log) | 59.8 ± 0.36   | ~7.31x          |

**Observation:** Deloxide introduces approximately 51 ns of overhead per lock/unlock cycle in this best-case, uncontended scenario. The cost of preparing log data for asynchronous writing is negligible (~0.3 ns).

---

### 2. Hierarchical Locking

* Scenario: Multiple threads acquire multiple locks in a strict, deadlock-free order.*
* Benching: Performance of sequential multi-lock acquisitions/releases as locks and threads scale.*

| Test Case             | Baseline Time (µs) | Detector Ratio | Detector (Log) Ratio |
|-----------------------|--------------------|----------------|----------------------|
| 2 Locks, 2 Threads    | 30.3 ± 0.91        | ~1.40x         | ~1.41x               |
| 4 Locks, 4 Threads    | 57.2 ± 2.01        | ~1.97x         | ~1.99x               |
| 8 Locks, 8 Threads    | 89.2 ± 3.61        | ~6.48x         | ~6.37x               |

**Observation:** The overhead factor increases with both the number of locks and threads, scaling from ~1.4x to ~6.4x in the tested configurations. Asynchronous logging consistently adds minimal overhead over detection alone.

---

### 3. Producer-Consumer

* Scenario: Multiple "producer" threads add items to a shared buffer (one mutex) and multiple "consumer" threads remove items.*
* Benching: `TrackedMutex` performance under high contention for a single resource.*

| Test Case             | Baseline Time (µs) | Detector Ratio | Detector (Log) Ratio |
|-----------------------|--------------------|----------------|----------------------|
| 1 Prod, 1 Cons        | 30.1 ± 1.19        | ~1.44x         | ~1.44x               |
| 2 Prod, 2 Cons        | 55.9 ± 2.51        | ~1.27x         | ~1.27x               |
| 4 Prod, 4 Cons        | 104.0 ± 4.40       | ~1.16x         | ~1.15x               |

**Observation:** Deloxide's overhead factor remains moderate and relatively stable (mostly between ~1.15x and ~1.44x) even with high contention on a single mutex.

---

### 4. Reader-Writer Pattern

* Scenario: Multiple threads access shared data (one mutex), with a configurable ratio of "read" vs. "write" operations.*
* Benching: `TrackedMutex` performance with mixed read/write access patterns.*

| Test Case                 | Baseline Time (µs) | Detector Ratio | Detector (Log) Ratio |
|---------------------------|--------------------|----------------|----------------------|
| 4 Threads, 10% Write      | 55.0 ± 3.34        | ~1.31x         | ~1.29x               |
| 8 Threads, 10% Write      | 90.0 ± 9.55        | ~1.37x         | ~1.32x               |
| 16 Threads, 10% Write     | 221.9 ± 12.39      | ~1.04x         | ~1.00x               |
| 32 Threads, 10% Write     | 425.5 ± 24.79      | ~1.39x         | ~1.22x               |

**Observation:**
*   With 4-8 threads, overhead is around ~1.3x.
*   With 16 threads, overhead is very low (~1.0x - 1.06x), indicating efficient operation. In some cases, Detector (Log) even slightly outperforms Detector, likely due to benchmark variance.
*   With 32 threads, overhead increases to ~1.2x - ~1.6x, with higher variability, suggesting increased contention effects.

## Examples

Example programs are provided in both Rust and C to demonstrate various deadlock scenarios and detection capabilities:

- **Two Thread Deadlock**: Simple deadlock between two threads
- **Dining Philosophers**: Classic deadlock scenario
- **Random Ring**: Deadlock in a ring of threads

See examples in /tests or /c_tests

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
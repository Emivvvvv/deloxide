# <img src='images/deloxide_logo_orange.png' height='25'> Deloxide - Cross-Language Deadlock Detector

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

## Performance & Evaluation

This part outlines the performance, deadlock detection capabilities, and robustness of `Deloxide`. We compare it against standard Rust mutexes (`std::sync::Mutex`), `parking_lot::Mutex` (with its `deadlock_detection` feature), and the `no_deadlocks` library.

**Key Takeaways (TL;DR):**
*   **Performance:** `Deloxide` introduces a manageable performance overhead in many common scenarios but can be more significant under heavy lock contention.
*   **Deadlock Detection:** `Deloxide`'s optional **stress testing** modes are exceptionally effective at uncovering hard-to-find "Heisenbug" deadlocks that are often missed by other detectors.
*   **Superior Speed:** `Deloxide` detects deadlocks up to **80x faster** than competing libraries, providing an immediate feedback loop for developers.
*   **Reliability:** `Deloxide` is robust and does **not** produce false alarms in deadlock-free code.

All benchmarks were run on a base M1 MacBook Pro with Rust 1.86.0-nightly.

### 1. Performance Overhead

We evaluated overhead using both low-level microbenchmarks and application-level macrobenchmarks.

#### Microbenchmark Overhead

These tests measure the raw performance of creating a mutex and performing a single, uncontended lock/unlock cycle.

| Tested Setup | Mutex Generation Time (ns) | Lock/Unlock Time |
| :--- | :--- | :--- |
| **Std** | 17.4 ± 0.16 ns | **8.5 ± 0.07 ns** |
| **ParkingLot** | **16.4 ± 0.27 ns** | 9.7 ± 0.07 ns |
| **NoDeadlocks** | 31.6 ± 0.20 ns | 10.6 ± 0.11 µs |
| **Deloxide (Default)** | 36.2 ± 0.28 ns | 82.1 ± 0.38 ns |
| `Deloxide+StressRand` | 36.4 ± 0.23 ns | 3.2 ± 1.06 ms |
| `Deloxide+StressComp` | 36.3 ± 0.27 ns | 241.6 ± 4.08 ns |

*(Lower is better)*

`Deloxide`'s mutex creation and lock/unlock operations carry a higher base cost than `std` or `parking_lot` due to the integrated, real-time detection logic that runs on every operation.

#### Application-Level Overhead

We simulated two common application workloads to measure performance at scale.

**A) Hierarchical Locking Benchmark**

This benchmark involves multiple threads acquiring a sequence of locks, simulating scenarios with complex, multi-lock dependencies.

![Producer Consumer Results Barchart](./images/hierarchical_locking_benchmark.png)

**Analysis:**
*   In this scenario, `Deloxide`'s baseline overhead is modest. At the 32x32 scale, it is **~1.62x slower** than `std::sync::Mutex` (526.0µs vs 324.2µs).
*   The stress testing modes (`Deloxide+StressRand`, `Deloxide+StressComp`) perform as expected, trading performance for improved bug detection, hence their significantly higher runtimes.
*   The `NoDeadlocks` library showed very high execution times and was not run at larger scales.

**B) Producer-Consumer Benchmark**

This benchmark models a high-contention scenario where multiple producer and consumer threads access a single shared queue protected by a mutex.

![Producer Consumer Results Barchart](./images/producer_consumer_results.png)

**Analysis:**
*   Under heavy contention for a single lock, `Deloxide`'s overhead is more pronounced. At the 4x4 scale, it is **~5.4x slower** than `std` (1.7ms vs 309.4µs).
*   The performance of `Deloxide+StressRand` (28.0s) and `NoDeadlocks` (7.1s) at the 4x4 scale made testing at larger scales impractical.
*   This benchmark highlights that `Deloxide`'s overhead is most noticeable in applications with a central, highly-contended bottleneck.

### 2. Deadlock Detection Capability

The primary goal of `Deloxide` is to find deadlocks. We tested its ability to detect "Heisenbugs"—elusive deadlocks that only occur under specific, rare thread interleavings. A superior detector not only finds these bugs but does so **quickly**, providing rapid feedback to the developer.

The table below shows the percentage of runs (out of 1000) where a deadlock was successfully detected, alongside the average time it took to find it.

| Tested Setup                  | Two-Lock Scenario  | Two-Lock Scenario  | Three-Lock-Cycle Scenario | Three-Lock-Cycle Scenario |
|:------------------------------|:------------------:|:------------------:|:-------------------------:|:-------------------------:|
|                               | **Detection Rate** | **Mean Time (ms)** |    **Detection Rate**     |    **Mean Time (ms)**     |
| **Deloxide (Default)**        |        5.9%        |        2.7         |           0.2%            |           45.9            |
| **`Deloxide+StressRand`**     |       51.2%        |        48.8        |           66.9%           |           158.5           |
| **`Deloxide+StressAggrRand`** |       57.0%        |        56.4        |           75.3%           |           124.4           |
| **`Deloxide+StressComp`**     |        4.6%        |        15.0        |        **100.0%**         |         **16.8**          |
| **ParkingLot**                |        3.7%        |        4.9         |           2.9%            |            5.8            |
| **NoDeadlocks**               |       100.0%       |     **1127.0**     |           98.9%           |        **1370.1**         |

*(Lower time is better)*

**Analysis:**
- Without stress testing, `Deloxide`'s detection rate for these rare deadlocks is low, similar to `parking_lot`. This is expected, as the deadlock condition rarely manifests naturally.
-  **Stress testing is the killer feature.** Enabling random preemption (`StressRand`) dramatically increases the detection rate to over 50-75%, while the component-based strategy (`StressComp`) achieved a **perfect 100% detection rate** for the complex three-lock cycle.
- **Superior Detection Speed:** The most critical finding is the **time to detection**.
   - `Deloxide+StressComp` found the three-lock deadlock in just **16.8 ms**.
   - In contrast, `NoDeadlocks` took **1,370 ms (1.4 seconds)** to detect the same bug.

### 3. False Positive Analysis

A deadlock detector must be reliable. We verified that `Deloxide` does not report deadlocks in correctly written, deadlock-free code.

We ran two deadlock-free scenarios 100 times each:
1.  **Gate Guarded:** Threads lock A then B, or B then A, but use a gate to prevent circular waits.
2.  **Four Hierarchical:** Locks are always acquired in a globally consistent order (A → B → C → D).

**Result:**
Across all tests, `Deloxide` (in all configurations), `parking_lot`, and `no_deadlocks` all passed with **zero false positives**.

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

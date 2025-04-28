# Deloxide - Cross-Language Deadlock Detector

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

- Rust API: `https://docs.rs/deloxide`
- C API: See `include/deloxide.h`

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
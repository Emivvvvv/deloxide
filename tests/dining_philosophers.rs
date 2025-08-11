use deloxide::{Mutex, Thread};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
mod common;
use common::{DEADLOCK_TIMEOUT, expect_deadlock, start_detector};

#[test]
fn test_dining_philosophers_deadlock() {
    let harness = start_detector();

    // Number of philosophers
    let num_philosophers = 5;

    // Create the forks (shared resources)
    let forks: Vec<Arc<Mutex<String>>> = (0..num_philosophers)
        .map(|i| Arc::new(Mutex::new(format!("Fork {}", i))))
        .collect();

    // Create philosophers (threads)
    let mut handles = vec![];

    for i in 0..num_philosophers {
        // Each philosopher needs two forks
        let left_fork = Arc::clone(&forks[i]);
        let right_fork = Arc::clone(&forks[(i + 1) % num_philosophers]);

        // Philosopher thread
        let handle = Thread::spawn(move || {
            // Try to take the left fork first
            println!("Philosopher {} is trying to take left fork", i);
            let _left = left_fork.lock();
            println!("Philosopher {} acquired left fork", i);

            // Small delay to increase chance of deadlock
            thread::sleep(Duration::from_millis(100));

            // Then try to take the right fork
            println!("Philosopher {} is trying to take right fork", i);
            let _right = right_fork.lock();
            println!("Philosopher {} acquired right fork and is eating", i);

            // Eat for a while
            thread::sleep(Duration::from_millis(500));

            println!("Philosopher {} is done eating", i);
        });

        handles.push(handle);
    }

    // Wait for a reasonable time to allow deadlock to be detected
    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);
    assert!(
        info.thread_cycle.len() >= 2,
        "Deadlock should involve at least 2 threads"
    );
    assert!(
        !info.thread_waiting_for_locks.is_empty(),
        "There should be thread-lock waiting relationships"
    );

    // Don't wait for threads to complete since they're deadlocked
    println!("Test complete - threads are intentionally left running");
}

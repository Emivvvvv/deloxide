use deloxide::{Mutex, thread};
use std::sync::Arc;
use std::time::Duration;
mod common;
use common::{DEADLOCK_TIMEOUT, expect_deadlock, start_detector};

#[test]
fn test_simple_two_thread_deadlock() {
    let harness = start_detector();

    // Create two mutexes
    let mutex_a = Arc::new(Mutex::new("Resource A"));
    let mutex_b = Arc::new(Mutex::new("Resource B"));

    // Clone references for the second thread
    let mutex_a_clone = Arc::clone(&mutex_a);
    let mutex_b_clone = Arc::clone(&mutex_b);

    // Thread 1: Lock A, then try to lock B
    let _thread1 = thread::spawn(move || {
        let _guard_a = mutex_a.lock();

        // Give thread 2 time to acquire lock B
        thread::sleep(Duration::from_millis(100));

        // This will cause a deadlock
        let _guard_b = mutex_b.lock();

        // We shouldn't reach here if deadlock is detected
        false
    });

    // Thread 2: Lock B, then try to lock A
    let _thread2 = thread::spawn(move || {
        let _guard_b = mutex_b_clone.lock();

        // Give thread 1 time to acquire lock A
        thread::sleep(Duration::from_millis(100));

        // This will cause a deadlock
        let _guard_a = mutex_a_clone.lock();

        // We shouldn't reach here if deadlock is detected
        false
    });

    // Wait for a reasonable time to allow deadlock to be detected
    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);
    assert_eq!(info.thread_cycle.len(), 2);
    assert_eq!(info.thread_waiting_for_locks.len(), 2);
}

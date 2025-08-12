use deloxide::{Mutex, RwLock, Thread};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
mod common;
use common::{DEADLOCK_TIMEOUT, expect_deadlock, start_detector};

#[test]
fn test_mutex_rwlock_deadlock() {
    let harness = start_detector();

    // The Mutex and RwLock under test
    let mutex = Arc::new(Mutex::new(()));
    let rwlock = Arc::new(RwLock::new(()));

    // Clone for threads
    let mutex1 = Arc::clone(&mutex);
    let rwlock1 = Arc::clone(&rwlock);
    let mutex2 = Arc::clone(&mutex);
    let rwlock2 = Arc::clone(&rwlock);

    // Thread 1: Lock Mutex, then try to lock RwLock (write)
    let _t1 = Thread::spawn(move || {
        let _g1 = mutex1.lock();
        thread::sleep(Duration::from_millis(100));
        let _g2 = rwlock1.write();
        false
    });

    // Thread 2: Lock RwLock (write), then try to lock Mutex
    let _t2 = Thread::spawn(move || {
        let _g1 = rwlock2.write();
        thread::sleep(Duration::from_millis(100));
        let _g2 = mutex2.lock();
        false
    });

    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);
    assert_eq!(info.thread_cycle.len(), 2);
    assert_eq!(info.thread_waiting_for_locks.len(), 2);
}

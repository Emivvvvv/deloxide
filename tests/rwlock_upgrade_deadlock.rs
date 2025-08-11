use deloxide::{RwLock, Thread};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
mod common;
use common::{DEADLOCK_TIMEOUT, expect_deadlock, start_detector};

#[test]
fn test_rwlock_upgrade_deadlock() {
    let harness = start_detector();

    let lock = Arc::new(RwLock::new(0));
    let ready_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for _ in 0..2 {
        let lock = Arc::clone(&lock);
        let ready = Arc::clone(&ready_count);
        handles.push(Thread::spawn(move || {
            let _r = lock.read();
            // Signal ready and wait for all threads
            ready.fetch_add(1, Ordering::SeqCst);
            while ready.load(Ordering::SeqCst) < 2 {
                std::thread::yield_now();
            }
            // Both threads attempt to upgrade at the same time: classic cycle!
            let _w = lock.write();
            // Never proceeds past here
        }));
    }

    // Wait for deadlock or timeout
    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);
    assert_eq!(
        info.thread_cycle.len(),
        2,
        "Deadlock should involve 2 threads"
    );
    assert_eq!(
        info.thread_waiting_for_locks.len(),
        2,
        "Should be 2 waiting relationships"
    );
    println!(
        "✔ Detected RwLock upgrade deadlock: {:?}",
        info.thread_cycle
    );
}

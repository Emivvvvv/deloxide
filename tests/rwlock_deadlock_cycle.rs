use deloxide::{RwLock, thread};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
mod common;
use common::{DEADLOCK_TIMEOUT, expect_deadlock, start_detector};

#[test]
fn test_guaranteed_three_thread_rwlock_deadlock() {
    let harness = start_detector();

    let lock1 = Arc::new(RwLock::new(0));
    let lock2 = Arc::new(RwLock::new(0));
    let lock3 = Arc::new(RwLock::new(0));
    let ready_count = Arc::new(AtomicUsize::new(0));

    let locks = [lock1, lock2, lock3];

    let mut handles = Vec::new();

    for i in 0..3 {
        let locks = locks.clone();
        let ready = Arc::clone(&ready_count);
        handles.push(thread::spawn(move || {
            // Each thread grabs read on i
            let _ri = locks[i].read();
            // Signal ready and wait for all threads
            ready.fetch_add(1, Ordering::SeqCst);
            while ready.load(Ordering::SeqCst) < 3 {
                std::thread::yield_now();
            }
            // Each tries to upgrade to write on (i+1)%3 (held for read by next thread)
            let _wi_next = locks[(i + 1) % 3].write();
            // Never proceeds
        }));
    }

    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);
    assert_eq!(
        info.thread_cycle.len(),
        3,
        "Deadlock should involve 3 threads"
    );
    println!(
        "✔ Detected 3-thread RwLock cycle deadlock: {:?}",
        info.thread_cycle
    );
}

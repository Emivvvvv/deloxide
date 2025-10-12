use deloxide::{Condvar, Mutex as DMutex, RwLock as DRwLock, thread};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
mod common;
use common::{DEADLOCK_TIMEOUT, expect_deadlock, start_detector};

#[test]
fn test_mixed_three_thread_deadlock_mutex_rwlock_condvar() {
    let harness = start_detector();

    // Resources
    let m1 = Arc::new(DMutex::new(()));
    let m2 = Arc::new(DMutex::new(()));
    let rw = Arc::new(DRwLock::new(()));
    let cv = Arc::new(Condvar::new());
    let ready = Arc::new(AtomicBool::new(false));

    // Thread A: holds m2, waits on cv, then attempts rw.write() (A -> B)
    {
        let m2 = Arc::clone(&m2);
        let rw = Arc::clone(&rw);
        let cv = Arc::clone(&cv);
        let ready = Arc::clone(&ready);
        thread::spawn(move || {
            let mut g2 = m2.lock();
            while !ready.load(Ordering::SeqCst) {
                cv.wait(&mut g2);
            }
            // m2 reacquired here; now attempt to get write on rw (will block due to reader)
            let _w = rw.write();
            let _ = &mut g2;
        });
    }

    // Thread B: holds rw.read(), then attempts to lock m1 (B -> C)
    {
        let rw = Arc::clone(&rw);
        let m1 = Arc::clone(&m1);
        thread::spawn(move || {
            let _r = rw.read();
            std::thread::sleep(Duration::from_millis(30));
            let _m1 = m1.lock();
            let _ = &_r;
        });
    }

    // Thread C: holds m1, notifies cv to wake A, then attempts m2 (C -> A)
    {
        let m1 = Arc::clone(&m1);
        let m2 = Arc::clone(&m2);
        let cv = Arc::clone(&cv);
        let ready = Arc::clone(&ready);
        thread::spawn(move || {
            let _c = m1.lock();
            // Let A start waiting and B acquire read lock
            std::thread::sleep(Duration::from_millis(20));
            ready.store(true, Ordering::SeqCst);
            cv.notify_one();
            // Give A a moment to wake and reacquire m2, then we try to get m2
            std::thread::sleep(Duration::from_millis(20));
            let _m2 = m2.lock();
            let _ = &_c;
        });
    }

    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);
    assert_eq!(info.thread_cycle.len(), 3, "Expected 3-thread cycle");
}

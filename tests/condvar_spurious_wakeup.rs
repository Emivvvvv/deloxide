use deloxide::{Condvar, Mutex as DMutex, thread};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
mod common;
use common::{NO_DEADLOCK_TIMEOUT, assert_no_deadlock, start_detector};

#[test]
fn test_condvar_spurious_wakeup_no_deadlock() {
    let harness = start_detector();

    let m = Arc::new(DMutex::new(false));
    let cv = Arc::new(Condvar::new());
    let notify_count = Arc::new(AtomicUsize::new(0));

    {
        let m = Arc::clone(&m);
        let cv = Arc::clone(&cv);
        let notify_count = Arc::clone(&notify_count);
        thread::spawn(move || {
            let mut g = m.lock();
            // Typical condvar loop against spurious wakeups
            while !*g {
                cv.wait(&mut g);
            }
            notify_count.fetch_add(1, Ordering::SeqCst);
        });
    }

    // Issue some notifications before setting predicate; these may spuriously wake
    for _ in 0..3 {
        cv.notify_one();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    // Set predicate and notify once to complete
    {
        let mut g = m.lock();
        *g = true;
    }
    cv.notify_one();

    // No deadlock should be detected; thread should either finish or keep waiting safely
    assert_no_deadlock(&harness, NO_DEADLOCK_TIMEOUT);
    // Not asserting notify_count value strictly; primary check is absence of deadlock
}

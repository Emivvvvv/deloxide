use deloxide::{Condvar, Mutex as DMutex, thread};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
mod common;
use common::{DEADLOCK_TIMEOUT, expect_deadlock, start_detector};

#[test]
fn test_condvar_cycle_deadlock() {
    let harness = start_detector();

    /* shared state */
    let m_a = Arc::new(DMutex::new(false)); // protects `ready`
    let m_b = Arc::new(DMutex::new(()));
    let cv = Arc::new(Condvar::new());
    let ready = Arc::new(AtomicBool::new(false));

    /* thread 1 : waits, then needs B ------------------------------------ */
    {
        let m_a = Arc::clone(&m_a);
        let m_b = Arc::clone(&m_b);
        let cv = Arc::clone(&cv);
        let ready = ready.clone();
        thread::spawn(move || {
            let mut guard_a = m_a.lock();
            while !*guard_a {
                cv.wait(&mut guard_a); // releases A while asleep
            }
            // now holds A again, tries to lock B  → deadlock
            let _guard_b = m_b.lock();
            ready.store(true, Ordering::SeqCst); // never reached
        });
    }

    /* thread 2 : holds B, signals, then needs A ------------------------- */
    thread::spawn({
        let m_a = Arc::clone(&m_a);
        let m_b = Arc::clone(&m_b);
        let cv = Arc::clone(&cv);
        move || {
            // Small delay to ensure thread 1 gets to wait first
            std::thread::sleep(Duration::from_millis(10));

            let _guard_b = m_b.lock(); // hold B first
            {
                let mut guard_a = m_a.lock(); // now also A
                *guard_a = true;
                cv.notify_one();
                drop(guard_a); // release A, keep B
            }

            // Small delay to let thread 1 wake up and try to get B
            std::thread::sleep(Duration::from_millis(10));

            // try to lock A again  → blocks (cycle)
            let _guard_a2 = m_a.lock();
        }
    });

    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);
    assert_eq!(info.thread_cycle.len(), 2);
    println!("✔️  Condvar cycle detected: {:?}", info.thread_cycle);
}

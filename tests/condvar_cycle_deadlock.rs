use deloxide::{
    Condvar, DeadlockInfo, Deloxide, Mutex as DMutex, Thread,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc, Mutex as StdMutex,
};
use std::time::Duration;

#[test]
fn test_condvar_cycle_deadlock() {
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();
    let detected = Arc::new(StdMutex::new(false));
    let info_slot = Arc::new(StdMutex::new(None));

    {
        let flag = detected.clone();
        let slot = info_slot.clone();
        Deloxide::new()
            .callback(move |info| {
                *flag.lock().unwrap() = true;
                *slot.lock().unwrap() = Some(info.clone());
                let _ = tx.send(info);
            })
            .start()
            .expect("detector init");
    }

    /* shared state */
    let m_a = Arc::new(DMutex::new(false));          // protects `ready`
    let m_b = Arc::new(DMutex::new(()));
    let cv   = Arc::new(Condvar::new());
    let ready = Arc::new(AtomicBool::new(false));

    /* thread 1 : waits, then needs B ------------------------------------ */
    {
        let m_a = Arc::clone(&m_a);
        let m_b = Arc::clone(&m_b);
        let cv  = Arc::clone(&cv);
        let ready = ready.clone();
        Thread::spawn(move || {
            let mut guard_a = m_a.lock();
            while !*guard_a {
                cv.wait(&mut guard_a);          // releases A while asleep
            }
            // now holds A again, tries to lock B  → deadlock
            let _guard_b = m_b.lock();
            ready.store(true, Ordering::SeqCst);     // never reached
        });
    }

    /* thread 2 : holds B, signals, then needs A ------------------------- */
    Thread::spawn({
        let m_a = Arc::clone(&m_a);
        let m_b = Arc::clone(&m_b);
        let cv  = Arc::clone(&cv);
        move || {
            // Small delay to ensure thread 1 gets to wait first
            std::thread::sleep(Duration::from_millis(10));
            
            let _guard_b = m_b.lock();               // hold B first
            {
                let mut guard_a = m_a.lock();        // now also A
                *guard_a = true;
                cv.notify_one();
                drop(guard_a);                       // release A, keep B
            }
            
            // Small delay to let thread 1 wake up and try to get B
            std::thread::sleep(Duration::from_millis(10));
            
            // try to lock A again  → blocks (cycle)
            let _guard_a2 = m_a.lock();
        }
    });

    let info = rx
        .recv_timeout(Duration::from_secs(3))
        .expect("condvar deadlock NOT detected");
    assert!(
        *detected.lock().unwrap(),
        "Deadlock flag was not raised"
    );
    assert_eq!(info.thread_cycle.len(), 2);
    println!("✔️  Condvar cycle detected: {:?}", info.thread_cycle);
}
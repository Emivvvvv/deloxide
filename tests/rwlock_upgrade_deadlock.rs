use deloxide::{DeadlockInfo, Deloxide, RwLock, Thread};
use std::sync::{
    Arc, Mutex as StdMutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

#[test]
fn test_rwlock_upgrade_deadlock() {
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();
    let detected = Arc::new(StdMutex::new(false));
    let info_slot = Arc::new(StdMutex::new(None));

    let flag = detected.clone();
    let slot = info_slot.clone();
    Deloxide::new()
        .callback(move |info| {
            *flag.lock().unwrap() = true;
            *slot.lock().unwrap() = Some(info.clone());
            let _ = tx.send(info);
        })
        .start()
        .expect("Failed to initialize detector");

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
    let timeout = Duration::from_secs(2);
    let info = rx
        .recv_timeout(timeout)
        .expect("No deadlock detected within timeout");
    assert!(*detected.lock().unwrap(), "Deadlock flag not set");
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

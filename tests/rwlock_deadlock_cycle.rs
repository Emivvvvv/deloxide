use deloxide::{DeadlockInfo, Deloxide, RwLock, Thread};
use std::sync::{Arc, Barrier, Mutex as StdMutex, mpsc};
use std::time::Duration;

#[test]
fn test_guaranteed_three_thread_rwlock_deadlock() {
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

    let lock1 = Arc::new(RwLock::new(0));
    let lock2 = Arc::new(RwLock::new(0));
    let lock3 = Arc::new(RwLock::new(0));
    let barrier = Arc::new(Barrier::new(3));

    let locks = [lock1, lock2, lock3];

    let mut handles = Vec::new();

    for i in 0..3 {
        let locks = locks.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(Thread::spawn(move || {
            // Each thread grabs read on i
            let _ri = locks[i].read();
            barrier.wait();
            // Each tries to upgrade to write on (i+1)%3 (held for read by next thread)
            let _wi_next = locks[(i + 1) % 3].write();
            // Never proceeds
        }));
    }

    let timeout = Duration::from_secs(2);
    let info = rx
        .recv_timeout(timeout)
        .expect("No deadlock detected within timeout");
    assert!(*detected.lock().unwrap(), "Deadlock flag not set");
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

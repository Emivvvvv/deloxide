use deloxide::{DeadlockInfo, Deloxide, Mutex, RwLock, Thread};
use std::sync::{Arc, Mutex as StdMutex, mpsc};
use std::thread;
use std::time::Duration;

#[test]
fn test_mutex_rwlock_deadlock() {
    // Channel for deadlock info
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();

    // Atomic flags to track detection
    let deadlock_detected = Arc::new(StdMutex::new(false));
    let deadlock_info = Arc::new(StdMutex::new(None));

    // For callback closure
    let detected_clone = Arc::clone(&deadlock_detected);
    let info_clone = Arc::clone(&deadlock_info);

    Deloxide::new()
        .callback(move |detected_info| {
            let mut detected = detected_clone.lock().unwrap();
            *detected = true;
            let mut info = info_clone.lock().unwrap();
            *info = Some(detected_info.clone());
            let _ = tx.send(detected_info);
        })
        .start()
        .expect("Failed to initialize detector");

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

    // Wait for deadlock or timeout
    let timeout = Duration::from_secs(2);
    match rx.recv_timeout(timeout) {
        Ok(info) => {
            assert!(
                *deadlock_detected.lock().unwrap(),
                "Deadlock flag should be set"
            );
            assert_eq!(
                info.thread_cycle.len(),
                2,
                "Deadlock should involve exactly 2 threads"
            );
            assert_eq!(
                info.thread_waiting_for_locks.len(),
                2,
                "There should be exactly 2 thread-lock waiting relationships"
            );
            println!(
                "✔ Detected Mutex-RwLock mixed deadlock: {:?}",
                info.thread_cycle
            );
        }
        Err(_) => {
            panic!("No deadlock detected within timeout period!");
        }
    }
}

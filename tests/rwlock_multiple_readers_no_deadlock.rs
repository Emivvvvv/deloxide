use deloxide::{DeadlockInfo, Deloxide, RwLock, Thread};
use std::sync::{Arc, Mutex as StdMutex, mpsc};
use std::thread;
use std::time::Duration;

#[test]
fn test_rwlock_multiple_readers_no_deadlock() {
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();
    let detected = Arc::new(StdMutex::new(false));
    let flag = detected.clone();

    Deloxide::new()
        .callback(move |_info| {
            *flag.lock().unwrap() = true;
            let _ = tx.send(_info);
        })
        .start()
        .expect("Failed to initialize detector");

    let lock = Arc::new(RwLock::new(42));
    let mut handles = Vec::new();

    for _ in 0..4 {
        let lock = Arc::clone(&lock);
        handles.push(Thread::spawn(move || {
            let _g = lock.read();
            thread::sleep(Duration::from_millis(50));
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // There should be no deadlock notification
    let timeout = Duration::from_millis(500);
    assert!(
        rx.recv_timeout(timeout).is_err(),
        "False deadlock detected with multiple readers!"
    );
    assert!(
        !*detected.lock().unwrap(),
        "Deadlock flag should not be set"
    );
}

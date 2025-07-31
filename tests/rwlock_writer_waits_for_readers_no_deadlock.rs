use deloxide::{DeadlockInfo, Deloxide, RwLock, Thread};
use std::sync::{Arc, Mutex as StdMutex, mpsc};
use std::thread;
use std::time::Duration;

#[test]
fn test_rwlock_writer_waits_for_readers_no_deadlock() {
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
    let l1 = Arc::clone(&lock);
    let l2 = Arc::clone(&lock);

    // One thread grabs a read lock for a while
    let reader = Thread::spawn(move || {
        let _g = l1.read();
        thread::sleep(Duration::from_millis(100));
    });

    // Let reader get the lock
    thread::sleep(Duration::from_millis(10));

    // Writer will block until reader is done (but not a deadlock!)
    let writer = Thread::spawn(move || {
        let _g = l2.write();
        // Should succeed after reader is done
    });

    reader.join().unwrap();
    writer.join().unwrap();

    // There should be no deadlock notification
    let timeout = Duration::from_millis(300);
    assert!(
        rx.recv_timeout(timeout).is_err(),
        "False deadlock detected with writer waiting for readers"
    );
    assert!(
        !*detected.lock().unwrap(),
        "Deadlock flag should not be set"
    );
}

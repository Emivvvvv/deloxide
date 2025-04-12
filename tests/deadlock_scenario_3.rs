use deloxide::{DeadlockInfo, Deloxide, TrackedMutex};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

#[test]
fn test_partial_deadlock_with_some_threads_finishing() {
    // Create a channel to receive deadlock detection information.
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();

    // Use Arc and Mutex to share an indicator and detailed info for the detected deadlock.
    let deadlock_detected = Arc::new(Mutex::new(false));
    let deadlock_info = Arc::new(Mutex::new(None));

    let deadlock_detected_cb = Arc::clone(&deadlock_detected);
    let deadlock_info_cb = Arc::clone(&deadlock_info);

    Deloxide::new()
        .callback(move |detected_info| {
            // Mark that a deadlock has been detected.
            let mut detected = deadlock_detected_cb.lock().unwrap();
            *detected = true;

            // Save the detailed deadlock info.
            let mut info = deadlock_info_cb.lock().unwrap();
            *info = Some(detected_info.clone());

            // Send the deadlock info via our channel.
            let _ = tx.send(detected_info);
        })
        .start()
        .expect("Failed to start the deadlock detector");

    // Setup shared resources (tracked mutexes).
    // Two resources will be used in a circular deadlock.
    let mutex_a = Arc::new(TrackedMutex::new("Resource A"));
    let mutex_b = Arc::new(TrackedMutex::new("Resource B"));
    // A third resource is used by a non-deadlocking thread.
    let mutex_c = Arc::new(TrackedMutex::new("Resource C"));

    // --- Deadlock-inducing threads ---

    // Thread 1: Locks Resource A, then after a short sleep attempts to lock Resource B.
    let a_thread1 = Arc::clone(&mutex_a);
    let b_thread1 = Arc::clone(&mutex_b);
    let deadlock_thread1 = thread::spawn(move || {
        let _guard_a = a_thread1.lock().unwrap();
        thread::sleep(Duration::from_millis(100));
        // This call will block if Resource B is already held.
        let _guard_b = b_thread1.lock().unwrap();
    });

    // Thread 2: Locks Resource B, then attempts to lock Resource A.
    let b_thread2 = Arc::clone(&mutex_b);
    let a_thread2 = Arc::clone(&mutex_a);
    let deadlock_thread2 = thread::spawn(move || {
        let _guard_b = b_thread2.lock().unwrap();
        thread::sleep(Duration::from_millis(100));
        // This call will block if Resource A is already held.
        let _guard_a = a_thread2.lock().unwrap();
    });

    // --- Non-deadlocking thread ---

    // Thread 3: Uses Resource C only and finishes normally.
    let c_thread3 = Arc::clone(&mutex_c);
    let non_deadlock_thread = thread::spawn(move || {
        let _guard_c = c_thread3.lock().unwrap();
        // Simulate some work that completes before the deadlock is detected.
        thread::sleep(Duration::from_millis(150));
        // The lock is automatically released when the scope ends.
    });

    // Wait for the non-deadlocking thread to finish.
    non_deadlock_thread
        .join()
        .expect("Thread 3 should complete normally");

    // Wait for deadlock detection (with a timeout).
    // In this example, only threads 1 and 2 are deadlocked.
    let timeout = Duration::from_secs(2);
    match rx.recv_timeout(timeout) {
        Ok(info) => {
            // Verify that the deadlock cycle involves exactly 2 threads.
            assert_eq!(
                info.thread_cycle.len(),
                2,
                "Deadlock should involve exactly 2 threads"
            );
            // Verify that there are 2 thread-lock waiting relationships.
            assert_eq!(
                info.thread_waiting_for_locks.len(),
                2,
                "There should be exactly 2 thread-lock waiting relationships"
            );
        }
        Err(_) => {
            panic!("Deadlock was not detected within the timeout period!");
        }
    }

    // Note:
    // Attempting to join the deadlocked threads here would block indefinitely.
    // In a real-world test, you may instead detach them or structure your test
    // to avoid waiting on deadlocked threads.
    let _ = deadlock_thread1.thread();
    let _ = deadlock_thread2.thread();
}

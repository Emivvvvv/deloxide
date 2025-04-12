use deloxide::{DeadlockInfo, Deloxide, TrackedMutex};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

#[test]
fn test_deadlock_detection() {
    // Create a channel to communicate deadlock detection
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();

    // Track whether deadlock was detected with an atomic flag
    let deadlock_detected = Arc::new(Mutex::new(false));
    let deadlock_info = Arc::new(Mutex::new(None));

    // Clone for the callback
    let deadlock_detected_clone = Arc::clone(&deadlock_detected);
    let deadlock_info_clone = Arc::clone(&deadlock_info);

    Deloxide::new()
        .callback(move |detected_info| {
            // Set the flag indicating deadlock was detected
            let mut detected = deadlock_detected_clone.lock().unwrap();
            *detected = true;

            // Store deadlock info for later verification
            let mut info = deadlock_info_clone.lock().unwrap();
            *info = Some(detected_info.clone());

            // Also send through channel
            let _ = tx.send(detected_info);
        })
        .start()
        .expect("Failed to initialize detector");

    // Create two mutexes
    let mutex_a = Arc::new(TrackedMutex::new("Resource A"));
    let mutex_b = Arc::new(TrackedMutex::new("Resource B"));

    // Clone references for the second thread
    let mutex_a_clone = Arc::clone(&mutex_a);
    let mutex_b_clone = Arc::clone(&mutex_b);

    // Thread 1: Lock A, then try to lock B
    let _thread1 = thread::spawn(move || {
        let _guard_a = mutex_a.lock().unwrap();

        // Give thread 2 time to acquire lock B
        thread::sleep(Duration::from_millis(100));

        // This will cause a deadlock
        let _guard_b = mutex_b.lock().unwrap();

        // We shouldn't reach here if deadlock is detected
        false
    });

    // Thread 2: Lock B, then try to lock A
    let _thread2 = thread::spawn(move || {
        let _guard_b = mutex_b_clone.lock().unwrap();

        // Give thread 1 time to acquire lock A
        thread::sleep(Duration::from_millis(100));

        // This will cause a deadlock
        let _guard_a = mutex_a_clone.lock().unwrap();

        // We shouldn't reach here if deadlock is detected
        false
    });

    // Wait for a reasonable time to allow deadlock to be detected
    let timeout = Duration::from_secs(2);
    match rx.recv_timeout(timeout) {
        Ok(info) => {
            // Verify deadlock was detected
            assert!(
                *deadlock_detected.lock().unwrap(),
                "Deadlock flag should be set"
            );

            // Verify cycle has exactly 2 threads (our specific case)
            assert_eq!(
                info.thread_cycle.len(),
                2,
                "Deadlock should involve exactly 2 threads"
            );

            // Verify there are 2 thread-lock waiting relationships
            assert_eq!(
                info.thread_waiting_for_locks.len(),
                2,
                "There should be exactly 2 thread-lock waiting relationships"
            );

            // Test passed
        }
        Err(_) => {
            panic!("No deadlock detected within timeout period!");
        }
    }
}

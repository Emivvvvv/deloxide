use deloxide::{DeadlockInfo, Deloxide, TrackedMutex};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

#[test]
fn test_partial_deadlock_with_completing_thread() {
    // Create a channel to receive deadlock detection info.
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();

    // Flag to indicate deadlock detection.
    let deadlock_detected = Arc::new(Mutex::new(false));
    // To store the detailed deadlock info.
    let deadlock_info = Arc::new(Mutex::new(None));

    // Cloning for use within the callback.
    let deadlock_detected_clone = Arc::clone(&deadlock_detected);
    let deadlock_info_clone = Arc::clone(&deadlock_info);

    Deloxide::new()
        .callback(move |detected_info| {
            // Set our detected flag to true.
            let mut detected = deadlock_detected_clone.lock().unwrap();
            *detected = true;

            // Save the detailed deadlock info.
            let mut info = deadlock_info_clone.lock().unwrap();
            *info = Some(detected_info.clone());

            // Also send the info via our communication channel.
            let _ = tx.send(detected_info);
        })
        .start()
        .expect("Failed to initialize deadlock detector");

    // Create four shared resources guarded by tracked mutexes.
    let mutex_a = Arc::new(TrackedMutex::new("Resource A"));
    let mutex_b = Arc::new(TrackedMutex::new("Resource B"));
    let mutex_c = Arc::new(TrackedMutex::new("Resource C"));
    let mutex_d = Arc::new(TrackedMutex::new("Resource D"));

    // Thread 1: Locks Resource A, then after a short delay tries to lock Resource B.
    let a_t1 = Arc::clone(&mutex_a);
    let b_t1 = Arc::clone(&mutex_b);
    let _thread1 = thread::spawn(move || {
        let _guard_a = a_t1.lock().unwrap();
        thread::sleep(Duration::from_millis(100));
        // This call will block when Resource B is already held by another thread.
        let _guard_b = b_t1.lock().unwrap();
    });

    // Thread 2: Locks Resource B, then tries to lock Resource C.
    let b_t2 = Arc::clone(&mutex_b);
    let c_t2 = Arc::clone(&mutex_c);
    let _thread2 = thread::spawn(move || {
        let _guard_b = b_t2.lock().unwrap();
        thread::sleep(Duration::from_millis(100));
        let _guard_c = c_t2.lock().unwrap();
    });

    // Thread 3: Locks Resource C, then tries to lock Resource D.
    let c_t3 = Arc::clone(&mutex_c);
    let d_t3 = Arc::clone(&mutex_d);
    let _thread3 = thread::spawn(move || {
        let _guard_c = c_t3.lock().unwrap();
        thread::sleep(Duration::from_millis(100));
        let _guard_d = d_t3.lock().unwrap();
    });

    // Thread 4: Locks Resource D, then tries to lock Resource A—closing the dependency cycle.
    let d_t4 = Arc::clone(&mutex_d);
    let a_t4 = Arc::clone(&mutex_a);
    let _thread4 = thread::spawn(move || {
        let _guard_d = d_t4.lock().unwrap();
        thread::sleep(Duration::from_millis(100));
        let _guard_a = a_t4.lock().unwrap();
    });

    // Wait for deadlock detection with a generous timeout.
    let timeout = Duration::from_secs(3);
    match rx.recv_timeout(timeout) {
        Ok(info) => {
            // Verify that the cycle includes exactly 4 threads.
            assert_eq!(
                info.thread_cycle.len(),
                4,
                "Deadlock should involve exactly 4 threads"
            );
            // Verify that there are 4 thread-lock waiting relationships.
            assert_eq!(
                info.thread_waiting_for_locks.len(),
                4,
                "There should be exactly 4 thread-lock waiting relationships"
            );
        }
        Err(_) => {
            panic!("No deadlock detected within the timeout period in the complex scenario!");
        }
    }
}

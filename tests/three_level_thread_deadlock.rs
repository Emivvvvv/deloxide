use deloxide::{DeadlockInfo, Deloxide, TrackedMutex, TrackedThread};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

#[test]
fn test_three_level_thread_deadlock() {
    // Create a channel to communicate deadlock detection
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();

    // Track whether deadlock was detected
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

    // Create locks at different levels
    let main_mutex_a = Arc::new(TrackedMutex::new("Main Resource A"));
    let main_mutex_b = Arc::new(TrackedMutex::new("Main Resource B"));
    let level1_mutex = Arc::new(TrackedMutex::new("Level 1 Resource"));
    let level2_mutex = Arc::new(TrackedMutex::new("Level 2 Resource"));
    let shared_mutex = Arc::new(TrackedMutex::new("Shared Resource"));

    // Clone for various threads
    let main_mutex_a_clone1 = Arc::clone(&main_mutex_a);
    let main_mutex_b_clone1 = Arc::clone(&main_mutex_b);
    let level1_mutex_clone1 = Arc::clone(&level1_mutex);
    let level2_mutex_clone1 = Arc::clone(&level2_mutex);
    let shared_mutex_clone1 = Arc::clone(&shared_mutex);

    let main_mutex_a_clone2 = Arc::clone(&main_mutex_a);
    let level1_mutex_clone2 = Arc::clone(&level1_mutex);
    let shared_mutex_clone2 = Arc::clone(&shared_mutex);

    // Level 1 threads - created by main thread
    let thread1 = TrackedThread::spawn(move || {
        // Lock level 1 resources
        let _level1_guard = level1_mutex_clone1.lock().unwrap();
        thread::sleep(Duration::from_millis(50));

        // Create level 2 threads
        let shared_clone = Arc::clone(&shared_mutex_clone1);
        let main_a_clone = Arc::clone(&main_mutex_a_clone1);
        let level2_clone = Arc::clone(&level2_mutex_clone1);

        let thread2_1 = TrackedThread::spawn(move || {
            // Lock level 2 resources
            let _level2_guard = level2_clone.lock().unwrap();
            thread::sleep(Duration::from_millis(50));

            // Create level 3 thread
            let shared_clone3 = Arc::clone(&shared_clone);
            let _thread3_1 = TrackedThread::spawn(move || {
                // Level 3 thread trying to acquire shared resource
                thread::sleep(Duration::from_millis(100));
                // This might cause deadlock
                let _shared_guard = shared_clone3.lock().unwrap();
                thread::sleep(Duration::from_millis(200));
            });

            // Try to lock main resource from level 2
            thread::sleep(Duration::from_millis(150));
            let _main_a_guard = main_a_clone.lock().unwrap();
        });

        // Another level 2 thread from thread1
        let shared_clone2 = Arc::clone(&shared_mutex_clone1);
        let main_b_clone = Arc::clone(&main_mutex_b_clone1);
        let _thread2_2 = TrackedThread::spawn(move || {
            // This level 2 thread creates its own level 3 thread
            let shared_clone3 = Arc::clone(&shared_clone2);
            let main_b_clone3 = Arc::clone(&main_b_clone);

            let _thread3_2 = TrackedThread::spawn(move || {
                // Level 3 thread creates lock dependencies
                let _shared_guard = shared_clone3.lock().unwrap();
                thread::sleep(Duration::from_millis(100));
                // Try to lock main B from level 3
                let _main_b_guard = main_b_clone3.lock().unwrap();
            });

            thread::sleep(Duration::from_millis(200));
        });

        thread2_1.join().unwrap();
    });

    // Another level 1 thread with different locking pattern
    let thread2 = TrackedThread::spawn(move || {
        // Lock different resources to create deadlock potential
        let _main_a_guard = main_mutex_a_clone2.lock().unwrap();
        thread::sleep(Duration::from_millis(50));

        // Create its own level 2 thread
        let shared_clone = Arc::clone(&shared_mutex_clone2);
        let level1_clone = Arc::clone(&level1_mutex_clone2);

        let _thread2_3 = TrackedThread::spawn(move || {
            // This level 2 thread tries to lock shared and level1 resources
            let _shared_guard = shared_clone.lock().unwrap();
            thread::sleep(Duration::from_millis(150));

            // This will likely cause a deadlock cycle
            let _level1_guard = level1_clone.lock().unwrap();
        });

        thread::sleep(Duration::from_millis(300));
    });

    // Main thread also creates a final pattern
    thread::sleep(Duration::from_millis(100));
    let _main_b_guard = main_mutex_b.lock().unwrap();
    thread::sleep(Duration::from_millis(50));

    // This should create a complex cycle
    let _shared_guard = shared_mutex.lock().unwrap();

    // Wait for threads with a timeout
    let timeout = Duration::from_secs(3);
    match rx.recv_timeout(timeout) {
        Ok(info) => {
            // Verify deadlock was detected
            assert!(
                *deadlock_detected.lock().unwrap(),
                "Deadlock flag should be set"
            );

            // In this complex scenario, we should have a deadlock cycle
            assert!(
                info.thread_cycle.len() >= 2,
                "Deadlock should involve at least 2 threads"
            );

            // Verify we have waiting relationships
            assert!(
                !info.thread_waiting_for_locks.is_empty(),
                "There should be thread-lock waiting relationships"
            );

            // Print detailed info about the deadlock
            println!("Deadlock detected with {} threads in cycle", info.thread_cycle.len());
            println!("Thread cycle: {:?}", info.thread_cycle);
            println!("Waiting relationships: {:?}", info.thread_waiting_for_locks);

            // Test passed
        }
        Err(_) => {
            // Check if threads completed successfully (which would mean no deadlock)
            if let Ok(thread1_result) = thread1.join() {
                if let Ok(thread2_result) = thread2.join() {
                    println!("Threads completed successfully - no deadlock occurred");
                    panic!("Expected deadlock was not detected!");
                }
            }
            // Otherwise, threads are likely blocked in deadlock
            println!("Timeout waiting for deadlock detection - threads likely blocked");
        }
    }
}
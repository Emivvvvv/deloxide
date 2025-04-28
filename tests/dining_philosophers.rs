use deloxide::{DeadlockInfo, Deloxide, TrackedMutex, TrackedThread};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

#[test]
fn test_dining_philosophers_deadlock() {
    // Create a channel to communicate deadlock detection
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();

    // Track whether deadlock was detected
    let deadlock_detected = Arc::new(Mutex::new(false));
    let deadlock_info = Arc::new(Mutex::new(None));

    // Clone for the callback
    let deadlock_detected_clone = Arc::clone(&deadlock_detected);
    let deadlock_info_clone = Arc::clone(&deadlock_info);

    // Initialize deadlock detector
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

    // Number of philosophers
    let num_philosophers = 5;

    // Create the forks (shared resources)
    let forks: Vec<Arc<TrackedMutex<String>>> = (0..num_philosophers)
        .map(|i| Arc::new(TrackedMutex::new(format!("Fork {}", i))))
        .collect();

    // Create philosophers (threads)
    let mut handles = vec![];

    for i in 0..num_philosophers {
        // Each philosopher needs two forks
        let left_fork = Arc::clone(&forks[i]);
        let right_fork = Arc::clone(&forks[(i + 1) % num_philosophers]);

        // Philosopher thread
        let handle = TrackedThread::spawn(move || {
            // Try to take the left fork first
            println!("Philosopher {} is trying to take left fork", i);
            let _left = left_fork.lock().unwrap();
            println!("Philosopher {} acquired left fork", i);

            // Small delay to increase chance of deadlock
            thread::sleep(Duration::from_millis(100));

            // Then try to take the right fork
            println!("Philosopher {} is trying to take right fork", i);
            let _right = right_fork.lock().unwrap();
            println!("Philosopher {} acquired right fork and is eating", i);

            // Eat for a while
            thread::sleep(Duration::from_millis(500));

            println!("Philosopher {} is done eating", i);
        });

        handles.push(handle);
    }

    // Wait for a reasonable time to allow deadlock to be detected
    let timeout = Duration::from_secs(3);
    match rx.recv_timeout(timeout) {
        Ok(info) => {
            // Verify deadlock was detected
            assert!(
                *deadlock_detected.lock().unwrap(),
                "Deadlock flag should be set"
            );

            // With all philosophers taking left fork first, we should get a cycle
            // The cycle length depends on timing, but should be at least 2
            assert!(
                info.thread_cycle.len() >= 2,
                "Deadlock should involve at least 2 threads"
            );

            // There should be waiting relationships for each philosopher in the cycle
            assert!(
                !info.thread_waiting_for_locks.is_empty(),
                "There should be thread-lock waiting relationships"
            );

            println!("Successfully detected dining philosophers deadlock!");
            println!("Thread cycle: {:?}", info.thread_cycle);
            println!(
                "Thread waiting for locks: {:?}",
                info.thread_waiting_for_locks
            );
        }
        Err(_) => {
            panic!("No deadlock detected within timeout period!");
        }
    }

    // Don't wait for threads to complete since they're deadlocked
    println!("Test complete - threads are intentionally left running");
}

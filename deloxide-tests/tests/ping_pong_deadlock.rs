// deloxide-tests/tests/ping_pong_deadlock.rs
#[cfg(feature = "detector")]
mod test {
    use deloxide_tests::*;
    use deloxide::DeadlockInfo;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;
    use std::thread;

    #[test]
    fn test_ping_pong_deadlock() {
        let (tx, rx) = mpsc::channel();

        // Initialize detector with callback
        maybe_start_detector(move |info: DeadlockInfo| {
            tx.send(info).unwrap();
        });

        // Create two mutexes
        let mutex_a = new_arc_mutex("A");
        let mutex_b = new_arc_mutex("B");

        let mutex_a_clone = Arc::clone(&mutex_a);
        let mutex_b_clone = Arc::clone(&mutex_b);

        // Thread 1: Lock A then B
        let _t1 = spawn_thread(move || {
            let _a = mutex_a.lock();
            thread::sleep(Duration::from_millis(50));
            let _b = mutex_b.lock();
            // This thread will be stuck here
            thread::park();
        });

        // Thread 2: Lock B then A (creates deadlock)
        let _t2 = spawn_thread(move || {
            let _b = mutex_b_clone.lock();
            thread::sleep(Duration::from_millis(50));
            let _a = mutex_a_clone.lock();
            // This thread will be stuck here
            thread::park();
        });

        // Wait for deadlock detection
        let deadlock_info = rx.recv_timeout(Duration::from_secs(2))
            .expect("Deadlock should be detected");

        // Verify the deadlock info
        assert_eq!(deadlock_info.thread_cycle.len(), 2);
        assert!(deadlock_info.thread_waiting_for_locks.len() >= 2);
    }
}
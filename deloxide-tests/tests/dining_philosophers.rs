// deloxide-tests/tests/dining_philosophers.rs
#[cfg(feature = "detector")]
mod test {
    use deloxide_tests::*;
    use deloxide::DeadlockInfo;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;
    use std::thread;

    #[test]
    fn test_dining_philosophers() {
        let (tx, rx) = mpsc::channel();

        maybe_start_detector(move |info: DeadlockInfo| {
            tx.send(info).unwrap();
        });

        const N: usize = 5;
        let forks: Vec<_> = (0..N).map(|i| new_arc_mutex(i)).collect();
        let mut handles = vec![];

        for i in 0..N {
            let left_fork = Arc::clone(&forks[i]);
            let right_fork = Arc::clone(&forks[(i + 1) % N]);

            let handle = spawn_thread(move || {
                // Each philosopher tries to pick up left fork first, then right
                let _left = left_fork.lock();
                thread::sleep(Duration::from_millis(50));
                let _right = right_fork.lock();

                // This thread will be stuck here
                thread::park();
            });

            handles.push(handle);
        }

        // Wait for deadlock detection
        let deadlock_info = rx.recv_timeout(Duration::from_secs(2))
            .expect("Deadlock should be detected");

        // Verify the deadlock involves all philosophers
        assert_eq!(deadlock_info.thread_cycle.len(), N);
        assert!(deadlock_info.thread_waiting_for_locks.len() >= N);
    }
}
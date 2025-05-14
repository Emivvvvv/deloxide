// deloxide-tests/tests/no_deadlock.rs
mod test {
    use deloxide_tests::*;
    use std::sync::Arc;
    use std::time::Duration;
    use std::thread;

    #[test]
    fn test_no_deadlock_scenario() {
        // This test doesn't need detector - just verifies no false positives
        let mutex_a = new_arc_mutex("A");
        let mutex_b = new_arc_mutex("B");

        let mutex_a_clone = Arc::clone(&mutex_a);
        let mutex_b_clone = Arc::clone(&mutex_b);

        // Both threads acquire locks in the same order
        let t1 = spawn_thread(move || {
            let _a = mutex_a.lock();
            thread::sleep(Duration::from_millis(10));
            let _b = mutex_b.lock();
        });

        let t2 = spawn_thread(move || {
            thread::sleep(Duration::from_millis(20)); // Ensure t1 goes first
            let _a = mutex_a_clone.lock();
            thread::sleep(Duration::from_millis(10));
            let _b = mutex_b_clone.lock();
        });

        // Should complete successfully
        t1.join().unwrap();
        t2.join().unwrap();
    }
}
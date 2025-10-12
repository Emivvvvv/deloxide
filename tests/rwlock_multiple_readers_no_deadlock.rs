use deloxide::{RwLock, thread};
use std::sync::Arc;
use std::time::Duration;
mod common;
use common::{NO_DEADLOCK_TIMEOUT, assert_no_deadlock, start_detector};

#[test]
fn test_rwlock_multiple_readers_no_deadlock() {
    let harness = start_detector();

    let lock = Arc::new(RwLock::new(42));
    let mut handles = Vec::new();

    for _ in 0..4 {
        let lock = Arc::clone(&lock);
        handles.push(thread::spawn(move || {
            let _g = lock.read();
            thread::sleep(Duration::from_millis(50));
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_no_deadlock(&harness, NO_DEADLOCK_TIMEOUT);
}

use deloxide::{RwLock, Thread};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
mod common;
use common::{NO_DEADLOCK_TIMEOUT, assert_no_deadlock, start_detector};

#[test]
fn test_rwlock_writer_waits_for_readers_no_deadlock() {
    let harness = start_detector();

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
    assert_no_deadlock(&harness, NO_DEADLOCK_TIMEOUT);
}

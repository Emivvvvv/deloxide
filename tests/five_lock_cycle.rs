use deloxide::{Mutex, thread};
use std::sync::Arc;
use std::time::Duration;

mod common;
use common::{expect_deadlock, start_detector, DEADLOCK_TIMEOUT};

#[test]
fn test_five_lock_cycle_deadlock() {
    let harness = start_detector();

    // 5 locks to be acquired in a circular pattern
    let a = Arc::new(Mutex::new(()));
    let b = Arc::new(Mutex::new(()));
    let c = Arc::new(Mutex::new(()));
    let d = Arc::new(Mutex::new(()));
    let e = Arc::new(Mutex::new(()));

    // Create 5 threads with circular dependencies
    let mut handles = vec![];

    // Thread 0: A -> B -> C -> D -> E
    {
        let (l0, l1, l2, l3, l4) = (a.clone(), b.clone(), c.clone(), d.clone(), e.clone());
        handles.push(thread::spawn(move || {
            let _g0 = l0.lock();
            thread::sleep(Duration::from_millis(50));
            let _g1 = l1.lock();
            thread::sleep(Duration::from_millis(50));
            let _g2 = l2.lock();
            thread::sleep(Duration::from_millis(50));
            let _g3 = l3.lock();
            thread::sleep(Duration::from_millis(50));
            let _g4 = l4.lock();
        }));
    }

    // Thread 1: B -> C -> D -> E -> A
    {
        let (l0, l1, l2, l3, l4) = (b.clone(), c.clone(), d.clone(), e.clone(), a.clone());
        handles.push(thread::spawn(move || {
            thread::sleep(Duration::from_micros(100)); // Stagger start
            let _g0 = l0.lock();
            thread::sleep(Duration::from_millis(50));
            let _g1 = l1.lock();
            thread::sleep(Duration::from_millis(50));
            let _g2 = l2.lock();
            thread::sleep(Duration::from_millis(50));
            let _g3 = l3.lock();
            thread::sleep(Duration::from_millis(50));
            let _g4 = l4.lock();
        }));
    }

    // Thread 2: C -> D -> E -> A -> B
    {
        let (l0, l1, l2, l3, l4) = (c.clone(), d.clone(), e.clone(), a.clone(), b.clone());
        handles.push(thread::spawn(move || {
            thread::sleep(Duration::from_micros(200)); // Stagger start
            let _g0 = l0.lock();
            thread::sleep(Duration::from_millis(50));
            let _g1 = l1.lock();
            thread::sleep(Duration::from_millis(50));
            let _g2 = l2.lock();
            thread::sleep(Duration::from_millis(50));
            let _g3 = l3.lock();
            thread::sleep(Duration::from_millis(50));
            let _g4 = l4.lock();
        }));
    }

    // Thread 3: D -> E -> A -> B -> C
    {
        let (l0, l1, l2, l3, l4) = (d.clone(), e.clone(), a.clone(), b.clone(), c.clone());
        handles.push(thread::spawn(move || {
            thread::sleep(Duration::from_micros(300)); // Stagger start
            let _g0 = l0.lock();
            thread::sleep(Duration::from_millis(50));
            let _g1 = l1.lock();
            thread::sleep(Duration::from_millis(50));
            let _g2 = l2.lock();
            thread::sleep(Duration::from_millis(50));
            let _g3 = l3.lock();
            thread::sleep(Duration::from_millis(50));
            let _g4 = l4.lock();
        }));
    }

    // Thread 4: E -> A -> B -> C -> D
    {
        let (l0, l1, l2, l3, l4) = (e.clone(), a.clone(), b.clone(), c.clone(), d.clone());
        handles.push(thread::spawn(move || {
            thread::sleep(Duration::from_micros(400)); // Stagger start
            let _g0 = l0.lock();
            thread::sleep(Duration::from_millis(50));
            let _g1 = l1.lock();
            thread::sleep(Duration::from_millis(50));
            let _g2 = l2.lock();
            thread::sleep(Duration::from_millis(50));
            let _g3 = l3.lock();
            thread::sleep(Duration::from_millis(50));
            let _g4 = l4.lock();
        }));
    }

    // Wait for the deadlock to be detected
    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);

    // Verify the deadlock report
    assert_eq!(
        info.thread_cycle.len(),
        5,
        "Deadlock should involve all 5 threads"
    );
    assert!(
        !info.thread_waiting_for_locks.is_empty(),
        "There should be thread-lock waiting relationships"
    );

    // Don't wait for threads to complete since they're deadlocked
    println!("Test complete - threads are intentionally left running in a deadlock.");
}

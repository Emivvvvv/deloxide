use deloxide::{DeadlockInfo, Deloxide, TrackedMutex};  // :contentReference[oaicite:0]{index=0}
use rand::Rng;
use std::{
    sync::{Arc, Barrier, Mutex, mpsc},
    thread,
    time::Duration,
};

#[test]
fn test_random_ring_deadlock() {
    // Channel to receive the deadlock info
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();

    // Shared flag & slot for assertions
    let detected  = Arc::new(Mutex::new(false));
    let info_slot = Arc::new(Mutex::new(None));

    // Initialize Deloxide with our callback
    let flag = detected.clone();
    let slot = info_slot.clone();
    Deloxide::new()
        .with_log("tests/random_ring_deadlock.log")
        .callback(move |info| {
            *flag.lock().unwrap() = true;
            *slot.lock().unwrap() = Some(info.clone());
            let _ = tx.send(info);
        })
        .start()
        .expect("Failed to initialize detector");

    // Pick a random ring size between 3 and 8
    let mut rng = rand::rng();
    let n = rng.random_range(3..=8);
    println!("→ testing a ring of {} threads", n);

    // Build n locks in a ring
    let locks: Vec<_> = (0..n)
        .map(|i| Arc::new(TrackedMutex::new(format!("L{}", i))))
        .collect();

    // Barrier so all threads start together
    let barrier = Arc::new(Barrier::new(n));
    let mut handles = Vec::with_capacity(n);

    for i in 0..n {
        let first  = locks[i].clone();
        let second = locks[(i + 1) % n].clone();
        let bar    = barrier.clone();

        handles.push(thread::spawn(move || {
            let mut rng = rand::rng();

            // Rendezvous
            bar.wait();

            // Random jitter before first lock
            thread::sleep(Duration::from_millis(rng.random_range(0..50)));
            let _a = first.lock().unwrap();

            // Random jitter before second lock
            thread::sleep(Duration::from_millis(rng.random_range(50..100)));
            let _b = second.lock().unwrap();

            // If there were no deadlock, we'd proceed:
            thread::sleep(Duration::from_millis(200));
        }));
    }

    // Wait up to 5s for the callback
    let timeout = Duration::from_secs(5);
    let info = rx
        .recv_timeout(timeout)
        .expect(&format!("No deadlock detected within {:?}", timeout));

    // Verify
    assert!(*detected.lock().unwrap(), "Deadlock flag not set");
    assert_eq!(
        info.thread_cycle.len(),
        n,
        "Expected a cycle of length {}, got {:?}",
        n,
        info.thread_cycle
    );
    assert!(
        !info.thread_waiting_for_locks.is_empty(),
        "No waiting relationships recorded"
    );

    println!("✔ detected {}-cycle deadlock: {:?}", n, info.thread_cycle);

    // Threads remain deadlocked; we don't join them.
}
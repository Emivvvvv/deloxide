use deloxide::{DeadlockInfo, Deloxide, Mutex, Thread};
use rand::Rng;
use std::{
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

#[test]
fn test_random_ring_deadlock() {
    // Channel to receive the deadlock info
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();

    // Shared flag & slot for assertions
    let detected = Arc::new(StdMutex::new(false));
    let info_slot = Arc::new(StdMutex::new(None));

    // Initialize Deloxide with our callback
    let flag = detected.clone();
    let slot = info_slot.clone();
    Deloxide::new()
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
        .map(|i| Arc::new(Mutex::new(format!("L{}", i))))
        .collect();

    // Counter to ensure all threads start together
    let ready_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(n);

    for i in 0..n {
        let first = locks[i].clone();
        let second = locks[(i + 1) % n].clone();
        let ready = ready_count.clone();

        handles.push(Thread::spawn(move || {
            let mut rng = rand::rng();

            // Signal ready and wait for all threads
            ready.fetch_add(1, Ordering::SeqCst);
            while ready.load(Ordering::SeqCst) < n {
                thread::yield_now();
            }

            // Random jitter before first lock
            thread::sleep(Duration::from_millis(rng.random_range(0..50)));
            let _a = first.lock();

            // Random jitter before second lock
            thread::sleep(Duration::from_millis(rng.random_range(50..100)));
            let _b = second.lock();

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

use deloxide::{DeadlockInfo, Deloxide, Mutex, thread};
use std::sync::{Arc, Barrier};

fn main() {
    // ANCHOR: setup
    Deloxide::new()
        .callback(|info: DeadlockInfo| {
            eprintln!("source: {:?}", info.source);
            eprintln!("thread cycle: {:?}", info.thread_cycle);
            eprintln!(
                "thread waiting for locks: {:?}",
                info.thread_waiting_for_locks
            );
            std::process::exit(0);
        })
        .start()
        .expect("detector initialization");

    let left = Arc::new(Mutex::new(()));
    let right = Arc::new(Mutex::new(()));
    let first_locks_acquired = Arc::new(Barrier::new(2));
    // ANCHOR_END: setup

    // ANCHOR: cycle
    let left_for_first = Arc::clone(&left);
    let right_for_first = Arc::clone(&right);
    let barrier_for_first = Arc::clone(&first_locks_acquired);
    let first = thread::spawn(move || {
        let _left = left_for_first.lock();
        barrier_for_first.wait();
        let _right = right_for_first.lock();
    });

    let left_for_second = Arc::clone(&left);
    let right_for_second = Arc::clone(&right);
    let barrier_for_second = Arc::clone(&first_locks_acquired);
    let second = thread::spawn(move || {
        let _right = right_for_second.lock();
        barrier_for_second.wait();
        let _left = left_for_second.lock();
    });

    let _workers = [first, second];
    // ANCHOR_END: cycle

    thread::park();
}

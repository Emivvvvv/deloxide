use deloxide::{Condvar, Deloxide, Mutex};
use std::sync::Arc;

fn main() {
    Deloxide::new().start().expect("detector initialization");
    let state = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_state = Arc::clone(&state);
    let worker = std::thread::spawn(move || {
        let (lock, ready) = &*worker_state;
        let mut guard = lock.lock();
        while !*guard {
            ready.wait(&mut guard);
        }
    });

    let (lock, ready) = &*state;
    *lock.lock() = true;
    ready.notify_one();
    worker.join().unwrap();
}

use deloxide::{Deloxide, Mutex};

fn main() {
    Deloxide::new()
        .callback(|info| eprintln!("deadlock: {:?}", info.thread_cycle))
        .start()
        .expect("detector initialization");

    let value = Mutex::new(41);
    *value.lock() += 1;
    assert_eq!(*value.lock(), 42);
}

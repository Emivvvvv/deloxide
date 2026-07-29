use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::Arc;

fn mutex_uncontended(c: &mut Criterion) {
    let lock = deloxide::Mutex::new(0_u64);
    c.bench_function("mutex/uncontended", |b| {
        b.iter(|| {
            let guard = lock.lock();
            black_box(*guard);
        });
    });

    let lock_without_read = deloxide::Mutex::new(0_u64);
    c.bench_function("mutex/uncontended_lock_drop", |b| {
        b.iter(|| {
            let guard = lock_without_read.lock();
            black_box(&guard);
        });
    });
}

fn parking_lot_mutex_controls(c: &mut Criterion) {
    let lock = parking_lot::Mutex::new(0_u64);
    c.bench_function("parking_lot/mutex_uncontended", |b| {
        b.iter(|| {
            let guard = lock.lock();
            black_box(*guard);
        });
    });

    let try_lock = parking_lot::Mutex::new(0_u64);
    c.bench_function("parking_lot/mutex_try_lock_uncontended", |b| {
        b.iter(|| {
            let guard = try_lock.try_lock().expect("uncontended lock");
            black_box(*guard);
        });
    });
}

fn rwlock_uncontended(c: &mut Criterion) {
    let lock = deloxide::RwLock::new(0_u64);
    c.bench_function("rwlock/read_uncontended", |b| {
        b.iter(|| {
            let guard = lock.read();
            black_box(*guard);
        });
    });
    c.bench_function("rwlock/write_uncontended", |b| {
        b.iter(|| {
            let guard = lock.write();
            black_box(*guard);
        });
    });
}

fn mutex_handoff(c: &mut Criterion) {
    c.bench_function("mutex/two_thread_handoff", |b| {
        let lock = Arc::new(deloxide::Mutex::new(0_u64));
        b.iter(|| {
            std::thread::scope(|scope| {
                for _ in 0..2 {
                    let lock = Arc::clone(&lock);
                    scope.spawn(move || {
                        for _ in 0..32 {
                            let mut guard = lock.lock();
                            *guard = black_box(*guard + 1);
                        }
                    });
                }
            });
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(30)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(2));
    targets =
        mutex_uncontended,
        parking_lot_mutex_controls,
        rwlock_uncontended,
        mutex_handoff
);
criterion_main!(benches);

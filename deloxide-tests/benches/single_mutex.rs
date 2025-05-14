use criterion::{black_box, criterion_group, criterion_main, Criterion};
use deloxide_tests::*;

fn single_mutex_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_mutex");

    // Test single mutex acquisition/release
    group.bench_function("lock_unlock", |b| {
        let mutex = new_arc_mutex(0);
        b.iter(|| {
            let guard = mutex.lock();
            black_box(*guard);
            drop(guard);
        });
    });

    group.finish();
}

criterion_group!(benches, single_mutex_operations);
criterion_main!(benches);

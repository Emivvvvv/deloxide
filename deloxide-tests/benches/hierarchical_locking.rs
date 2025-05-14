use criterion::{black_box, criterion_group, criterion_main, Criterion};
use deloxide_tests::*;

fn hierarchical_locking(c: &mut Criterion) {
    let mut group = c.benchmark_group("hierarchical");

    // Threads acquire multiple locks following strict hierarchy
    for &lock_count in &[2, 4, 8] {
        for &thread_count in &[2, 4, 8] {
            group.bench_function(
                format!("locks_{}_threads_{}", lock_count, thread_count),
                |b| {
                    b.iter(|| {
                        let locks: Vec<_> = (0..lock_count)
                            .map(|i| new_arc_mutex(i))
                            .collect();
                        let mut handles = vec![];

                        for _ in 0..thread_count {
                            let locks = locks.clone();

                            let handle = spawn_thread(move || {
                                for _ in 0..50 {
                                    // Acquire locks in ascending order
                                    let guards: Vec<_> = locks.iter()
                                        .map(|lock| lock.lock())
                                        .collect();
                                    black_box(&guards);
                                    // All guards drop in reverse order
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.join().unwrap();
                        }
                    });
                }
            );
        }
    }

    group.finish();
}

criterion_group!(benches, hierarchical_locking);
criterion_main!(benches);

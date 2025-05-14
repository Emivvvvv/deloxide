use criterion::{black_box, criterion_group, criterion_main, Criterion};
use deloxide_tests::*;
use std::sync::Arc;

fn reader_writer_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("reader_writer");

    for &thread_count in &[4, 8, 16, 32] {
        for &write_ratio in &[1, 5, 10, 20] {
            group.bench_function(
                format!("threads_{}_write_{}pct", thread_count, write_ratio),
                |b| {
                    b.iter(|| {
                        let data = new_arc_mutex(0);
                        let mut handles = vec![];

                        for i in 0..thread_count {
                            let data = Arc::clone(&data);

                            let handle = spawn_thread(move || {
                                for j in 0..100 {
                                    if (i * 17 + j * 13) % 100 < write_ratio {
                                        // Writer
                                        let mut guard = data.lock();
                                        *guard += 1;
                                    } else {
                                        // Reader
                                        let guard = data.lock();
                                        black_box(*guard);
                                    }
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

criterion_group!(benches, reader_writer_pattern);
criterion_main!(benches);

use std::sync::Arc;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use deloxide_tests::*;

fn producer_consumer(c: &mut Criterion) {
    let mut group = c.benchmark_group("producer_consumer");

    for &producers in &[1, 2, 4] {
        for &consumers in &[1, 2, 4] {
            group.bench_function(
                format!("prod_{}_cons_{}", producers, consumers),
                |b| {
                    b.iter(|| {
                        let buffer = new_arc_mutex(Vec::new());
                        let mut handles = vec![];

                        // Producers
                        for _ in 0..producers {
                            let buffer = Arc::clone(&buffer);

                            let handle = spawn_thread(move || {
                                for i in 0..100 {
                                    let mut buf = buffer.lock();
                                    buf.push(i);
                                }
                            });
                            handles.push(handle);
                        }

                        // Consumers
                        for _ in 0..consumers {
                            let buffer = Arc::clone(&buffer);

                            let handle = spawn_thread(move || {
                                for _ in 0..100 {
                                    let mut buf = buffer.lock();
                                    if let Some(val) = buf.pop() {
                                        black_box(val);
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

criterion_group!(benches, producer_consumer);
criterion_main!(benches);

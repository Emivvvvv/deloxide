use deloxide::{Condvar, Mutex as DMutex, Thread};
use std::sync::Arc;
use std::time::Duration;
mod common;
use common::{DEADLOCK_TIMEOUT, expect_deadlock, start_detector};

#[test]
fn test_condvar_producer_consumer_deadlock() {
    let harness = start_detector();

    /* Shared state for producer-consumer scenario */
    let buffer_mutex = Arc::new(DMutex::new(Vec::<i32>::new())); // shared buffer
    let consumer_mutex = Arc::new(DMutex::new(())); // consumer resource
    let producer_cv = Arc::new(Condvar::new()); // producer waits for space

    /* Producer thread: produces data, waits for buffer space, needs consumer resource */
    {
        let buffer_mutex = Arc::clone(&buffer_mutex);
        let consumer_mutex = Arc::clone(&consumer_mutex);
        let producer_cv = Arc::clone(&producer_cv);

        Thread::spawn(move || {
            // Producer holds buffer mutex
            let mut buffer = buffer_mutex.lock();
            println!("Producer: Got buffer mutex");

            // Initialize buffer to be "full" to force waiting
            for i in 0..5 {
                buffer.push(i);
            }

            // Simulate buffer being full - wait for consumer to make space
            while buffer.len() >= 5 {
                println!("Producer: Buffer full, waiting for space...");
                producer_cv.wait(&mut buffer); // Releases buffer_mutex while waiting
            }
            // Buffer mutex is reacquired here
            println!("Producer: Woke up, buffer mutex reacquired");

            // Try to access consumer resource → DEADLOCK
            // Consumer holds consumer_mutex and is trying to get buffer_mutex
            println!("Producer: Trying to get consumer resource...");
            let _consumer_resource = consumer_mutex.lock();

            // This code is never reached
            buffer.push(42);
            println!("Producer: Added item to buffer");
        });
    }

    /* Consumer thread: holds consumer resource, signals producer, needs buffer */
    {
        let buffer_mutex = Arc::clone(&buffer_mutex);
        let consumer_mutex = Arc::clone(&consumer_mutex);
        let producer_cv = Arc::clone(&producer_cv);

        Thread::spawn(move || {
            // Small delay to let producer start waiting
            std::thread::sleep(Duration::from_millis(50));

            // Consumer holds its resource first
            let _consumer_resource = consumer_mutex.lock();
            println!("Consumer: Got consumer mutex");

            // Actually make space in the buffer so producer can proceed
            {
                let mut buffer = buffer_mutex.lock();
                if !buffer.is_empty() {
                    buffer.pop();
                    println!("Consumer: Removed item from buffer, space available");
                }
            }

            // Signal producer that space is available
            println!("Consumer: Signaling producer...");
            producer_cv.notify_one();

            // Small delay to let producer wake up and try to get consumer_mutex
            std::thread::sleep(Duration::from_millis(50));

            // Try to access buffer → DEADLOCK
            // Producer holds buffer_mutex and is trying to get consumer_mutex (which we hold)
            println!("Consumer: Trying to get buffer mutex...");
            let _buffer = buffer_mutex.lock();

            // This code is never reached
            println!("Consumer: Got buffer mutex");
        });
    }

    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);
    assert_eq!(info.thread_cycle.len(), 2);
    println!("✅ Producer-Consumer condvar deadlock test passed");
}

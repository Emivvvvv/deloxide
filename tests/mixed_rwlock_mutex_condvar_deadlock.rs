use deloxide::{Condvar, Mutex as DMutex, RwLock as DRwLock, Thread};
use std::sync::Arc;
use std::time::Duration;
mod common;
use common::{DEADLOCK_TIMEOUT, expect_deadlock, start_detector};

#[test]
fn test_mixed_rwlock_mutex_condvar_deadlock() {
    let harness = start_detector();

    /* Shared state - simulating a data processing system */
    let shared_data = Arc::new(DRwLock::new(vec![1, 2, 3, 4, 5])); // Data that can be read/written
    let processor_mutex = Arc::new(DMutex::new(String::from("idle"))); // Processing state
    let data_ready_cv = Arc::new(Condvar::new()); // Signals when data is ready

    /* Reader thread: reads data, waits for processing completion, then needs processor access */
    {
        let shared_data = Arc::clone(&shared_data);
        let processor_mutex = Arc::clone(&processor_mutex);
        let data_ready_cv = Arc::clone(&data_ready_cv);

        Thread::spawn(move || {
            // Reader gets read access to shared data
            let data_guard = shared_data.read();
            println!("Reader: Got read lock on data: {:?}", *data_guard);

            // Wait for data processing to be ready
            let mut processor_state = processor_mutex.lock();
            while *processor_state == "idle" {
                println!("Reader: Waiting for processor to be ready...");
                data_ready_cv.wait(&mut processor_state); // Releases processor_mutex while waiting
            }
            // processor_mutex is reacquired here, but we still hold the RwLock read guard

            println!("Reader: Processor is ready, now trying to access it...");
            // This creates the deadlock - we hold RwLock (read) and try to get processor_mutex
            // But the writer thread holds processor_mutex and is trying to get RwLock (write)
            // The reader already has processor_mutex from the wait, but let's simulate
            // needing it again for a different operation
            drop(processor_state); // Release the mutex from wait

            // Now try to get it again for "final processing"
            let _final_processor_access = processor_mutex.lock();

            println!("Reader: Got final processor access");
            // This code is never reached due to deadlock
        });
    }

    /* Writer thread: manages processing state, signals readiness, then needs data write access */
    {
        let shared_data = Arc::clone(&shared_data);
        let processor_mutex = Arc::clone(&processor_mutex);
        let data_ready_cv = Arc::clone(&data_ready_cv);

        Thread::spawn(move || {
            // Small delay to let reader get the read lock first
            std::thread::sleep(Duration::from_millis(10));

            // Writer takes control of processor
            let mut processor_state = processor_mutex.lock();
            *processor_state = String::from("processing");
            println!("Writer: Set processor to 'processing' state");

            // Signal that data processing is ready
            data_ready_cv.notify_one();
            println!("Writer: Notified reader that processing is ready");

            // Small delay to let reader wake up and try to get processor_mutex again
            std::thread::sleep(Duration::from_millis(20));

            // Now try to write to shared data → DEADLOCK
            // Reader holds RwLock (read) and is trying to get processor_mutex (which we hold)
            // We hold processor_mutex and are trying to get RwLock (write) - blocked by reader
            println!("Writer: Trying to get write access to data...");
            let _data_write_guard = shared_data.write();

            println!("Writer: Got write access to data");
            // This code is never reached due to deadlock
        });
    }

    let info = expect_deadlock(&harness, DEADLOCK_TIMEOUT);
    assert_eq!(info.thread_cycle.len(), 2);

    println!("✅ Mixed RwLock+Mutex+Condvar deadlock test passed");
    println!("   Thread cycle: {:?}", info.thread_cycle);
    println!("   Waiting for locks: {:?}", info.thread_waiting_for_locks);
}

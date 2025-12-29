#[cfg(feature = "logging-and-visualization")]
use deloxide::showcase_this;
use deloxide::{DeadlockInfo, Deloxide};
use std::sync::{Arc, Mutex as StdMutex, mpsc};
use std::time::Duration;

#[allow(dead_code)]
pub const DEADLOCK_TIMEOUT: Duration = Duration::from_secs(3);
#[allow(dead_code)]
pub const NO_DEADLOCK_TIMEOUT: Duration = Duration::from_millis(500);

pub struct DetectorHarness {
    pub rx: mpsc::Receiver<DeadlockInfo>,
    pub detected: Arc<StdMutex<bool>>,
}

pub fn start_detector() -> DetectorHarness {
    let (tx, rx) = mpsc::channel::<DeadlockInfo>();
    let detected = Arc::new(StdMutex::new(false));
    let flag = Arc::clone(&detected);

    let builder = Deloxide::new().callback(move |info| {
        #[cfg(feature = "logging-and-visualization")]
        {
            let _ = showcase_this();
        }
        *flag.lock().unwrap() = true;
        let _ = tx.send(info);
    });

    #[cfg(feature = "logging-and-visualization")]
    let builder = builder.with_log("logs/deloxide_{timestamp}.log");

    builder.start().expect("Failed to initialize detector");

    DetectorHarness { rx, detected }
}

#[allow(dead_code)]
pub fn expect_deadlock(h: &DetectorHarness, timeout: Duration) -> DeadlockInfo {
    match h.rx.recv_timeout(timeout) {
        Ok(info) => {
            assert!(*h.detected.lock().unwrap(), "Deadlock flag should be set");
            info
        }
        Err(_) => panic!("No deadlock detected within {timeout:?}"),
    }
}

#[allow(dead_code)]
pub fn assert_no_deadlock(h: &DetectorHarness, timeout: Duration) {
    assert!(
        h.rx.recv_timeout(timeout).is_err(),
        "Unexpected deadlock detected"
    );
    assert!(
        !*h.detected.lock().unwrap(),
        "Deadlock flag should not be set"
    );
}

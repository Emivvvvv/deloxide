#[cfg(feature = "stress-test")]
use crate::core::stress::calculate_stress_delay;

#[cfg(feature = "stress-test")]
impl crate::core::Detector {
    pub fn calculate_stress_delay(
        &self,
        thread_id: crate::core::ThreadId,
        lock_id: crate::core::LockId,
    ) -> Option<std::time::Duration> {
        if self.stress_mode != crate::core::stress::StressMode::None {
            if let Some(config) = &self.stress_config {
                let held_locks = self
                    .thread_holds
                    .get(&thread_id)
                    .map(|set| set.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default();

                calculate_stress_delay(self.stress_mode, thread_id, lock_id, &held_locks, config)
                    .map(std::time::Duration::from_micros)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn stress_on_lock_release(
        &self,
        _thread_id: crate::core::ThreadId,
        _lock_id: crate::core::LockId,
    ) {
        if self.stress_mode != crate::core::stress::StressMode::None {
            if let Some(config) = &self.stress_config {
                if config.preempt_after_release {
                    std::thread::yield_now();
                }
            }
        }
    }
}

#[cfg(feature = "stress-test")]
use crate::core::stress::{on_lock_attempt, on_lock_release};

#[cfg(feature = "stress-test")]
impl crate::core::Detector {
    pub fn stress_on_lock_attempt(
        &self,
        thread_id: crate::core::ThreadId,
        lock_id: crate::core::LockId,
    ) {
        if self.stress_mode != crate::core::stress::StressMode::None
            && let Some(config) = &self.stress_config
        {
            let held_locks = self
                .thread_holds
                .get(&thread_id)
                .map(|set| set.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default();

            on_lock_attempt(self.stress_mode, thread_id, lock_id, &held_locks, config);
        }
    }

    pub fn stress_on_lock_release(
        &self,
        thread_id: crate::core::ThreadId,
        lock_id: crate::core::LockId,
    ) {
        if self.stress_mode != crate::core::stress::StressMode::None
            && let Some(config) = &self.stress_config
        {
            on_lock_release(self.stress_mode, thread_id, lock_id, config);
        }
    }
}

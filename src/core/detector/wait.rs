use crate::core::Detector;
use crate::{LockId, ThreadId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WaitMode {
    Mutex,
    RwRead,
    RwWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WaitIntent {
    pub(crate) lock_id: LockId,
    pub(crate) mode: WaitMode,
}

impl WaitIntent {
    pub(crate) const fn new(lock_id: LockId, mode: WaitMode) -> Self {
        Self { lock_id, mode }
    }
}

impl Detector {
    pub(crate) fn set_wait_intent(&mut self, thread_id: ThreadId, intent: WaitIntent) {
        self.clear_wait_intent(thread_id);
        self.thread_waits_for.insert(thread_id, intent);
        self.lock_waiters
            .entry(intent.lock_id)
            .or_default()
            .insert(thread_id);
    }

    pub(crate) fn clear_wait_intent(&mut self, thread_id: ThreadId) {
        let Some(intent) = self.thread_waits_for.remove(&thread_id) else {
            self.wait_for_graph.clear_wait_edges(thread_id);
            return;
        };

        if let Some(waiters) = self.lock_waiters.get_mut(&intent.lock_id) {
            waiters.remove(&thread_id);
            if waiters.is_empty() {
                self.lock_waiters.remove(&intent.lock_id);
            }
        }
        self.wait_for_graph.clear_wait_edges(thread_id);
    }

    pub(crate) fn incompatible_owners(
        &self,
        _waiting_thread: ThreadId,
        intent: WaitIntent,
    ) -> Vec<ThreadId> {
        match intent.mode {
            WaitMode::Mutex => self
                .mutex_owners
                .get(&intent.lock_id)
                .copied()
                .into_iter()
                .collect(),
            WaitMode::RwRead => self
                .rwlock_writer
                .get(&intent.lock_id)
                .copied()
                .into_iter()
                .collect(),
            WaitMode::RwWrite => {
                let mut owners: Vec<_> = self
                    .rwlock_readers
                    .get(&intent.lock_id)
                    .into_iter()
                    .flat_map(|readers| readers.keys().copied())
                    .collect();
                if let Some(writer) = self.rwlock_writer.get(&intent.lock_id).copied()
                    && !owners.contains(&writer)
                {
                    owners.push(writer);
                }
                owners
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutex_wait_resolves_current_owner() {
        let mut detector = Detector::new();
        detector.mutex_owners.insert(10, 2);

        let intent = WaitIntent::new(10, WaitMode::Mutex);

        assert_eq!(detector.incompatible_owners(1, intent), vec![2]);
    }

    #[test]
    fn read_wait_resolves_only_writer() {
        let mut detector = Detector::new();
        detector
            .rwlock_readers
            .entry(10)
            .or_default()
            .insert(3, 1);
        detector.rwlock_writer.insert(10, 2);

        let intent = WaitIntent::new(10, WaitMode::RwRead);

        assert_eq!(detector.incompatible_owners(1, intent), vec![2]);
    }

    #[test]
    fn write_wait_resolves_all_readers_including_self() {
        let mut detector = Detector::new();
        detector
            .rwlock_readers
            .entry(10)
            .or_default()
            .insert(1, 1);
        detector
            .rwlock_readers
            .entry(10)
            .or_default()
            .insert(3, 2);

        let intent = WaitIntent::new(10, WaitMode::RwWrite);
        let mut owners = detector.incompatible_owners(1, intent);
        owners.sort_unstable();

        assert_eq!(owners, vec![1, 3]);
    }

    #[test]
    fn wait_intent_updates_forward_and_reverse_indexes() {
        let mut detector = Detector::new();
        let intent = WaitIntent::new(10, WaitMode::Mutex);

        detector.set_wait_intent(1, intent);

        assert_eq!(detector.thread_waits_for.get(&1), Some(&intent));
        assert!(detector.lock_waiters.get(&10).unwrap().contains(&1));

        detector.clear_wait_intent(1);

        assert!(!detector.thread_waits_for.contains_key(&1));
        assert!(!detector.lock_waiters.contains_key(&10));
    }
}

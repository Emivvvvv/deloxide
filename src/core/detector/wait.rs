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

    pub(crate) fn register_wait(
        &mut self,
        thread_id: ThreadId,
        intent: WaitIntent,
    ) -> Option<Vec<ThreadId>> {
        self.set_wait_intent(thread_id, intent);
        let owners = self.incompatible_owners(thread_id, intent);
        let mut detected_cycle = None;
        for owner in owners {
            if let Some(cycle) = self.wait_for_graph.add_edge(thread_id, owner)
                && detected_cycle.is_none()
            {
                detected_cycle = Some(cycle);
            }
        }
        detected_cycle
    }

    pub(crate) fn refresh_waiters_for_lock(
        &mut self,
        lock_id: LockId,
    ) -> Option<Vec<ThreadId>> {
        let waiters: Vec<_> = self
            .lock_waiters
            .get(&lock_id)
            .into_iter()
            .flat_map(|waiters| waiters.iter().copied())
            .collect();

        let mut detected_cycle = None;
        for waiter in waiters {
            let Some(intent) = self.thread_waits_for.get(&waiter).copied() else {
                continue;
            };
            self.wait_for_graph.clear_wait_edges(waiter);
            for owner in self.incompatible_owners(waiter, intent) {
                if let Some(cycle) = self.wait_for_graph.add_edge(waiter, owner)
                    && detected_cycle.is_none()
                {
                    detected_cycle = Some(cycle);
                }
            }
        }
        detected_cycle
    }

    pub(crate) fn validate_wait_cycle(&self, cycle: &[ThreadId]) -> bool {
        !cycle.is_empty()
            && cycle.iter().enumerate().all(|(index, source)| {
                let target = cycle[(index + 1) % cycle.len()];
                self.thread_waits_for
                    .get(source)
                    .map(|intent| {
                        self.incompatible_owners(*source, *intent)
                            .contains(&target)
                    })
                    .unwrap_or(false)
            })
    }

    pub(crate) fn validated_deadlock_info(
        &mut self,
        cycle: Vec<ThreadId>,
    ) -> Option<crate::DeadlockInfo> {
        if self.validate_wait_cycle(&cycle) {
            return Some(self.extract_deadlock_info(cycle));
        }

        let mut locks: Vec<_> = cycle
            .iter()
            .filter_map(|thread| {
                self.thread_waits_for
                    .get(thread)
                    .map(|intent| intent.lock_id)
            })
            .collect();
        locks.sort_unstable();
        locks.dedup();
        let mut refreshed_cycle = None;
        for lock_id in locks {
            if let Some(cycle) = self.refresh_waiters_for_lock(lock_id)
                && refreshed_cycle.is_none()
            {
                refreshed_cycle = Some(cycle);
            }
        }
        refreshed_cycle.and_then(|cycle| {
            self.validate_wait_cycle(&cycle)
                .then(|| self.extract_deadlock_info(cycle))
        })
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

    #[test]
    fn refresh_retargets_waiter_after_owner_handoff() {
        let mut detector = Detector::new();
        detector.mutex_owners.insert(10, 2);
        detector.register_wait(1, WaitIntent::new(10, WaitMode::Mutex));
        assert_eq!(detector.wait_for_graph.outgoing(1), vec![2]);

        detector.mutex_owners.insert(10, 3);
        detector.refresh_waiters_for_lock(10);

        assert_eq!(detector.wait_for_graph.outgoing(1), vec![3]);
    }

    #[test]
    fn complete_cycle_validation_rejects_stale_older_edge() {
        let mut detector = Detector::new();
        detector.mutex_owners.insert(10, 2);
        detector.mutex_owners.insert(20, 1);
        detector.register_wait(1, WaitIntent::new(10, WaitMode::Mutex));
        detector.register_wait(2, WaitIntent::new(20, WaitMode::Mutex));

        assert!(detector.validate_wait_cycle(&[1, 2]));

        detector.mutex_owners.insert(10, 3);

        assert!(!detector.validate_wait_cycle(&[1, 2]));
    }

    #[test]
    fn stale_cycle_is_not_converted_to_deadlock_report() {
        let mut detector = Detector::new();
        detector.mutex_owners.insert(10, 2);
        detector.mutex_owners.insert(20, 1);
        detector.register_wait(1, WaitIntent::new(10, WaitMode::Mutex));
        let cycle = detector
            .register_wait(2, WaitIntent::new(20, WaitMode::Mutex))
            .expect("cycle");

        detector.mutex_owners.insert(10, 3);

        assert!(detector.validated_deadlock_info(cycle).is_none());
    }
    #[test]
    fn register_wait_records_every_incompatible_owner_after_finding_cycle() {
        let mut detector = Detector::new();
        for owner in [2, 3, 4] {
            detector
                .rwlock_readers
                .entry(10)
                .or_default()
                .insert(owner, 1);
        }
        let intent = WaitIntent::new(10, WaitMode::RwWrite);
        let cycle_owner = detector.incompatible_owners(1, intent)[0];
        let dependency_lock = 20 + cycle_owner;
        detector.mutex_owners.insert(dependency_lock, 1);
        detector.register_wait(
            cycle_owner,
            WaitIntent::new(dependency_lock, WaitMode::Mutex),
        );

        assert!(detector.register_wait(1, intent).is_some());

        let mut expected = vec![2, 3, 4];
        expected.retain(|owner| *owner != cycle_owner);
        assert_eq!(detector.wait_for_graph.outgoing(1), expected);
    }

    #[test]
    fn refresh_updates_every_waiter_after_finding_cycle() {
        let mut detector = Detector::new();
        detector.mutex_owners.insert(10, 2);
        for waiter in [1, 3, 4] {
            detector.set_wait_intent(waiter, WaitIntent::new(10, WaitMode::Mutex));
            detector.wait_for_graph.add_edge(waiter, 99);
        }

        let cycle_waiter = *detector.lock_waiters[&10].iter().next().unwrap();
        detector.mutex_owners.insert(20, cycle_waiter);
        detector.register_wait(2, WaitIntent::new(20, WaitMode::Mutex));

        assert!(detector.refresh_waiters_for_lock(10).is_some());

        for waiter in [1, 3, 4] {
            let expected = if waiter == cycle_waiter {
                Vec::new()
            } else {
                vec![2]
            };
            assert_eq!(detector.wait_for_graph.outgoing(waiter), expected);
        }
    }

    #[test]
    fn stale_candidate_returns_valid_cycle_found_during_refresh() {
        let mut detector = Detector::new();
        detector.mutex_owners.insert(10, 2);
        detector.mutex_owners.insert(20, 1);
        detector.set_wait_intent(1, WaitIntent::new(10, WaitMode::Mutex));
        detector.set_wait_intent(2, WaitIntent::new(20, WaitMode::Mutex));
        detector.wait_for_graph.add_edge(1, 3);
        detector.wait_for_graph.add_edge(2, 1);

        let info = detector
            .validated_deadlock_info(vec![1, 3])
            .expect("refresh should recover the current cycle");

        let mut cycle = info.thread_cycle;
        cycle.sort_unstable();
        assert_eq!(cycle, vec![1, 2]);
    }
}

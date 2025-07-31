// src/core/stress.rs
// This module provides stress testing functionality for Deloxide
// It's only compiled when the "stress-test" feature is enabled

use crate::core::types::{LockId, ThreadId};
use fxhash::FxHashMap;
use parking_lot::Mutex;
use std::thread;
use std::time::Duration;

#[cfg(feature = "stress-test")]
use rand::{Rng, rng};

/// Stress testing modes available in Deloxide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StressMode {
    /// No stress testing (default)
    #[default]
    None,
    /// Random preemption at lock acquisition points
    RandomPreemption,
    /// Component-based delays using lock acquisition patterns
    ComponentBased,
}

/// Configuration options for stress testing
#[derive(Debug, Clone)]
pub struct StressConfig {
    /// Probability of preemption (0.0-1.0)
    pub preemption_probability: f64,
    /// Minimum delay in milliseconds
    pub min_delay_ms: u64,
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    /// Whether to preempt after lock releases
    pub preempt_after_release: bool,
}

impl Default for StressConfig {
    fn default() -> Self {
        StressConfig {
            preemption_probability: 0.5,
            min_delay_ms: 1,
            max_delay_ms: 10,
            preempt_after_release: true,
        }
    }
}

/// State for tracking lock relationships
#[derive(Default)]
struct ComponentTracker {
    /// Maps locks to their component IDs
    components: FxHashMap<LockId, usize>,
    /// Records lock acquisition order
    acquisitions: Vec<(LockId, LockId)>,
}

impl ComponentTracker {
    fn new() -> Self {
        Default::default()
    }

    fn record_acquisition(&mut self, from_lock: LockId, to_lock: LockId) {
        self.acquisitions.push((from_lock, to_lock));

        // Assign components (simple approach)
        if !self.components.contains_key(&from_lock) {
            let comp_id = self.components.len();
            self.components.insert(from_lock, comp_id);
        }

        if !self.components.contains_key(&to_lock) {
            let from_comp = *self.components.get(&from_lock).unwrap();
            self.components.insert(to_lock, from_comp);
        }
    }

    fn should_delay(&self, from_lock: LockId, to_lock: LockId) -> bool {
        let from_comp = self
            .components
            .get(&from_lock)
            .copied()
            .unwrap_or(usize::MAX);
        let to_comp = self.components.get(&to_lock).copied().unwrap_or(usize::MAX);

        // If both locks are in the same component (potential cycle)
        if from_comp == to_comp && from_comp != usize::MAX {
            return true;
        }

        // If there's a reverse acquisition pattern
        self.acquisitions
            .iter()
            .any(|&(f, t)| f == to_lock && t == from_lock)
    }
}

/// Global state for stress testing
struct StressState {
    /// Track lock relationships
    tracker: ComponentTracker,
    /// Count preemptions per lock
    preemption_counts: FxHashMap<LockId, usize>,
}

impl StressState {
    fn new() -> Self {
        StressState {
            tracker: ComponentTracker::new(),
            preemption_counts: FxHashMap::default(),
        }
    }

    fn record_acquisition(&mut self, from_lock: LockId, to_lock: LockId) {
        self.tracker.record_acquisition(from_lock, to_lock);
    }

    fn should_delay_component(&self, from_lock: LockId, to_lock: LockId) -> bool {
        self.tracker.should_delay(from_lock, to_lock)
    }

    fn track_preemption(&mut self, lock_id: LockId) {
        *self.preemption_counts.entry(lock_id).or_insert(0) += 1;
    }
}

lazy_static::lazy_static! {
    static ref STRESS_STATE: Mutex<StressState> = Mutex::new(StressState::new());
}

/// Apply a delay to the current thread
#[cfg(feature = "stress-test")]
pub fn apply_delay(min_ms: u64, max_ms: u64) {
    let mut rng = rng();
    let delay_ms = if min_ms == max_ms {
        min_ms
    } else {
        rng.random_range(min_ms..=max_ms)
    };
    thread::sleep(Duration::from_millis(delay_ms));
}

#[cfg(not(feature = "stress-test"))]
pub fn apply_delay(_min_ms: u64, _max_ms: u64) {
    // No-op when stress testing is disabled
}

/// Perform a random preemption if probability check passes
#[cfg(feature = "stress-test")]
#[allow(unused_variables)]
pub fn try_random_preemption(
    thread_id: ThreadId,
    lock_id: LockId,
    probability: f64,
    min_delay_ms: u64,
    max_delay_ms: u64,
) -> bool {
    let mut rng = rng();

    if rng.random::<f64>() < probability {
        // Track this preemption
        let mut state = STRESS_STATE.lock();
        state.track_preemption(lock_id);
        drop(state); // Release lock before sleeping

        // Apply delay
        apply_delay(min_delay_ms, max_delay_ms);
        true
    } else {
        false
    }
}

#[cfg(not(feature = "stress-test"))]
pub fn try_random_preemption(
    _thread_id: ThreadId,
    _lock_id: LockId,
    _probability: f64,
    _min_delay_ms: u64,
    _max_delay_ms: u64,
) -> bool {
    false
}

/// Apply component-based delay strategy
#[allow(unused_variables)]
pub fn apply_component_delay(
    thread_id: ThreadId,
    lock_id: LockId,
    held_locks: &[LockId],
    min_delay_ms: u64,
    max_delay_ms: u64,
) -> bool {
    if held_locks.is_empty() {
        return false;
    }

    let mut should_delay = false;

    // Check relationships with held locks
    let mut state = STRESS_STATE.lock();
    for &held_lock in held_locks {
        // Record the acquisition pattern for future analysis
        state.record_acquisition(held_lock, lock_id);

        // Check if this acquisition pattern should be delayed
        if state.should_delay_component(held_lock, lock_id) {
            should_delay = true;
            break;
        }
    }

    if should_delay {
        state.track_preemption(lock_id);
        drop(state); // Release lock before sleeping

        // Apply delay
        apply_delay(min_delay_ms, max_delay_ms);
        true
    } else {
        false
    }
}

/// Apply stress testing before lock acquisition
pub fn on_lock_attempt(
    mode: StressMode,
    thread_id: ThreadId,
    lock_id: LockId,
    held_locks: &[LockId],
    config: &StressConfig,
) -> bool {
    match mode {
        StressMode::None => false,

        StressMode::RandomPreemption => try_random_preemption(
            thread_id,
            lock_id,
            config.preemption_probability,
            config.min_delay_ms,
            config.max_delay_ms,
        ),

        StressMode::ComponentBased => apply_component_delay(
            thread_id,
            lock_id,
            held_locks,
            config.min_delay_ms,
            config.max_delay_ms,
        ),
    }
}

/// Apply stress testing after lock release
pub fn on_lock_release(
    mode: StressMode,
    _thread_id: ThreadId,
    _lock_id: LockId,
    config: &StressConfig,
) {
    if mode != StressMode::None && config.preempt_after_release {
        thread::yield_now();
    }
}

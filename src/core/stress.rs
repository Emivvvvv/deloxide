// src/core/stress.rs
// This module provides stress testing functionality for Deloxide
// It's only compiled when the "stress-test" feature is enabled
#![allow(dead_code)]

use crate::core::types::{LockId, ThreadId};
use fxhash::FxHashMap;
use parking_lot::Mutex;
use std::thread;
use std::time::Duration;



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
    /// Minimum delay in microseconds
    pub min_delay_us: u64,
    /// Maximum delay in microseconds
    pub max_delay_us: u64,
    /// Whether to preempt after lock releases
    pub preempt_after_release: bool,
}

impl Default for StressConfig {
    fn default() -> Self {
        StressConfig {
            preemption_probability: 0.5,
            min_delay_us: 250,  // 250us
            max_delay_us: 2000, // 2ms
            preempt_after_release: true,
        }
    }
}

impl StressConfig {
    /// Configuration with higher probability
    pub fn high_probability() -> Self {
        Self {
            preemption_probability: 0.8,
            ..Default::default()
        }
    }

    /// Configuration with lower probability
    pub fn low_probability() -> Self {
        Self {
            preemption_probability: 0.2,
            ..Default::default()
        }
    }

    /// Aggressive configuration (high delay and high probability)
    pub fn aggressive() -> Self {
        Self {
            preemption_probability: 0.8,
            min_delay_us: 500,
            max_delay_us: 5000,
            preempt_after_release: true,
        }
    }

    /// Gentle configuration (low delay and low probability)
    pub fn gentle() -> Self {
        Self {
            preemption_probability: 0.2,
            min_delay_us: 20,
            max_delay_us: 100,
            preempt_after_release: false,
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
pub fn apply_delay(min_us: u64, max_us: u64) {
    let mut rng = rng();
    let delay_us = if min_us == max_us {
        min_us
    } else {
        rng.random_range(min_us..=max_us)
    };
    thread::sleep(Duration::from_micros(delay_us));
}

/// Perform a random preemption if probability check passes
#[allow(unused_variables)]
pub fn try_random_preemption(thread_id: ThreadId, lock_id: LockId, held_locks: &[LockId], config: &StressConfig) -> Option<u64> {
    // Only apply random preemption if the thread already holds locks.
    // This prevents "random backoff" which can desynchronize threads and prevent deadlocks.
    if held_locks.is_empty() {
        return None;
    }

    let prob_int = (config.preemption_probability * 1_000_000.0) as u64;
    if prob_int == 0 {
        return None;
    }

    let mut rng = rng();

    if rng.random_range(0..1_000_000) < prob_int {
        // Track this preemption
        let mut state = STRESS_STATE.lock();
        state.track_preemption(lock_id);
        drop(state); // Release lock before returning

        // Calculate delay
        let min_us = config.min_delay_us;
        let max_us = config.max_delay_us;
        
        let delay_us = if min_us == max_us {
            min_us
        } else {
            rng.random_range(min_us..=max_us)
        };
        Some(delay_us)
    } else {
        None
    }
}

/// Apply component-based delay strategy
#[allow(unused_variables)]
pub fn apply_component_delay(thread_id: ThreadId, lock_id: LockId, held_locks: &[LockId], config: &StressConfig) -> Option<u64> {
    if held_locks.is_empty() {
        return None;
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
        drop(state); // Release lock before returning

        // Calculate delay
        let min_us = config.min_delay_us;
        let max_us = config.max_delay_us;
        
        let mut rng = rng();
        let delay_us = if min_us == max_us {
            min_us
        } else {
            rng.random_range(min_us..=max_us)
        };
        Some(delay_us)
    } else {
        None
    }
}

/// Apply stress testing before lock acquisition
pub fn calculate_stress_delay(
    mode: StressMode,
    thread_id: ThreadId,
    lock_id: LockId,
    held_locks: &[LockId],
    config: &StressConfig,
) -> Option<u64> {
    match mode {
        StressMode::None => None,

        StressMode::RandomPreemption => try_random_preemption(thread_id, lock_id, held_locks, config),

        StressMode::ComponentBased => apply_component_delay(thread_id, lock_id, held_locks, config),
    }
}

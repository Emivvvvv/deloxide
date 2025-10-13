//! Lock Order Graph for detecting lock ordering violations
//!
//! This module implements a lock order graph that tracks the order in which locks
//! are acquired across all threads. It detects potential deadlocks by identifying
//! lock ordering violations, even when threads don't actually block.
//!
//! # How it works
//!
//! When a thread holds lock A and then acquires lock B, we record that A < B.
//! If later we see an attempt to acquire A while holding B (B < A), this creates
//! a cycle in the lock order and indicates a potential deadlock.

use crate::core::types::LockId;
use fxhash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

/// Cache entry for cycle detection results
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Generation when this entry was created
    generation: u64,
    /// The cycle found, or None if no cycle
    result: Option<Vec<LockId>>,
}

/// Represents a directed edge in the lock order graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LockOrderEdge {
    /// Lock that must be acquired before `after`
    pub before: LockId,
    /// Lock that must be acquired after `before`
    pub after: LockId,
}

/// Lock order graph that tracks lock acquisition ordering
///
/// The graph maintains edges representing the order in which locks are acquired.
/// An edge from lock A to lock B means that A has been acquired before B in some
/// execution path.
#[derive(Debug)]
pub struct LockOrderGraph {
    /// Adjacency list: maps each lock to the set of locks that come after it
    /// If there's an edge A -> B, then A must be acquired before B
    edges: FxHashMap<LockId, FxHashSet<LockId>>,

    /// Reverse adjacency list: maps each lock to the set of locks that come before it
    /// Used for efficient cycle detection
    reverse_edges: FxHashMap<LockId, FxHashSet<LockId>>,

    /// All recorded edges for debugging and reporting
    all_edges: FxHashSet<LockOrderEdge>,

    /// Optimization 2: Cache for cycle detection results
    /// Key: (before, after), Value: cached result with generation
    cycle_cache: FxHashMap<(LockId, LockId), CacheEntry>,

    /// Generation counter, incremented on each edge addition
    /// Used to invalidate stale cache entries
    generation: u64,
}

impl LockOrderGraph {
    /// Create a new empty lock order graph
    pub fn new() -> Self {
        LockOrderGraph {
            edges: FxHashMap::default(),
            reverse_edges: FxHashMap::default(),
            all_edges: FxHashSet::default(),
            cycle_cache: FxHashMap::default(),
            generation: 0,
        }
    }

    /// Add an edge to the lock order graph indicating that `before` must be acquired before `after`
    ///
    /// # Arguments
    /// * `before` - Lock that was acquired first
    /// * `after` - Lock that was acquired second
    ///
    /// # Returns
    /// `Some(Vec<LockId>)` containing a cycle if adding this edge creates a lock order violation,
    /// `None` if the edge is valid and doesn't create a cycle
    pub fn add_edge(&mut self, before: LockId, after: LockId) -> Option<Vec<LockId>> {
        // Don't add self-edges
        if before == after {
            return None;
        }

        // Optimization 2: Check cache first
        let cache_key = (before, after);
        if let Some(cached) = self.cycle_cache.get(&cache_key) {
            // Cache hit! Check if still valid (same generation means no new edges since)
            if cached.generation == self.generation {
                return cached.result.clone();
            }
        }

        // Check if adding this edge would create a cycle (i.e., if there's already a path from `after` to `before`)
        let cycle_result = if let Some(cycle) = self.find_path(after, before) {
            // Found a cycle: there's already a path after -> ... -> before
            // Adding before -> after would complete the cycle
            let mut full_cycle = cycle;
            full_cycle.push(after); // Close the cycle
            Some(full_cycle)
        } else {
            None
        };

        // Cache the result before modifying the graph
        self.cycle_cache.insert(
            cache_key,
            CacheEntry {
                generation: self.generation,
                result: cycle_result.clone(),
            },
        );

        // If no cycle, record the edge
        if cycle_result.is_none() {
            let edge = LockOrderEdge { before, after };
            if self.all_edges.insert(edge) {
                // This is a new edge, add it to the adjacency lists
                self.edges.entry(before).or_default().insert(after);
                self.reverse_edges.entry(after).or_default().insert(before);

                // Increment generation to invalidate cache entries
                // (they're based on the old graph state)
                self.generation = self.generation.wrapping_add(1);

                // Optimization 3: Incremental cache invalidation
                // Only invalidate cache entries that might be affected
                // For now, we invalidate all (future: only invalidate paths through new edge)
                if self.cycle_cache.len() > 1000 {
                    // Clear cache if it gets too large
                    self.cycle_cache.clear();
                }
            }
        }

        cycle_result
    }

    /// Find a path from `start` to `end` in the lock order graph using BFS
    ///
    /// Optimization 3: Early termination and edge existence check
    /// - Returns immediately if start has no outgoing edges
    /// - Stops as soon as end is found (no need to explore further)
    ///
    /// # Arguments
    /// * `start` - Starting lock
    /// * `end` - Target lock
    ///
    /// # Returns
    /// `Some(Vec<LockId>)` containing the path from start to end if one exists,
    /// `None` if no path exists
    fn find_path(&self, start: LockId, end: LockId) -> Option<Vec<LockId>> {
        if start == end {
            return Some(vec![start]);
        }

        // Optimization 3a: Early termination - check if there are any edges from start
        if !self.edges.contains_key(&start) {
            return None;
        }

        // Standard BFS with early termination
        let mut queue = VecDeque::new();
        let mut visited = FxHashSet::default();
        let mut parent: FxHashMap<LockId, LockId> = FxHashMap::default();

        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            if let Some(neighbors) = self.edges.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        parent.insert(neighbor, current);

                        // Optimization 3b: Early termination - found the target
                        if neighbor == end {
                            // Reconstruct path immediately
                            let mut path = vec![end];
                            let mut node = end;
                            while let Some(&prev) = parent.get(&node) {
                                path.push(prev);
                                node = prev;
                            }
                            path.reverse();
                            return Some(path);
                        }

                        queue.push_back(neighbor);
                    }
                }
            }
        }

        None
    }

    /// Remove all edges involving a specific lock
    ///
    /// This is called when a lock is destroyed.
    ///
    /// # Arguments
    /// * `lock_id` - ID of the lock to remove
    pub fn remove_lock(&mut self, lock_id: LockId) {
        // Remove outgoing edges
        if let Some(successors) = self.edges.remove(&lock_id) {
            for successor in successors {
                if let Some(preds) = self.reverse_edges.get_mut(&successor) {
                    preds.remove(&lock_id);
                }
                self.all_edges.remove(&LockOrderEdge {
                    before: lock_id,
                    after: successor,
                });
            }
        }

        // Remove incoming edges
        if let Some(predecessors) = self.reverse_edges.remove(&lock_id) {
            for predecessor in predecessors {
                if let Some(succs) = self.edges.get_mut(&predecessor) {
                    succs.remove(&lock_id);
                }
                self.all_edges.remove(&LockOrderEdge {
                    before: predecessor,
                    after: lock_id,
                });
            }
        }
    }

    /// Get all edges in the graph
    ///
    /// # Returns
    /// Reference to the set of all edges
    #[allow(dead_code)]
    pub fn get_all_edges(&self) -> &FxHashSet<LockOrderEdge> {
        &self.all_edges
    }

    /// Check if there's an edge from `before` to `after`
    ///
    /// # Arguments
    /// * `before` - Starting lock
    /// * `after` - Ending lock
    ///
    /// # Returns
    /// `true` if there's a direct edge from `before` to `after`, `false` otherwise
    #[allow(dead_code)]
    pub fn has_edge(&self, before: LockId, after: LockId) -> bool {
        self.edges
            .get(&before)
            .map(|succs| succs.contains(&after))
            .unwrap_or(false)
    }

    /// Clear all edges from the graph
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.edges.clear();
        self.reverse_edges.clear();
        self.all_edges.clear();
        self.cycle_cache.clear();
        self.generation = 0;
    }
}

impl Default for LockOrderGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_cycle_detection() {
        let mut graph = LockOrderGraph::new();

        // Create A -> B -> A cycle
        assert!(graph.add_edge(1, 2).is_none()); // A -> B, no cycle yet

        // This should detect cycle: A -> B, then B -> A completes cycle
        if let Some(cycle) = graph.add_edge(2, 1) {
            assert!(cycle.len() >= 2); // Should have at least the cycle nodes
            assert!(cycle.contains(&1)); // Should contain lock 1
            assert!(cycle.contains(&2)); // Should contain lock 2
        } else {
            panic!("Should have detected cycle");
        }
    }

    #[test]
    fn test_direct_cycle() {
        let mut graph = LockOrderGraph::new();

        // Direct cycle: 1 -> 2, then 2 -> 1
        assert!(graph.add_edge(1, 2).is_none());
        assert!(graph.add_edge(2, 1).is_some());
    }

    #[test]
    fn test_no_false_cycles() {
        let mut graph = LockOrderGraph::new();

        // Linear chain: A -> B -> C (no cycle)
        assert!(graph.add_edge(1, 2).is_none());
        assert!(graph.add_edge(2, 3).is_none());
        assert!(graph.add_edge(1, 3).is_none()); // Redundant but valid
    }

    #[test]
    fn test_cache_behavior() {
        let mut graph = LockOrderGraph::new();

        // First check should miss cache and do BFS
        assert!(graph.add_edge(1, 2).is_none());

        // Same check should hit cache
        assert!(graph.add_edge(1, 2).is_none());

        // Adding new edge should invalidate cache
        assert!(graph.add_edge(3, 4).is_none());

        // Cache should work for new queries
        assert!(graph.add_edge(3, 4).is_none());
    }
}

//! Wait-For Graph for detecting active deadlocks
//!
//! This module implements a directed wait-for graph (WFG) that tracks runtime
//! dependencies between threads. It detects actual deadlocks by monitoring
//! situations where threads are blocked waiting for resources held by other threads.
//!
//! # How it works
//!
//! The graph maintains two internal mappings to ensure O(1) performance for both
//! edge addition and thread removal:
//! 1. *Forward Graph (⁠ edges ⁠)*: Maps ⁠ Thread A -> {Thread B} ⁠. Used to detect cycles (BFS).
//! 2. *Reverse Graph (⁠ incoming_edges ⁠)*: Maps ⁠ Thread B -> {Thread A} ⁠. Used to efficiently
//!    clean up dependencies when a thread exits without iterating the entire graph.
//!
//! When a thread (Thread A) attempts to acquire a resource held by another thread
//! (Thread B), we propose a directed edge ⁠ A -> B ⁠. Before adding this edge,
//! the graph checks if a path already exists from B to A (cycle detection).

use crate::core::types::ThreadId;
use fxhash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

/// Represents a directed graph of thread wait relationships
pub struct WaitForGraph {
    /// Maps a thread to all the threads it is waiting for (outgoing edges).
    /// Primary source for cycle detection.
    pub(crate) edges: FxHashMap<ThreadId, FxHashSet<ThreadId>>,

    /// Maps a thread to all threads that are waiting for it (incoming edges).
    /// Used for O(1) cleanup when a thread exits.
    pub(crate) incoming_edges: FxHashMap<ThreadId, FxHashSet<ThreadId>>,

    // Cached buffers for BFS to avoid repeated allocations
    bfs_queue: VecDeque<ThreadId>,
    bfs_visited: FxHashSet<ThreadId>,
    bfs_parent: FxHashMap<ThreadId, ThreadId>,
}

impl Default for WaitForGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl WaitForGraph {
    /// Create a new empty wait-for graph
    ///
    /// # Returns
    /// A new initialized WaitForGraph instance
    pub fn new() -> Self {
        Self {
            edges: FxHashMap::default(),
            incoming_edges: FxHashMap::default(),
            bfs_queue: VecDeque::with_capacity(64),
            bfs_visited: FxHashSet::default(),
            bfs_parent: FxHashMap::default(),
        }
    }

    /// Add a directed edge: ⁠ from ⁠ thread waits for ⁠ to ⁠ thread
    ///
    /// Adds the edge and detects if it would create a deadlock cycle.
    ///
    /// # Arguments
    /// * ⁠ from ⁠ - The thread ID that is waiting
    /// * ⁠ to ⁠ - The thread ID that holds the resource
    ///
    /// # Returns
    /// * ⁠ Some(Vec<ThreadId>) ⁠ - The cycle if adding this edge would create one
    /// * ⁠ None ⁠ - If no cycle would be created
    pub fn add_edge(&mut self, from: ThreadId, to: ThreadId) -> Option<Vec<ThreadId>> {
        // Optimization: Do not perform BFS if the edge already exists
        if let Some(targets) = self.edges.get(&from)
            && targets.contains(&to)
        {
            return None;
        }

        // Check if adding this edge would create a cycle
        // A cycle is created if there is already a path from 'to' to 'from'.
        if let Some(path) = self.find_path(to, from) {
            return Some(path);
        }

        // No cycle - add the Forward Edge
        self.edges.entry(from).or_default().insert(to);

        // Add the Reverse Edge (for efficient cleanup)
        self.incoming_edges.entry(to).or_default().insert(from);

        None
    }

    /// Clear the wait edges for a thread (what it's waiting for)
    ///
    /// This is typically called when a thread successfully acquires a lock
    /// and is no longer waiting.
    ///
    /// # Arguments
    /// * ⁠ thread_id ⁠ - The thread that stopped waiting
    pub fn clear_wait_edges(&mut self, thread_id: ThreadId) {
        // Remove the forward edges
        if let Some(targets) = self.edges.remove(&thread_id) {
            // Update the reverse mapping for every thread we were waiting on
            for target in targets {
                if let Some(waiters) = self.incoming_edges.get_mut(&target) {
                    waiters.remove(&thread_id);
                    // Cleanup empty sets to save memory
                    if waiters.is_empty() {
                        self.incoming_edges.remove(&target);
                    }
                }
            }
        }
    }

    /// Remove a specific directed edge: ⁠ from ⁠ thread waits for ⁠ to ⁠ thread
    ///
    /// # Arguments
    /// * ⁠ from ⁠ - The waiting thread
    /// * ⁠ to ⁠ - The target thread
    pub fn remove_edge(&mut self, from: ThreadId, to: ThreadId) {
        // Remove from forward graph
        if let Some(neighbors) = self.edges.get_mut(&from)
            && neighbors.remove(&to)
        {
            if neighbors.is_empty() {
                self.edges.remove(&from);
            }

            // Remove from reverse graph
            if let Some(waiters) = self.incoming_edges.get_mut(&to) {
                waiters.remove(&from);
                if waiters.is_empty() {
                    self.incoming_edges.remove(&to);
                }
            }
        }
    }

    /// Remove all edges for the specified thread (both incoming and outgoing)
    ///
    /// This is called when a thread exits. Unlike the naive implementation,
    /// this operation is efficient (proportional to neighbors, not total threads)
    /// thanks to the reverse graph.
    ///
    /// # Arguments
    /// * ⁠ thread_id ⁠ - ID of the thread being removed
    pub fn remove_thread(&mut self, thread_id: ThreadId) {
        // 1. Remove outgoing edges (Who was this thread waiting for?)
        self.clear_wait_edges(thread_id);

        // 2. Remove incoming edges (Who was waiting for this thread?)
        if let Some(waiters) = self.incoming_edges.remove(&thread_id) {
            for waiter in waiters {
                // For every thread that was waiting on the exiting thread,
                // remove the forward edge pointing to the exiting thread.
                if let Some(forward_set) = self.edges.get_mut(&waiter) {
                    forward_set.remove(&thread_id);
                    if forward_set.is_empty() {
                        self.edges.remove(&waiter);
                    }
                }
            }
        }
    }

    /// Find a path from start to target using BFS
    ///
    /// Used internally for cycle detection.
    fn find_path(&mut self, start: ThreadId, target: ThreadId) -> Option<Vec<ThreadId>> {
        if start == target {
            return Some(vec![start]);
        }

        // Reuse cached buffers
        self.bfs_queue.clear();
        self.bfs_visited.clear();
        self.bfs_parent.clear();

        self.bfs_queue.push_back(start);
        self.bfs_visited.insert(start);

        while let Some(current) = self.bfs_queue.pop_front() {
            if current == target {
                // Reconstruct path
                let mut path = Vec::with_capacity(self.bfs_parent.len() + 1);
                let mut curr = target;
                path.push(curr);
                while let Some(&p) = self.bfs_parent.get(&curr) {
                    path.push(p);
                    curr = p;
                }
                path.reverse();
                return Some(path);
            }

            if let Some(neighbors) = self.edges.get(&current) {
                for &neighbor in neighbors {
                    if self.bfs_visited.insert(neighbor) {
                        self.bfs_parent.insert(neighbor, current);
                        self.bfs_queue.push_back(neighbor);
                    }
                }
            }
        }

        None
    }
}

use crate::core::types::ThreadId;
use fxhash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

/// Represents a directed graph of thread wait relationships with optimized cycle detection
///
/// This implementation uses an incremental approach for cycle detection, which avoids
/// costly full graph traversals on each edge addition. It maintains specialized data structures
/// to efficiently detect potential cycles and track thread dependencies.
pub struct WaitForGraph {
    /// Maps a thread to all the threads it is waiting for (outgoing edges)
    pub(crate) edges: FxHashMap<ThreadId, FxHashSet<ThreadId>>,

    /// Reverse mapping for efficient backward traversal (maps thread to threads waiting for it)
    reverse_edges: FxHashMap<ThreadId, FxHashSet<ThreadId>>,

    /// Cached reachability information for fast cycle detection
    /// Maps each node to the set of nodes reachable from it
    reachability: FxHashMap<ThreadId, FxHashSet<ThreadId>>,
}

impl Default for WaitForGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl WaitForGraph {
    /// Create a new empty wait-for graph
    pub fn new() -> Self {
        WaitForGraph {
            edges: FxHashMap::default(),
            reverse_edges: FxHashMap::default(),
            reachability: FxHashMap::default(),
        }
    }

    /// Add a directed edge: `from` thread waits for `to` thread
    ///
    /// Adds the edge and detects if it would create a deadlock cycle.
    /// Uses optimized detection by checking reachability before the edge is added,
    /// and maintains reverse edges and reachability information for future checks.
    ///
    /// # Arguments
    /// * `from` - The thread ID that is waiting
    /// * `to` - The thread ID that holds the resource
    ///
    /// # Returns
    /// * `Some(Vec<ThreadId>)` - The cycle if adding this edge would create one
    /// * `None` - If no cycle would be created
    pub fn add_edge(&mut self, from: ThreadId, to: ThreadId) -> Option<Vec<ThreadId>> {
        // Check if adding this edge would create a cycle
        if self.can_reach(to, from) {
            return self.find_cycle_path(from, to);
        }

        // No cycle - add the edge
        self.edges.entry(from).or_default().insert(to);
        self.reverse_edges.entry(to).or_default().insert(from);

        // Ensure both nodes exist in the graph
        self.edges.entry(to).or_default();
        self.reverse_edges.entry(from).or_default();

        // Update reachability information incrementally
        self.update_reachability(from, to);

        None
    }

    /// Update reachability information after adding a new edge
    fn update_reachability(&mut self, from: ThreadId, to: ThreadId) {
        let to_reachable: Vec<ThreadId> = self
            .reachability
            .get(&to)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();

        let mut queue = VecDeque::new();
        queue.push_back(from);

        let mut visited = FxHashSet::default();
        visited.insert(from);

        while let Some(current) = queue.pop_front() {
            let reachable = self.reachability.entry(current).or_default();
            reachable.insert(to);
            reachable.extend(&to_reachable);

            if let Some(predecessors) = self.reverse_edges.get(&current) {
                for &pred in predecessors {
                    if !visited.contains(&pred) {
                        visited.insert(pred);
                        queue.push_back(pred);
                    }
                }
            }
        }
    }

    /// Find the exact cycle path when we know one exists
    ///
    /// Uses BFS to find the shortest path forming the cycle.
    /// Called after detecting that a cycle exists between the nodes.
    fn find_cycle_path(&self, from: ThreadId, to: ThreadId) -> Option<Vec<ThreadId>> {
        // Use BFS to find shortest path from 'to' to 'from'
        let mut queue = VecDeque::new();
        let mut parent = FxHashMap::default();
        let mut visited = FxHashSet::default();

        queue.push_back(to);
        visited.insert(to);

        while let Some(current) = queue.pop_front() {
            if current == from {
                // Found the cycle, reconstruct path
                let mut path = vec![current];
                let mut node = current;

                while let Some(&p) = parent.get(&node) {
                    path.push(p);
                    node = p;
                    if node == to {
                        break;
                    }
                }

                path.reverse();
                return Some(path);
            }

            if let Some(neighbors) = self.edges.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        parent.insert(neighbor, current);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        None
    }

    /// Check if one node can reach another using the reachability cache
    fn can_reach(&self, from: ThreadId, to: ThreadId) -> bool {
        if from == to {
            return true;
        }

        if let Some(reachable) = self.reachability.get(&from) {
            reachable.contains(&to)
        } else {
            false
        }
    }

    /// Clear the wait edges for a thread (what it's waiting for)
    ///
    /// Called when a thread successfully acquires a lock and is no longer waiting.
    /// Other threads may still be waiting for this thread, so we keep the thread
    /// in the graph and only clear its outgoing edges.
    pub fn clear_wait_edges(&mut self, thread_id: ThreadId) {
        // Remove outgoing edges and update reverse edges
        if let Some(outgoing) = self.edges.get_mut(&thread_id) {
            for neighbor in outgoing.iter() {
                if let Some(reverse) = self.reverse_edges.get_mut(neighbor) {
                    reverse.remove(&thread_id);
                }
            }
            outgoing.clear();
        }

        // Clear reachability for this thread since it no longer waits for anyone
        // Reachability for other threads will be updated incrementally on next add_edge()
        self.reachability.remove(&thread_id);
    }

    /// Remove all edges for the specified thread
    ///
    /// Completely removes the thread from the graph, updating all
    /// related data structures including edges, reverse edges,
    /// cycle tracking, and reachability information.
    pub fn remove_thread(&mut self, thread_id: ThreadId) {
        // Remove outgoing edges and update reverse edges
        if let Some(outgoing) = self.edges.remove(&thread_id) {
            for neighbor in outgoing {
                if let Some(reverse) = self.reverse_edges.get_mut(&neighbor) {
                    reverse.remove(&thread_id);
                }
            }
        }

        // Remove incoming edges and update main edges
        if let Some(incoming) = self.reverse_edges.remove(&thread_id) {
            for neighbor in incoming {
                if let Some(edges) = self.edges.get_mut(&neighbor) {
                    edges.remove(&thread_id);
                }
            }
        }

        // Update reachability information
        self.reachability.remove(&thread_id);
        for reachable in self.reachability.values_mut() {
            reachable.remove(&thread_id);
        }
    }
}

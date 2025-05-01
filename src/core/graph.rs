use crate::core::types::ThreadId;
use fxhash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

/// Represents a directed graph of thread wait relationships with efficient cycle detection
///
/// This implementation uses an incremental approach to cycle detection, avoiding
/// full DFS traversals on every edge addition. It maintains additional data structures
/// to quickly detect when a new edge would create a cycle.
pub struct WaitForGraph {
    /// Maps a thread to all the threads it is waiting for
    pub(crate) edges: FxHashMap<ThreadId, FxHashSet<ThreadId>>,

    /// Reverse edges for quick backward traversal (maps thread to threads waiting for it)
    reverse_edges: FxHashMap<ThreadId, FxHashSet<ThreadId>>,

    /// Tracks which nodes are in known cycles
    nodes_in_cycles: FxHashSet<ThreadId>,

    /// Cached cycle detection results (reachability matrix)
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
            nodes_in_cycles: FxHashSet::default(),
            reachability: FxHashMap::default(),
        }
    }

    /// Add a directed edge: `from` thread waits for `to` thread
    ///
    /// This method uses a more efficient approach by checking if the edge would create
    /// a cycle before adding it. It also maintains reverse edges and reachability info.
    ///
    /// # Arguments
    /// * `from` - The thread ID that is waiting
    /// * `to` - The thread ID that holds the resource
    ///
    /// # Returns
    /// * `Some(Vec<ThreadId>)` - The cycle if adding this edge would create one
    /// * `None` - If no cycle would be created
    pub fn add_edge(&mut self, from: ThreadId, to: ThreadId) -> Option<Vec<ThreadId>> {
        // Short circuit if we already know this would create a cycle
        if self.would_create_cycle(from, to) {
            // Find and return the actual cycle
            let cycle = self.find_cycle_path(from, to);
            if let Some(ref _cycle) = cycle {
                // Update nodes in cycles
                self.update_cycle_nodes(_cycle);
            }
            return cycle;
        }

        // Add edge and update reverse edges
        self.edges.entry(from).or_default().insert(to);
        self.reverse_edges.entry(to).or_default().insert(from);

        // Ensure both nodes exist in the graph
        self.edges.entry(to).or_default();
        self.reverse_edges.entry(from).or_default();

        // Update reachability information incrementally
        self.update_reachability(from, to);

        None
    }

    /// Check if adding an edge would create a cycle
    fn would_create_cycle(&self, from: ThreadId, to: ThreadId) -> bool {
        // If 'to' can reach 'from', adding this edge would create a cycle
        if let Some(reachable) = self.reachability.get(&to) {
            if reachable.contains(&from) {
                return true;
            }
        }

        // Check if both nodes are already in a cycle
        if self.nodes_in_cycles.contains(&from) && self.nodes_in_cycles.contains(&to) {
            // Perform a quick check to see if they're in the same cycle
            return self.are_in_same_cycle(from, to);
        }

        false
    }

    /// Update reachability information incrementally
    fn update_reachability(&mut self, from: ThreadId, to: ThreadId) {
        // BFS to update reachability from 'from' node
        let mut queue = VecDeque::new();
        queue.push_back(from);

        let mut visited = FxHashSet::default();
        visited.insert(from);

        while let Some(current) = queue.pop_front() {
            // First collect all nodes that 'to' can reach
            let to_reachable: Vec<ThreadId> = self
                .reachability
                .get(&to)
                .map(|set| set.iter().copied().collect())
                .unwrap_or_default();

            // Then update reachability for current node
            let reachable = self.reachability.entry(current).or_default();
            reachable.insert(to);
            reachable.extend(to_reachable);

            // Propagate to predecessors
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

    /// Find the actual cycle path when we know one exists
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

    /// Update nodes in cycles tracking
    fn update_cycle_nodes(&mut self, cycle: &[ThreadId]) {
        for &node in cycle {
            self.nodes_in_cycles.insert(node);
        }
    }

    /// Check if two nodes are in the same cycle
    fn are_in_same_cycle(&self, node1: ThreadId, node2: ThreadId) -> bool {
        // Simple check - if both can reach each other, they're in the same cycle
        if let Some(reachable1) = self.reachability.get(&node1) {
            if reachable1.contains(&node2) {
                if let Some(reachable2) = self.reachability.get(&node2) {
                    return reachable2.contains(&node1);
                }
            }
        }
        false
    }

    /// Remove all edges for the specified thread (both incoming and outgoing)
    pub fn remove_thread(&mut self, thread_id: ThreadId) {
        // Remove from cycle tracking
        self.nodes_in_cycles.remove(&thread_id);

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

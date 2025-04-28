use crate::core::types::ThreadId;
use std::collections::{HashMap, HashSet};

/// Represents a directed graph of thread wait relationships
///
/// The WaitForGraph tracks which threads are waiting for which other threads,
/// allowing the detector to identify cycles that indicate potential deadlocks.
///
/// # How it works
///
/// The graph is represented as an adjacency list, where each node is a thread ID
/// and each edge represents a "waits for" relationship. When thread A attempts to
/// acquire a lock owned by thread B, an edge is added from A to B.
///
/// Deadlock detection works by searching for cycles in this graph. A cycle indicates
/// a circular wait condition that can lead to a deadlock.
pub struct WaitForGraph {
    /// Maps a thread to all the threads it is waiting for
    pub(crate) edges: HashMap<ThreadId, HashSet<ThreadId>>,
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
            edges: HashMap::new(),
        }
    }

    /// Add a directed edge: `from` thread waits for `to` thread
    ///
    /// This method adds a "waits for" relationship between two threads, indicating
    /// that the `from` thread is waiting to acquire a resource held by the `to` thread.
    ///
    /// # Arguments
    /// * `from` - The thread ID that is waiting
    /// * `to` - The thread ID that holds the resource
    pub fn add_edge(&mut self, from: ThreadId, to: ThreadId) {
        self.edges.entry(from).or_default().insert(to);
        // Ensure 'to' exists in the graph even if it has no outgoing edges
        self.edges.entry(to).or_default();
    }

    /// Remove all edges for the specified thread (both incoming and outgoing)
    ///
    /// This method is called when a thread exits or acquires a lock, to clean up
    /// any wait relationships that are no longer valid.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the thread to remove from the graph
    pub fn remove_thread(&mut self, thread_id: ThreadId) {
        // Remove outgoing edges
        self.edges.remove(&thread_id);

        // Remove incoming edges
        for (_from, to_set) in self.edges.iter_mut() {
            to_set.remove(&thread_id);
        }
    }

    /// Detect if there is a cycle in the graph, starting from the given thread.
    ///
    /// This method uses a depth-first search (DFS) algorithm to detect cycles in the
    /// wait-for graph. A cycle indicates a potential deadlock situation.
    ///
    /// # Arguments
    /// * `start` - The thread ID to start the search from
    ///
    /// # Returns
    /// * `Some(Vec<ThreadId>)` - The cycle as a vector of thread IDs if found
    /// * `None` - If no cycle is found
    pub fn detect_cycle_from(&self, start: ThreadId) -> Option<Vec<ThreadId>> {
        if !self.edges.contains_key(&start) {
            return None;
        }

        // Using DFS to detect cycles
        let mut visited = HashSet::new();
        let mut path = Vec::new();
        let mut path_set = HashSet::new();

        fn dfs(
            graph: &WaitForGraph,
            current: ThreadId,
            visited: &mut HashSet<ThreadId>,
            path: &mut Vec<ThreadId>,
            path_set: &mut HashSet<ThreadId>,
        ) -> Option<Vec<ThreadId>> {
            if path_set.contains(&current) {
                // Found a cycle - extract the cycle part of the path
                let cycle_start = path.iter().position(|&id| id == current).unwrap();
                return Some(path[cycle_start..].to_vec());
            }

            if visited.contains(&current) {
                return None;
            }

            visited.insert(current);
            path.push(current);
            path_set.insert(current);

            if let Some(neighbors) = graph.edges.get(&current) {
                for &neighbor in neighbors {
                    if let Some(cycle) = dfs(graph, neighbor, visited, path, path_set) {
                        return Some(cycle);
                    }
                }
            }

            path.pop();
            path_set.remove(&current);
            None
        }

        dfs(self, start, &mut visited, &mut path, &mut path_set)
    }

    /// Detect any cycle in the graph.
    ///
    /// This method checks every node in the graph as a potential starting point
    /// for detecting a cycle. It is used for testing and validation of the graph.
    ///
    /// # Returns
    /// * `Some(Vec<ThreadId>)` - The cycle as a vector of thread IDs if found
    /// * `None` - If no cycle is found
    #[cfg(test)]
    pub fn detect_cycle(&self) -> Option<Vec<ThreadId>> {
        for &thread_id in self.edges.keys() {
            if let Some(cycle) = self.detect_cycle_from(thread_id) {
                return Some(cycle);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cycle() {
        let mut graph = WaitForGraph::new();
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        assert_eq!(graph.detect_cycle(), None);
    }

    #[test]
    fn test_simple_cycle() {
        let mut graph = WaitForGraph::new();
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        graph.add_edge(3, 1);
        assert!(graph.detect_cycle().is_some());
    }

    #[test]
    fn test_remove_thread_breaks_cycle() {
        let mut graph = WaitForGraph::new();
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        graph.add_edge(3, 1);
        assert!(graph.detect_cycle().is_some());

        graph.remove_thread(2);
        assert_eq!(graph.detect_cycle(), None);
    }
}

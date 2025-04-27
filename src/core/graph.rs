use crate::core::types::ThreadId;
use std::collections::{HashMap, HashSet};

/// Represents a directed graph of thread wait relationships
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
    pub fn add_edge(&mut self, from: ThreadId, to: ThreadId) {
        self.edges.entry(from).or_default().insert(to);
        // Ensure 'to' exists in the graph even if it has no outgoing edges
        self.edges.entry(to).or_default();
    }

    /// Remove all edges for the specified thread (both incoming and outgoing)
    pub fn remove_thread(&mut self, thread_id: ThreadId) {
        // Remove outgoing edges
        self.edges.remove(&thread_id);

        // Remove incoming edges
        for (_from, to_set) in self.edges.iter_mut() {
            to_set.remove(&thread_id);
        }
    }

    /// Detect if there is a cycle in the graph, starting from the given thread.
    /// Returns the cycle as a vector of thread IDs if found.
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
    /// Returns the cycle as a vector of thread IDs if found.
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

use crate::ThreadId;
use crate::core::detector::GLOBAL_DETECTOR;
use crate::core::{Detector, Events};

impl Detector {
    /// Register a thread spawn
    ///
    /// This method is called when a new thread is created. It records the thread
    /// in the wait-for graph and establishes parent-child relationships for proper
    /// resource tracking.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the newly spawned thread
    /// * `parent_id` - Optional ID of the parent thread that created this thread
    pub fn on_thread_spawn(&mut self, thread_id: ThreadId, parent_id: Option<ThreadId>) {
        if let Some(logger) = &self.logger {
            logger.log_thread_event(thread_id, parent_id, Events::Spawn);
        }

        // Ensure node exists in the wait-for graph
        self.wait_for_graph.edges.entry(thread_id).or_default();
    }

    /// Register a thread exit
    ///
    /// This method is called when a thread is about to exit. It cleans up resources
    /// associated with the thread and updates the wait-for graph.
    ///
    /// # Arguments
    /// * `thread_id` - ID of the exiting thread
    pub fn on_thread_exit(&mut self, thread_id: ThreadId) {
        if let Some(logger) = &self.logger {
            logger.log_thread_event(thread_id, None, Events::Exit);
        }

        // remove thread and its edges from the wait-for graph
        self.wait_for_graph.remove_thread(thread_id);
        // no more held locks
        self.thread_holds.remove(&thread_id);
    }
}

/// Register a thread spawn with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the spawned thread
/// * `parent_id` - Optional ID of the parent thread that created this thread
pub fn on_thread_spawn(thread_id: ThreadId, parent_id: Option<ThreadId>) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_thread_spawn(thread_id, parent_id);
}

/// Register a thread exit with the global detector
///
/// # Arguments
/// * `thread_id` - ID of the exiting thread
pub fn on_thread_exit(thread_id: ThreadId) {
    let mut detector = GLOBAL_DETECTOR.lock();
    detector.on_thread_exit(thread_id);
}

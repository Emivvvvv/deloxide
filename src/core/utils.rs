use crate::ThreadId;
use std::thread;

/// Get a unique identifier of the current thread
pub fn get_current_thread_id() -> ThreadId {
    thread::current().id().as_u64().get() as usize
}

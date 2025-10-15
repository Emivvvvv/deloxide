use crate::ThreadId;
use crate::core::detector::thread::{exit_thread, spawn_thread};
use crate::core::get_current_thread_id;
use std::os::raw::c_int;

/// Register a thread spawn with the global detector.
///
/// This function should be called when a new thread is created to enable
/// proper tracking of thread relationships and resources.
///
/// # Arguments
/// * `thread_id` - ID of the spawned thread.
/// * `parent_id` - ID of the parent thread that created this thread, or 0 for no parent.
///
/// # Returns
/// * `0` on success
///
/// # Safety
/// - The caller must ensure thread_id represents a real thread.
/// - This function is normally called automatically by the CREATE_TRACKED_THREAD macro.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_register_thread_spawn(
    thread_id: usize,
    parent_id: usize,
) -> c_int {
    let parent = if parent_id == 0 {
        None
    } else {
        Some(parent_id as ThreadId)
    };
    spawn_thread(thread_id as ThreadId, parent);
    0 // Success
}

/// Register a thread exit with the global detector.
///
/// This function should be called when a thread is about to exit to ensure
/// proper cleanup of resources owned by the thread.
///
/// # Arguments
/// * `thread_id` - ID of the exiting thread.
///
/// # Returns
/// * `0` on success
///
/// # Safety
/// - The caller must ensure thread_id represents a real thread that is exiting.
/// - This function is normally called automatically by the CREATE_TRACKED_THREAD macro.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_register_thread_exit(thread_id: usize) -> c_int {
    exit_thread(thread_id as ThreadId);
    0 // Success
}

/// Get the current thread ID.
///
/// Returns a unique identifier for the current thread that can be used
/// with other Deloxide functions.
///
/// # Returns
/// A unique identifier for the current thread as an unsigned long
///
/// # Safety
/// This function is safe to call from FFI contexts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_get_thread_id() -> usize {
    get_current_thread_id() as usize
}

use crate::core::detector::rwlock::create_rwlock;
use crate::core::locks::rwlock::RwLock;
use crate::core::types::ThreadId;
use std::cell::RefCell;
use std::ffi::{c_int, c_void};

// Each thread can hold one read and one write guard at a time (per-thread tracking)
thread_local! {
    static FFI_RW_READ_GUARD: RefCell<Option<crate::core::locks::rwlock::RwLockReadGuard<'static, ()>>> = const {RefCell::new(None)};
    static FFI_RW_WRITE_GUARD: RefCell<Option<crate::core::locks::rwlock::RwLockWriteGuard<'static, ()>>> = const {RefCell::new(None)};
}

/// Create a new tracked RwLock (reader-writer lock).
///
/// # Returns
/// * Void pointer to the RwLock, or NULL on allocation failure
///
/// # Safety
/// - The returned pointer is a raw pointer and must be destroyed with `deloxide_destroy_rwlock`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_create_rwlock() -> *mut c_void {
    let rwlock = Box::new(RwLock::new(()));
    Box::into_raw(rwlock) as *mut c_void
}

/// Create a new tracked RwLock with specified creator thread ID.
///
/// # Arguments
/// * `creator_thread_id` - ID of the thread to register as the creator
///
/// # Returns
/// * Void pointer to the RwLock, or NULL on allocation failure
///
/// # Safety
/// - The pointer must be freed using `deloxide_destroy_rwlock`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_create_rwlock_with_creator(
    creator_thread_id: usize,
) -> *mut c_void {
    let rwlock = Box::new(RwLock::new(()));
    create_rwlock(rwlock.id(), Some(creator_thread_id as ThreadId));
    Box::into_raw(rwlock) as *mut c_void
}

/// Destroy a tracked RwLock.
///
/// # Arguments
/// * `rwlock` - Pointer to an RwLock created with `deloxide_create_rwlock`.
///
/// # Safety
/// - Must not use `rwlock` after this.
/// - Must be a valid pointer from `deloxide_create_rwlock`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_destroy_rwlock(rwlock: *mut c_void) {
    if !rwlock.is_null() {
        unsafe {
            drop(Box::from_raw(rwlock as *mut RwLock<()>));
        }
    }
}

/// Lock an RwLock for reading.
///
/// # Arguments
/// * `rwlock` - Pointer to an RwLock.
///
/// # Returns
/// * `0` on success
/// * `-1` if pointer is NULL
///
/// # Safety
/// - Do not call twice from the same thread without unlocking.
/// - Must use `deloxide_rw_unlock_read` to unlock.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_rw_lock_read(rwlock: *mut c_void) -> c_int {
    if rwlock.is_null() {
        return -1;
    }
    let rwlock_ref = unsafe { &*(rwlock as *const RwLock<()>) };
    let guard = rwlock_ref.read();
    unsafe {
        FFI_RW_READ_GUARD.with(|slot| {
            *slot.borrow_mut() = Some(std::mem::transmute::<
                crate::core::locks::rwlock::RwLockReadGuard<'_, ()>,
                crate::core::locks::rwlock::RwLockReadGuard<'_, ()>,
            >(guard))
        });
    }
    0
}

/// Unlock an RwLock after reading.
///
/// # Arguments
/// * `rwlock` - Pointer to an RwLock.
///
/// # Returns
/// * `0` on success
/// * `-1` if pointer is NULL
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_rw_unlock_read(rwlock: *mut c_void) -> c_int {
    if rwlock.is_null() {
        return -1;
    }
    FFI_RW_READ_GUARD.with(|slot| {
        let _ = slot.borrow_mut().take();
    });
    0
}

/// Lock an RwLock for writing.
///
/// # Arguments
/// * `rwlock` - Pointer to an RwLock.
///
/// # Returns
/// * `0` on success
/// * `-1` if pointer is NULL
///
/// # Safety
/// - Do not call twice from the same thread without unlocking.
/// - Must use `deloxide_rw_unlock_write` to unlock.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_rw_lock_write(rwlock: *mut c_void) -> c_int {
    if rwlock.is_null() {
        return -1;
    }
    let rwlock_ref = unsafe { &*(rwlock as *const RwLock<()>) };
    let guard = rwlock_ref.write();
    unsafe {
        FFI_RW_WRITE_GUARD.with(|slot| {
            *slot.borrow_mut() = Some(std::mem::transmute::<
                crate::core::locks::rwlock::RwLockWriteGuard<'_, ()>,
                crate::core::locks::rwlock::RwLockWriteGuard<'_, ()>,
            >(guard))
        });
    }

    0
}

/// Unlock an RwLock after writing.
///
/// # Arguments
/// * `rwlock` - Pointer to an RwLock.
///
/// # Returns
/// * `0` on success
/// * `-1` if pointer is NULL
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_rw_unlock_write(rwlock: *mut c_void) -> c_int {
    if rwlock.is_null() {
        return -1;
    }
    FFI_RW_WRITE_GUARD.with(|slot| {
        let _ = slot.borrow_mut().take();
    });
    0
}

/// Get the creator thread ID of an RwLock.
///
/// # Arguments
/// * `rwlock` - Pointer to an RwLock.
///
/// # Returns
/// * Creator thread ID, or 0 if `rwlock` is NULL
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_get_rwlock_creator(rwlock: *mut c_void) -> usize {
    if rwlock.is_null() {
        return 0;
    }
    let rwlock_ref = unsafe { &*(rwlock as *const RwLock<()>) };
    rwlock_ref.creator_thread_id()
}

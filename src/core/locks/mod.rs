pub mod condvar;
pub mod mutex;
pub mod rwlock;

use std::sync::atomic::AtomicUsize;

pub(crate) static NEXT_LOCK_ID: AtomicUsize = AtomicUsize::new(1);

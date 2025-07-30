pub mod mutex;
pub mod rwlock;

use std::sync::atomic::AtomicUsize;

static NEXT_LOCK_ID: AtomicUsize = AtomicUsize::new(1);

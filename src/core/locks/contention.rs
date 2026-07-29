use std::sync::atomic::{AtomicUsize, Ordering};

pub(crate) struct ContentionState {
    slow_waiters: AtomicUsize,
}

pub(crate) struct SlowWaiter<'a> {
    state: &'a ContentionState,
}

impl ContentionState {
    pub(crate) const fn new() -> Self {
        Self {
            slow_waiters: AtomicUsize::new(0),
        }
    }

    pub(crate) fn register(&self) -> SlowWaiter<'_> {
        self.slow_waiters.fetch_add(1, Ordering::AcqRel);
        SlowWaiter { state: self }
    }

    pub(crate) fn has_waiters(&self) -> bool {
        self.slow_waiters.load(Ordering::Acquire) != 0
    }

    #[cfg(test)]
    pub(crate) fn waiter_count(&self) -> usize {
        self.slow_waiters.load(Ordering::Acquire)
    }
}

impl Drop for SlowWaiter<'_> {
    fn drop(&mut self) {
        self.state.slow_waiters.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waiter_token_balances_count() {
        let state = ContentionState::new();
        assert!(!state.has_waiters());

        {
            let _waiter = state.register();
            assert!(state.has_waiters());
            assert_eq!(state.waiter_count(), 1);
        }

        assert!(!state.has_waiters());
    }
}

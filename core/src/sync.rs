//! Short locks on shared state.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `mutex`. A poisoned lock is no reason to stop: the value is intact (a panic in
/// another holder does not undo its counters or its database connection), and without it
/// the app could not go on at all.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poisoned_lock_still_gives_the_value() {
        let mutex = std::sync::Arc::new(Mutex::new(5));
        let other = mutex.clone();
        let _ = std::thread::spawn(move || {
            let _guard = other.lock().unwrap();
            panic!("poison the lock");
        })
        .join();
        assert!(mutex.is_poisoned());
        assert_eq!(*lock(&mutex), 5);
    }
}

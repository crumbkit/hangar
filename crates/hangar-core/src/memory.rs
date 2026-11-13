//! In-memory stores for tests.
//!
//! Each store is a [`std::sync::Mutex`] over maps. Every operation is one
//! step under the lock, so no state sits between two steps. A binding's
//! operation reads the type of each bound record first, without the lock,
//! because the read waits on another store. A save then takes the lock once
//! and, in that one step, checks the name, the version, the origin and the
//! fit and changes the binding. A move and a fit check first read the
//! binding's fills under the lock, release it, read the records, and take the
//! lock again. The check and the change happen in that later step. The fit
//! holds for the records as that read saw them, as
//! [`PageStore`](crate::PageStore) says.
//!
//! A poisoned lock means a thread panicked in the middle of a step, and the
//! maps may be half-changed, so every operation returns `Unavailable`
//! (BASE-50). The stores hold no code that panics under the lock, so a
//! poisoned lock comes from a caller's own panic in a test, and it stays
//! poisoned.

mod pages;
mod records;

pub use pages::MemoryPageStore;
pub use records::MemoryRecordStore;

use std::sync::{Mutex, MutexGuard};

/// The one place that takes a lock. A poisoned lock yields `None`, and the
/// caller turns that into its own `Unavailable` error.
fn locked<T>(mutex: &Mutex<T>) -> Option<MutexGuard<'_, T>> {
    mutex.lock().ok()
}

#[cfg(test)]
mod tests;

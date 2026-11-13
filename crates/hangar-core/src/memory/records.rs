//! The in-memory [`RecordStore`].

use super::locked;
use crate::provenance::WriteProvenance;
use crate::record::{Blob, Record, RecordBody, RecordPath, RecordType};
use crate::store::{ReadError, RecordStore, WriteError};
use std::collections::HashMap;
use std::sync::Mutex;

pub(super) struct RecordEntry {
    record_type: RecordType,
    pub(super) body: RecordBody,
    pub(super) blob: Blob,
    pub(super) last_write: Option<WriteProvenance>,
}

#[derive(Default)]
pub(super) struct RecordState {
    next_blob: u64,
    pub(super) records: HashMap<RecordPath, RecordEntry>,
}

impl RecordState {
    /// A blob that no earlier call returned.
    ///
    /// Shortcut: the blob is a counter value written as 40 hex digits. It
    /// handles every use in one process, because no two writes get the same
    /// value, and so it meets the rule that a blob never repeats for one
    /// record. It misses content addressing, so two writes of one body get
    /// different blobs and a blob from another store means nothing here. A
    /// store backed by Git replaces it with the commit id of the record.
    fn fresh_blob(&mut self) -> Option<Blob> {
        self.next_blob = self.next_blob.checked_add(1)?;
        Blob::try_from(format!("{:040x}", self.next_blob).as_str()).ok()
    }
}

/// A [`RecordStore`] that keeps its records in memory.
#[derive(Default)]
pub struct MemoryRecordStore {
    pub(super) state: Mutex<RecordState>,
}

impl std::fmt::Debug for MemoryRecordStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MemoryRecordStore")
            .finish_non_exhaustive()
    }
}

impl MemoryRecordStore {
    /// Builds an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates the record at `path`, or replaces it, and returns its blob.
    /// The [`RecordStore`] trait cannot create a record, so tests seed one.
    ///
    /// This method bypasses the blob check. It replaces an existing record
    /// whatever its blob, can change the record's type and clears the
    /// record's last write, so a write that already returned `Ok` is undone
    /// without an error. It exists only under the `testing` feature and in
    /// this crate's own tests.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Unavailable`] when the lock is poisoned.
    #[cfg(any(test, feature = "testing"))]
    pub fn seed(
        &self,
        path: RecordPath,
        record_type: RecordType,
        body: RecordBody,
    ) -> Result<Blob, WriteError> {
        let mut state = locked(&self.state).ok_or(WriteError::Unavailable)?;
        let blob = state.fresh_blob().ok_or(WriteError::Unavailable)?;
        state.records.insert(
            path,
            RecordEntry {
                record_type,
                body,
                blob: blob.clone(),
                last_write: None,
            },
        );
        Ok(blob)
    }

    /// The [`WriteProvenance`] of the last successful write to `path`, or
    /// `None` when the record was never written or does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Unavailable`] when the lock is poisoned, so a
    /// poisoned store does not look like a record that was never written.
    pub fn last_write(&self, path: &RecordPath) -> Result<Option<WriteProvenance>, WriteError> {
        let state = locked(&self.state).ok_or(WriteError::Unavailable)?;
        Ok(state
            .records
            .get(path)
            .and_then(|entry| entry.last_write.clone()))
    }
}

impl RecordStore for MemoryRecordStore {
    async fn read(&self, path: &RecordPath) -> Result<Record, ReadError> {
        let state = locked(&self.state).ok_or(ReadError::Unavailable)?;
        let entry = state.records.get(path).ok_or(ReadError::NotFound)?;
        Ok(Record::new(
            path.clone(),
            entry.record_type.clone(),
            entry.blob.clone(),
            entry.body.clone(),
        ))
    }

    async fn write(
        &self,
        path: &RecordPath,
        body: &RecordBody,
        provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        let mut state = locked(&self.state).ok_or(WriteError::Unavailable)?;
        let current = state.records.get(path).ok_or(WriteError::NotFound)?;
        if current.blob != *provenance.based_on() {
            return Err(WriteError::Conflict);
        }
        let blob = state.fresh_blob().ok_or(WriteError::Unavailable)?;
        let entry = state.records.get_mut(path).ok_or(WriteError::NotFound)?;
        entry.body = body.clone();
        entry.blob = blob.clone();
        entry.last_write = Some(provenance.clone());
        Ok(blob)
    }
}

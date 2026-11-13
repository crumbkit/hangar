//! In-memory stores for tests.
//!
//! Each store is a [`std::sync::Mutex`] over maps. Every operation is one
//! step under the lock, so no state sits between two steps. A binding's
//! operation reads the type of each bound record first, without the lock,
//! because the read waits on another store. It then takes the lock once and,
//! in that one step, checks the name, the version, the origin and the fit and
//! inserts the binding.
//!
//! A poisoned lock means a thread panicked in the middle of a step, and the
//! maps may be half-changed, so every operation returns `Unavailable`
//! (BASE-50).

use crate::binding::Binding;
use crate::name::{BindingName, PageName, PageVersion};
use crate::page::{Page, PagePin, PublishedPage};
use crate::provenance::{MadeFrom, Provenance, WriteProvenance};
use crate::record::{Blob, Record, RecordBody, RecordPath, RecordType};
use crate::store::{FillTypes, PageStore, PageStoreError, ReadError, RecordStore, WriteError};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, MutexGuard};

/// The one place that takes a lock. A poisoned lock yields `None`, and the
/// caller turns that into its own `Unavailable` error.
fn locked<T>(mutex: &Mutex<T>) -> Option<MutexGuard<'_, T>> {
    mutex.lock().ok()
}

struct RecordEntry {
    record_type: RecordType,
    body: RecordBody,
    blob: Blob,
    last_write: Option<WriteProvenance>,
}

#[derive(Default)]
struct RecordState {
    next_blob: u64,
    records: HashMap<RecordPath, RecordEntry>,
}

impl RecordState {
    /// A blob that no earlier call returned.
    ///
    /// Shortcut: the blob is a counter value written as 40 hex digits. It
    /// handles every use in one process, because no two writes get the same
    /// value. It misses content addressing, so two writes of one body get
    /// different blobs and a blob from another store means nothing here. A
    /// store backed by Git replaces it with the object id of the record.
    fn fresh_blob(&mut self) -> Option<Blob> {
        self.next_blob = self.next_blob.checked_add(1)?;
        Blob::try_from(format!("{:040x}", self.next_blob).as_str()).ok()
    }
}

/// A [`RecordStore`] that keeps its records in memory.
#[derive(Default)]
pub struct MemoryRecordStore {
    state: Mutex<RecordState>,
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
    /// # Errors
    ///
    /// Returns [`WriteError::Unavailable`] when the lock is poisoned.
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
    /// `None` when the record was never written, does not exist or the lock
    /// is poisoned.
    #[must_use]
    pub fn last_write(&self, path: &RecordPath) -> Option<WriteProvenance> {
        locked(&self.state)?.records.get(path)?.last_write.clone()
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

#[derive(Default)]
struct PageState {
    /// The versions of each page, oldest first.
    pages: BTreeMap<PageName, Vec<PublishedPage>>,
    bindings: BTreeMap<BindingName, Binding>,
}

impl PageState {
    fn version_of(&self, pin: &PagePin) -> Option<&PublishedPage> {
        self.pages
            .get(pin.name())?
            .iter()
            .find(|published| published.version() == pin.version())
    }

    fn origin_exists(&self, made_from: &MadeFrom) -> bool {
        match made_from {
            MadeFrom::Original => true,
            MadeFrom::Page(pin) => self.version_of(pin).is_some(),
            MadeFrom::Binding(name) => self.bindings.contains_key(name),
        }
    }
}

/// A [`PageStore`] that keeps its pages and bindings in memory. A page's
/// versions are named `v1`, `v2` and so on.
#[derive(Default)]
pub struct MemoryPageStore {
    state: Mutex<PageState>,
}

impl std::fmt::Debug for MemoryPageStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MemoryPageStore")
            .finish_non_exhaustive()
    }
}

impl MemoryPageStore {
    /// Builds an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl PageStore for MemoryPageStore {
    async fn publish(
        &self,
        page: Page,
        provenance: Provenance,
    ) -> Result<PageVersion, PageStoreError> {
        let mut state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        if !state.origin_exists(provenance.made_from()) {
            return Err(PageStoreError::UnresolvedOrigin);
        }
        let versions = state.pages.entry(page.name().clone()).or_default();
        // Shortcut: versions are named v1, v2 and so on by position. That
        // handles one process that never deletes a version. It misses
        // versions that outlive the process and versions made by several
        // writers, and a store backed by Git replaces the names with commit
        // ids.
        let version = PageVersion::try_from(format!("v{}", versions.len() + 1).as_str())
            .map_err(|_| PageStoreError::Unavailable)?;
        versions.push(PublishedPage::new(page, version.clone(), provenance));
        Ok(version)
    }

    async fn latest(&self, name: &PageName) -> Result<PublishedPage, PageStoreError> {
        let state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        let versions = state.pages.get(name).ok_or(PageStoreError::NotFound)?;
        versions.last().cloned().ok_or(PageStoreError::NotFound)
    }

    async fn at(&self, pin: &PagePin) -> Result<PublishedPage, PageStoreError> {
        let state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        state
            .version_of(pin)
            .cloned()
            .ok_or(PageStoreError::NotFound)
    }

    async fn versions(&self, name: &PageName) -> Result<Vec<PageVersion>, PageStoreError> {
        let state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        let versions = state.pages.get(name).ok_or(PageStoreError::NotFound)?;
        Ok(versions
            .iter()
            .rev()
            .map(|published| published.version().clone())
            .collect())
    }

    async fn pages(&self) -> Result<Vec<PageName>, PageStoreError> {
        let state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        Ok(state.pages.keys().cloned().collect())
    }

    async fn save_binding(
        &self,
        binding: Binding,
        records: &impl RecordStore,
    ) -> Result<(), PageStoreError> {
        let types = FillTypes::read(binding.fills(), records).await;
        let mut state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        if state.bindings.contains_key(binding.name()) {
            return Err(PageStoreError::NameTaken);
        }
        let published = state
            .version_of(binding.pin())
            .ok_or(PageStoreError::UnknownVersion)?;
        if !state.origin_exists(binding.provenance().made_from()) {
            return Err(PageStoreError::UnresolvedOrigin);
        }
        types.check(published.page())?;
        state.bindings.insert(binding.name().clone(), binding);
        Ok(())
    }

    async fn binding(&self, name: &BindingName) -> Result<Binding, PageStoreError> {
        let state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        state
            .bindings
            .get(name)
            .cloned()
            .ok_or(PageStoreError::NotFound)
    }

    async fn bindings_of(&self, name: &PageName) -> Result<Vec<Binding>, PageStoreError> {
        let state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        if !state.pages.contains_key(name) {
            return Err(PageStoreError::NotFound);
        }
        Ok(state
            .bindings
            .values()
            .filter(|binding| binding.pin().name() == name)
            .cloned()
            .collect())
    }

    async fn move_binding(
        &self,
        name: &BindingName,
        version: &PageVersion,
        records: &impl RecordStore,
    ) -> Result<(), PageStoreError> {
        // No operation changes a binding's fills, so the fills read here are
        // the fills that the locked step below checks.
        let fills = {
            let state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
            state
                .bindings
                .get(name)
                .ok_or(PageStoreError::NotFound)?
                .fills()
                .clone()
        };
        let types = FillTypes::read(&fills, records).await;
        let mut state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        let current = state.bindings.get(name).ok_or(PageStoreError::NotFound)?;
        let target = PagePin::new(current.pin().name().clone(), version.clone());
        let published = state
            .version_of(&target)
            .ok_or(PageStoreError::UnknownVersion)?;
        types.check(published.page())?;
        let moved = current.with_version(version.clone());
        state.bindings.insert(name.clone(), moved);
        Ok(())
    }
}

#[cfg(test)]
mod tests;

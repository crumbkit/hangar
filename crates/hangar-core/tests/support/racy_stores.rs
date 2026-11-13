//! Stores that check first and act later, for the tests that show the
//! concurrency clauses of the contract suites catch a store that is correct
//! one call at a time. Each double waits at a two-party barrier between its
//! check and its action, so two callers that run together both pass the
//! check. A clause that calls one of these doubles from a single thread
//! waits for ever, so only the concurrency clauses run against them.

use hangar_core::contract::block_on;
use hangar_core::memory::{MemoryPageStore, MemoryRecordStore};
use hangar_core::{
    Binding, BindingName, Blob, MoveError, Page, PageName, PagePin, PageStore, PageStoreError,
    PageVersion, Provenance, PublishedPage, ReadError, Record, RecordBody, RecordPath, RecordStore,
    WriteError, WriteProvenance,
};
use std::sync::{Barrier, Mutex};

/// A record store that compares the blob, waits for the other caller and
/// then writes over whatever is there.
///
/// The write that comes second runs after the first one has finished, so its
/// second read sees the first write's blob and the stale compare of the
/// first read is the only one that runs.
pub(crate) struct RacyRecordWrites {
    inner: MemoryRecordStore,
    barrier: Barrier,
    serial: Mutex<()>,
}

impl RacyRecordWrites {
    pub(crate) fn new(inner: MemoryRecordStore) -> Self {
        Self {
            inner,
            barrier: Barrier::new(2),
            serial: Mutex::new(()),
        }
    }
}

impl RecordStore for RacyRecordWrites {
    async fn read(&self, path: &RecordPath) -> Result<Record, ReadError> {
        self.inner.read(path).await
    }

    async fn write(
        &self,
        path: &RecordPath,
        body: &RecordBody,
        provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        let seen = self
            .inner
            .read(path)
            .await
            .map_err(|_| WriteError::NotFound)?;
        if seen.blob() != provenance.based_on() {
            return Err(WriteError::Conflict);
        }
        self.barrier.wait();
        let _one_at_a_time = self.serial.lock().map_err(|_| WriteError::Unavailable)?;
        block_on(async {
            let now = self
                .inner
                .read(path)
                .await
                .map_err(|_| WriteError::NotFound)?;
            let forced = WriteProvenance::new(
                now.blob().clone(),
                provenance.made_by().clone(),
                provenance.through().clone(),
            );
            self.inner.write(path, body, &forced).await
        })
    }
}

/// The operation of a [`RacyPageStore`] that races.
#[derive(Clone, Copy)]
pub(crate) enum Race {
    /// `publish` names the version from the count it read before the wait.
    Publish,
    /// `save_binding` trusts a name check that it made before the wait.
    Save,
    /// `move_binding` compares the pin, waits and then moves from whatever
    /// pin it finds.
    Move,
}

/// A page store that is correct one call at a time and races in one
/// operation.
pub(crate) struct RacyPageStore {
    inner: MemoryPageStore,
    race: Race,
    barrier: Barrier,
    serial: Mutex<()>,
}

impl RacyPageStore {
    pub(crate) fn new(race: Race) -> Self {
        Self {
            inner: MemoryPageStore::new(),
            race,
            barrier: Barrier::new(2),
            serial: Mutex::new(()),
        }
    }
}

impl PageStore for RacyPageStore {
    async fn publish(
        &self,
        page: Page,
        provenance: Provenance,
    ) -> Result<PageVersion, PageStoreError> {
        if !matches!(self.race, Race::Publish) {
            return self.inner.publish(page, provenance).await;
        }
        let counted = match self.inner.versions(page.name()).await {
            Ok(versions) => versions.len(),
            Err(PageStoreError::NotFound) => 0,
            Err(other) => return Err(other),
        };
        self.barrier.wait();
        self.inner.publish(page, provenance).await?;
        PageVersion::try_from(format!("v{}", counted + 1).as_str())
            .map_err(|_| PageStoreError::Unavailable)
    }

    async fn latest(&self, name: &PageName) -> Result<PublishedPage, PageStoreError> {
        self.inner.latest(name).await
    }

    async fn at(&self, pin: &PagePin) -> Result<PublishedPage, PageStoreError> {
        self.inner.at(pin).await
    }

    async fn versions(&self, name: &PageName) -> Result<Vec<PageVersion>, PageStoreError> {
        self.inner.versions(name).await
    }

    async fn pages(&self) -> Result<Vec<PageName>, PageStoreError> {
        self.inner.pages().await
    }

    async fn save_binding(
        &self,
        binding: Binding,
        records: &impl RecordStore,
    ) -> Result<(), PageStoreError> {
        if !matches!(self.race, Race::Save) {
            return self.inner.save_binding(binding, records).await;
        }
        let free = matches!(
            self.inner.binding(binding.name()).await,
            Err(PageStoreError::NotFound)
        );
        self.barrier.wait();
        match self.inner.save_binding(binding, records).await {
            Err(PageStoreError::NameTaken) if free => Ok(()),
            other => other,
        }
    }

    async fn binding(&self, name: &BindingName) -> Result<Binding, PageStoreError> {
        self.inner.binding(name).await
    }

    async fn bindings_of(&self, name: &PageName) -> Result<Vec<Binding>, PageStoreError> {
        self.inner.bindings_of(name).await
    }

    async fn check_fit(
        &self,
        name: &BindingName,
        records: &impl RecordStore,
    ) -> Result<(), PageStoreError> {
        self.inner.check_fit(name, records).await
    }

    async fn move_binding(
        &self,
        name: &BindingName,
        expected: &PageVersion,
        version: &PageVersion,
        records: &impl RecordStore,
        mover: &WriteProvenance,
    ) -> Result<(), MoveError> {
        if !matches!(self.race, Race::Move) {
            return self
                .inner
                .move_binding(name, expected, version, records, mover)
                .await;
        }
        let seen = self.inner.binding(name).await?;
        if seen.pin().version() != expected {
            return Err(MoveError::Moved {
                current: seen.pin().version().clone(),
            });
        }
        self.barrier.wait();
        let _one_at_a_time = self
            .serial
            .lock()
            .map_err(|_| PageStoreError::Unavailable)?;
        block_on(async {
            let now = self.inner.binding(name).await?;
            self.inner
                .move_binding(name, now.pin().version(), version, records, mover)
                .await
        })
    }
}

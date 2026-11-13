//! The in-memory [`PageStore`].

use super::locked;
use crate::binding::Binding;
use crate::name::{BindingName, PageName, PageVersion};
use crate::page::{Page, PagePin, PublishedPage};
use crate::provenance::{MadeFrom, Provenance, WriteProvenance};
use crate::store::{FillTypes, MoveError, PageStore, PageStoreError, RecordStore};
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Default)]
pub(super) struct PageState {
    /// The versions of each page, oldest first. A page exists only while it
    /// has a version.
    pub(super) pages: BTreeMap<PageName, Vec<PublishedPage>>,
    pub(super) bindings: BTreeMap<BindingName, Binding>,
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

/// The version name for a page that has `existing` versions.
///
/// Shortcut: versions are named v1, v2 and so on by position. That handles
/// one process that never deletes a version, and the [`PageStore`] contract
/// says no store deletes one or reuses a name. It misses versions that
/// outlive the process and versions made by several writers, and a store
/// backed by Git replaces the names with commit ids.
fn numbered_version(existing: usize) -> Option<PageVersion> {
    PageVersion::try_from(format!("v{}", existing.checked_add(1)?).as_str()).ok()
}

/// A [`PageStore`] that keeps its pages and bindings in memory. A page's
/// versions are named `v1`, `v2` and so on.
#[derive(Default)]
pub struct MemoryPageStore {
    pub(super) state: Mutex<PageState>,
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

    /// Publishes a page, with `name_version` building the version name from
    /// the number of versions the page has. The name is built before the page
    /// entry exists, so a refused name leaves no page behind.
    pub(super) fn publish_with(
        &self,
        page: Page,
        provenance: Provenance,
        name_version: impl FnOnce(usize) -> Option<PageVersion>,
    ) -> Result<PageVersion, PageStoreError> {
        let mut state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        if !state.origin_exists(provenance.made_from()) {
            return Err(PageStoreError::UnresolvedOrigin);
        }
        let existing = state.pages.get(page.name()).map_or(0, Vec::len);
        let version = name_version(existing).ok_or(PageStoreError::Unavailable)?;
        state
            .pages
            .entry(page.name().clone())
            .or_default()
            .push(PublishedPage::new(page, version.clone(), provenance));
        Ok(version)
    }
}

impl PageStore for MemoryPageStore {
    async fn publish(
        &self,
        page: Page,
        provenance: Provenance,
    ) -> Result<PageVersion, PageStoreError> {
        self.publish_with(page, provenance, numbered_version)
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

    async fn check_fit(
        &self,
        name: &BindingName,
        records: &impl RecordStore,
    ) -> Result<(), PageStoreError> {
        let fills = fills_of(self, name)?;
        let types = FillTypes::read(&fills, records).await;
        let state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        let current = state.bindings.get(name).ok_or(PageStoreError::NotFound)?;
        let published = state
            .version_of(current.pin())
            .ok_or(PageStoreError::UnknownVersion)?;
        types.check(published.page())
    }

    async fn move_binding(
        &self,
        name: &BindingName,
        expected: &PageVersion,
        version: &PageVersion,
        records: &impl RecordStore,
        mover: &WriteProvenance,
    ) -> Result<(), MoveError> {
        let fills = fills_of(self, name)?;
        let types = FillTypes::read(&fills, records).await;
        let mut state = locked(&self.state).ok_or(PageStoreError::Unavailable)?;
        let current = state.bindings.get(name).ok_or(PageStoreError::NotFound)?;
        if current.pin().version() != expected {
            return Err(MoveError::Moved {
                current: current.pin().version().clone(),
            });
        }
        let target = PagePin::new(current.pin().name().clone(), version.clone());
        let published = state
            .version_of(&target)
            .ok_or(PageStoreError::UnknownVersion)?;
        types.check(published.page())?;
        let moved = current.moved_to(version.clone(), mover.clone());
        state.bindings.insert(name.clone(), moved);
        Ok(())
    }
}

/// The fills of a stored binding. No operation changes a binding's fills, so
/// the fills read here are the fills that the later locked step checks.
fn fills_of(
    store: &MemoryPageStore,
    name: &BindingName,
) -> Result<crate::page::Fills, PageStoreError> {
    let state = locked(&store.state).ok_or(PageStoreError::Unavailable)?;
    Ok(state
        .bindings
        .get(name)
        .ok_or(PageStoreError::NotFound)?
        .fills()
        .clone())
}

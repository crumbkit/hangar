//! The [`RecordStore`] and [`PageStore`] traits and their errors.
//!
//! Both traits are async, written as methods that return
//! `impl Future<Output = …> + Send`, so this crate needs no runtime and no
//! `async-trait`. No signature names an HTTP, SDK or Board type.

use crate::binding::Binding;
use crate::name::{BindingName, PageName, PageVersion, SlotName};
use crate::page::{Fills, Misfit, Page, PagePin, PublishedPage};
use crate::provenance::{Provenance, WriteProvenance};
use crate::record::{Blob, Record, RecordBody, RecordPath, RecordType};
use std::future::Future;

/// Why a store refused to read a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReadError {
    /// No record exists at the path.
    #[error("no record exists at that path")]
    NotFound,
    /// The store could not answer.
    #[error("the record store is unavailable")]
    Unavailable,
}

/// The text of a schema or guard refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Rejection(String);

impl Rejection {
    /// Wraps the text that explains the refusal.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The text that explains the refusal.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.0
    }
}

/// Why a store refused to write a record.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    /// No record exists at the path.
    #[error("no record exists at that path")]
    NotFound,
    /// The record's blob differs from the blob the write was based on. The
    /// error carries no record; the caller reads the newer one.
    #[error("the record changed since the write was based on it")]
    Conflict,
    /// A schema or guard refused the new body.
    #[error("the write was rejected: {}", .0.text())]
    Rejected(Rejection),
    /// The store could not answer.
    #[error("the record store is unavailable")]
    Unavailable,
}

/// Reads and writes the records of a board.
pub trait RecordStore: Send + Sync {
    /// Reads the record at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ReadError::NotFound`] when no record exists at `path`.
    fn read(&self, path: &RecordPath) -> impl Future<Output = Result<Record, ReadError>> + Send;

    /// Replaces the body of the record at `path`, when the record's current
    /// blob equals the blob that `provenance` names. Returns the new blob.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NotFound`] when no record exists at `path`,
    /// [`WriteError::Conflict`] when the blob differs, and
    /// [`WriteError::Rejected`] when a schema or guard refuses the body.
    fn write(
        &self,
        path: &RecordPath,
        body: &RecordBody,
        provenance: &WriteProvenance,
    ) -> impl Future<Output = Result<Blob, WriteError>> + Send;
}

/// Why a page store refused an operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PageStoreError {
    /// The page or the binding does not exist.
    #[error("no such page or binding")]
    NotFound,
    /// A binding with that name exists.
    #[error("a binding with that name exists")]
    NameTaken,
    /// The pin or the target names a version the page does not have.
    #[error("the page has no such version")]
    UnknownVersion,
    /// `made_from` names a page version or binding that does not exist.
    #[error("the origin names no existing page version or binding")]
    UnresolvedOrigin,
    /// The bound records do not fit the page.
    #[error("the records do not fit the page")]
    Misfit(#[source] Misfit),
    /// The store could not answer.
    #[error("the page store is unavailable")]
    Unavailable,
}

/// What a record store answered for each record that a binding fills.
///
/// Every [`PageStore`] applies the same rule to a binding: it reads the type
/// of each filled record, a record that does not exist is a
/// [`Misfit::MissingRecord`], and [`Page::check_fit`] compares the rest. This
/// type holds that rule once. [`FillTypes::read`] does the asynchronous
/// reads, and [`FillTypes::check`] applies the rule without waiting, so a
/// store can read first and then check and insert in one step under its own
/// lock.
#[derive(Debug, Clone)]
pub struct FillTypes {
    entries: Vec<(SlotName, Fetched)>,
}

#[derive(Debug, Clone)]
enum Fetched {
    Found(RecordType),
    Missing(RecordPath),
    Unavailable,
}

impl FillTypes {
    /// Reads the record at each path of `fills`.
    pub async fn read(fills: &Fills, records: &impl RecordStore) -> Self {
        let mut entries = Vec::new();
        for (slot, path) in fills.iter() {
            let fetched = match records.read(path).await {
                Ok(record) => Fetched::Found(record.record_type().clone()),
                Err(ReadError::NotFound) => Fetched::Missing(path.clone()),
                Err(_) => Fetched::Unavailable,
            };
            entries.push((slot.clone(), fetched));
        }
        Self { entries }
    }

    /// Checks the records that were read against `page`.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::Misfit`] for the first fill, in the order of
    /// the binding, whose record is missing, whose slot the page does not
    /// declare or whose record has another type than its slot. Returns
    /// [`PageStoreError::Unavailable`] when a read failed for any other
    /// reason.
    pub fn check(&self, page: &Page) -> Result<(), PageStoreError> {
        let mut found: Vec<(&SlotName, &RecordType)> = Vec::new();
        for (slot, fetched) in &self.entries {
            match fetched {
                Fetched::Found(record_type) => found.push((slot, record_type)),
                Fetched::Missing(path) => {
                    return Err(PageStoreError::Misfit(Misfit::MissingRecord {
                        slot: slot.clone(),
                        path: path.clone(),
                    }));
                }
                Fetched::Unavailable => return Err(PageStoreError::Unavailable),
            }
        }
        page.check_fit(found).map_err(PageStoreError::Misfit)
    }
}

/// Stores page versions and bindings.
///
/// A store keeps the [`Provenance`] with each page version and each binding
/// and returns it unchanged on every read, because Board's git identity does
/// not say who wrote a record. A store does not resolve a provenance's
/// `sources`, because a source record may live in another board or be
/// removed later.
pub trait PageStore: Send + Sync {
    /// Publishes a new version of a page and returns its version.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::UnresolvedOrigin`] when `made_from` is not
    /// `Original` and names no existing page version or binding.
    fn publish(
        &self,
        page: Page,
        provenance: Provenance,
    ) -> impl Future<Output = Result<PageVersion, PageStoreError>> + Send;

    /// The newest version of a page.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown page.
    fn latest(
        &self,
        name: &PageName,
    ) -> impl Future<Output = Result<PublishedPage, PageStoreError>> + Send;

    /// One version of a page.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown page or version.
    fn at(
        &self,
        pin: &PagePin,
    ) -> impl Future<Output = Result<PublishedPage, PageStoreError>> + Send;

    /// The versions of a page, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown page.
    fn versions(
        &self,
        name: &PageName,
    ) -> impl Future<Output = Result<Vec<PageVersion>, PageStoreError>> + Send;

    /// The names of the published pages.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::Unavailable`] when the store cannot answer.
    fn pages(&self) -> impl Future<Output = Result<Vec<PageName>, PageStoreError>> + Send;

    /// Saves a new binding. The store reads the type of each record the
    /// binding fills from `records` and applies [`Page::check_fit`] against
    /// the pinned version of the page.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NameTaken`] for an existing name,
    /// [`PageStoreError::UnknownVersion`] for a pin that names no version,
    /// [`PageStoreError::UnresolvedOrigin`] when `made_from` names nothing
    /// that exists, and [`PageStoreError::Misfit`] when a filled slot is not
    /// declared, a record's type differs from its slot's type or a bound
    /// record does not exist.
    fn save_binding(
        &self,
        binding: Binding,
        records: &impl RecordStore,
    ) -> impl Future<Output = Result<(), PageStoreError>> + Send;

    /// A binding.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown binding.
    fn binding(
        &self,
        name: &BindingName,
    ) -> impl Future<Output = Result<Binding, PageStoreError>> + Send;

    /// The bindings pinned to any version of a page.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown page.
    fn bindings_of(
        &self,
        name: &PageName,
    ) -> impl Future<Output = Result<Vec<Binding>, PageStoreError>> + Send;

    /// Moves a binding's pin to another version of its page, and changes
    /// nothing else. The target may be older or newer than the pin. The store
    /// applies the same fit check as [`PageStore::save_binding`] against the
    /// target version.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown binding,
    /// [`PageStoreError::UnknownVersion`] for a version the page does not
    /// have and [`PageStoreError::Misfit`] when the bound records do not fit
    /// the target version. The pin is unchanged after an error.
    fn move_binding(
        &self,
        name: &BindingName,
        version: &PageVersion,
        records: &impl RecordStore,
    ) -> impl Future<Output = Result<(), PageStoreError>> + Send;
}

#[cfg(test)]
mod tests;

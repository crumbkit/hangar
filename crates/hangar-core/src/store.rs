//! The [`RecordStore`] and [`PageStore`] traits and their errors.
//!
//! Both traits are async, written as methods that return
//! `impl Future<Output = …> + Send`, so this crate needs no runtime and no
//! `async-trait`. No signature names an HTTP, SDK or Board type.

mod errors;
mod fill_types;

pub use errors::{MoveError, PageStoreError, ReadError, Rejection, WriteError};
pub use fill_types::FillTypes;

use crate::binding::Binding;
use crate::name::{BindingName, PageName, PageVersion};
use crate::page::{Page, PagePin, PublishedPage};
use crate::provenance::{Provenance, WriteProvenance};
use crate::record::{Blob, Record, RecordBody, RecordPath};
use std::future::Future;

/// Reads and writes the records of a board.
///
/// A store never reuses a blob for one record. After a record goes from body
/// A to body B and back to A, its blob is neither the first blob nor the
/// second, so a writer that names an old blob gets
/// [`WriteError::Conflict`] however the bodies compare. A store backed by Git
/// meets this with commit ids.
pub trait RecordStore: Send + Sync {
    /// Reads the record at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ReadError::NotFound`] when no record exists at `path` and
    /// [`ReadError::Unavailable`] when the store cannot answer.
    fn read(&self, path: &RecordPath) -> impl Future<Output = Result<Record, ReadError>> + Send;

    /// Replaces the body of the record at `path`, when the record's current
    /// blob equals the blob that `provenance` names. Returns the new blob.
    ///
    /// When more than one error applies, the store answers the first of
    /// [`WriteError::Unavailable`], [`WriteError::NotFound`],
    /// [`WriteError::Conflict`] and [`WriteError::Rejected`], in that order.
    /// Every error leaves the record unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NotFound`] when no record exists at `path`,
    /// [`WriteError::Conflict`] when the blob differs,
    /// [`WriteError::Rejected`] when the store refuses the body for a reason
    /// of its own and [`WriteError::Unavailable`] when it cannot answer.
    fn write(
        &self,
        path: &RecordPath,
        body: &RecordBody,
        provenance: &WriteProvenance,
    ) -> impl Future<Output = Result<Blob, WriteError>> + Send;
}

/// Stores page versions and bindings.
///
/// A store keeps the [`Provenance`] with each page version and each binding
/// and returns it unchanged on every read, because Board's git identity does
/// not say who wrote a record. A binding also keeps the [`WriteProvenance`]
/// of its last move, beside the provenance of the save. A store does not
/// resolve a provenance's `sources`, because a source record may live in
/// another board or be removed later.
///
/// A page version is never removed, and a store never gives one page the same
/// version name twice, so a pin names the same content for as long as the
/// store exists.
///
/// # The fit check
///
/// [`PageStore::save_binding`] and [`PageStore::move_binding`] check that the
/// bound records fit the page version. A store cannot lock the record store
/// that it reads from, so the check holds for the records as they were read
/// during the call. A record can change type or disappear afterwards, and a
/// stored binding then stops fitting its page. A reader that needs a fit
/// calls [`PageStore::check_fit`], which checks one stored binding against
/// the records as they are now. A store that reads all fills from one
/// revision of its records belongs to a later change.
pub trait PageStore: Send + Sync {
    /// Publishes a new version of a page and returns its version.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::UnresolvedOrigin`] when `made_from` is not
    /// `Original` and names no existing page version or binding, and
    /// [`PageStoreError::Unavailable`] when the store cannot answer. An error
    /// leaves no page and no version behind.
    fn publish(
        &self,
        page: Page,
        provenance: Provenance,
    ) -> impl Future<Output = Result<PageVersion, PageStoreError>> + Send;

    /// The newest version of a page.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown page and
    /// [`PageStoreError::Unavailable`] when the store cannot answer.
    fn latest(
        &self,
        name: &PageName,
    ) -> impl Future<Output = Result<PublishedPage, PageStoreError>> + Send;

    /// One version of a page.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown page or version
    /// and [`PageStoreError::Unavailable`] when the store cannot answer.
    fn at(
        &self,
        pin: &PagePin,
    ) -> impl Future<Output = Result<PublishedPage, PageStoreError>> + Send;

    /// The versions of a page, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown page and
    /// [`PageStoreError::Unavailable`] when the store cannot answer.
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
    /// When more than one error applies, the store answers the first of
    /// [`PageStoreError::Unavailable`], [`PageStoreError::NameTaken`],
    /// [`PageStoreError::UnknownVersion`],
    /// [`PageStoreError::UnresolvedOrigin`] and the fit errors together with
    /// [`PageStoreError::RecordsUnavailable`]. Of those it answers the one
    /// for the first fill in the binding's slot order that fails. An error
    /// saves nothing.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NameTaken`] for an existing name,
    /// [`PageStoreError::UnknownVersion`] for a pin that names no version,
    /// [`PageStoreError::UnresolvedOrigin`] when `made_from` names nothing
    /// that exists, [`PageStoreError::Misfit`] when a filled slot is not
    /// declared, a record's type differs from its slot's type or a bound
    /// record does not exist, [`PageStoreError::RecordsUnavailable`] when
    /// `records` could not answer a read and [`PageStoreError::Unavailable`]
    /// when the page store cannot answer.
    fn save_binding(
        &self,
        binding: Binding,
        records: &impl RecordStore,
    ) -> impl Future<Output = Result<(), PageStoreError>> + Send;

    /// A binding.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown binding and
    /// [`PageStoreError::Unavailable`] when the store cannot answer.
    fn binding(
        &self,
        name: &BindingName,
    ) -> impl Future<Output = Result<Binding, PageStoreError>> + Send;

    /// The bindings pinned to any version of a page, old or new.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown page and
    /// [`PageStoreError::Unavailable`] when the store cannot answer.
    fn bindings_of(
        &self,
        name: &PageName,
    ) -> impl Future<Output = Result<Vec<Binding>, PageStoreError>> + Send;

    /// Checks a stored binding against the records as `records` holds them
    /// now, with the same rule as [`PageStore::save_binding`]. It changes
    /// nothing.
    ///
    /// # Errors
    ///
    /// Returns [`PageStoreError::NotFound`] for an unknown binding,
    /// [`PageStoreError::Misfit`] when a bound record no longer fits its
    /// slot or no longer exists, [`PageStoreError::RecordsUnavailable`] when
    /// `records` could not answer a read and
    /// [`PageStoreError::Unavailable`] when the page store cannot answer. The
    /// version a binding is pinned to always exists, because a page store
    /// never removes a version, so the check does not answer
    /// [`PageStoreError::UnknownVersion`].
    fn check_fit(
        &self,
        name: &BindingName,
        records: &impl RecordStore,
    ) -> impl Future<Output = Result<(), PageStoreError>> + Send;

    /// Moves a binding's pin from the version `expected` to `version`, and
    /// changes nothing else except that the binding keeps `mover` as the
    /// provenance of its last move. The target may be older or newer than the
    /// pin. The store applies the same fit check as
    /// [`PageStore::save_binding`] against the target version. A store does
    /// not compare `mover`'s `based_on` blob with anything, because a page
    /// store holds no record.
    ///
    /// The move happens in one step: when the binding's pin is not
    /// `expected`, the store changes nothing and answers
    /// [`MoveError::Moved`] with the pin the binding has.
    ///
    /// When more than one error applies, the store answers the first of
    /// [`PageStoreError::Unavailable`], [`PageStoreError::NotFound`],
    /// [`MoveError::Moved`], [`PageStoreError::UnknownVersion`] and the fit
    /// errors together with [`PageStoreError::RecordsUnavailable`]. Of the fit
    /// errors and `RecordsUnavailable` it answers the one for the first fill
    /// in slot order that fails.
    /// The pin and the last move are unchanged after an error.
    ///
    /// # Errors
    ///
    /// Returns [`MoveError::Moved`] when the pin differs from `expected`.
    /// Every other refusal arrives as [`MoveError::Refused`]:
    /// [`PageStoreError::NotFound`] for an unknown binding,
    /// [`PageStoreError::UnknownVersion`] for a version the page does not
    /// have, [`PageStoreError::Misfit`] when the bound records do not fit the
    /// target version, [`PageStoreError::RecordsUnavailable`] when `records`
    /// could not answer a read and [`PageStoreError::Unavailable`] when the
    /// page store cannot answer.
    fn move_binding(
        &self,
        name: &BindingName,
        expected: &PageVersion,
        version: &PageVersion,
        records: &impl RecordStore,
        mover: &WriteProvenance,
    ) -> impl Future<Output = Result<(), MoveError>> + Send;
}

#[cfg(test)]
mod tests;

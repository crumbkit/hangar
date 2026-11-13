//! The errors of the [`RecordStore`](super::RecordStore) and
//! [`PageStore`](super::PageStore) traits.

use crate::name::PageVersion;
use crate::page::Misfit;

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
    /// The store refuses this content for a reason of its own, such as a
    /// schema or a guard. The record is unchanged. A store answers
    /// [`WriteError::NotFound`] before it answers `Conflict`, and `Conflict`
    /// before it answers `Rejected`, so a rejection means the write was
    /// based on the current blob of an existing record.
    #[error("the write was rejected: {}", .0.text())]
    Rejected(Rejection),
    /// The store could not answer. The record is unchanged.
    #[error("the record store is unavailable")]
    Unavailable,
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
    /// The record store that the fit check reads from could not answer. The
    /// page store itself is available, and nothing was changed.
    #[error("the record store is unavailable")]
    RecordsUnavailable,
}

/// Why a page store refused to move a binding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MoveError {
    /// The binding is pinned to another version than the caller expected,
    /// so another move or a newer read came first. Nothing changed, and the
    /// caller reads the binding again before it decides what to do.
    #[error("the binding is pinned to {}, not to the version that was expected", .current.as_str())]
    Moved {
        /// The version that the binding is pinned to now.
        current: PageVersion,
    },
    /// The move failed for a reason that a save shares.
    #[error(transparent)]
    Refused(#[from] PageStoreError),
}

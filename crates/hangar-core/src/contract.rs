//! Contract suites that every implementation of a hangar trait runs.
//!
//! Each suite is a set of public async functions. A function takes an
//! implementation and a small fixture, checks one clause of the trait's
//! contract and returns `Result<(), Violation>`. A function returns an error
//! and never panics, because this is library code (BASE-4). The calling test
//! unwraps.
//!
//! Run each function against a new store, seeded with the records that the
//! fixture's `seeds` lists. A later crate calls every function of a suite
//! from its own tests, on its own executor.

use std::fmt;

pub mod authenticator;
pub mod page_store;
pub mod record_store;

/// Defines [`Clause`] with one sentence per clause.
macro_rules! clauses {
    ($($variant:ident => $text:literal,)*) => {
        /// A clause of a trait's contract.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum Clause {
            $(#[doc = $text] $variant,)*
        }

        impl fmt::Display for Clause {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(match self { $(Self::$variant => $text,)* })
            }
        }
    };
}

clauses! {
    ReadReturnsWhatWasSeeded => "a read returns the record that was seeded",
    WriteReturnsANewBlobAndReplacesTheBody => "a write with the current blob returns a new blob and replaces the body",
    AStaleBlobConflictsAndChangesNothing => "a write with a stale blob conflicts and leaves the record unchanged",
    AnOldBlobConflictsAfterAWrite => "a write with the blob from before an earlier write conflicts",
    AnUnknownPathIsNotFound => "a read or a write of an unknown path is not found",
    PublishThenLatestReturnsThePage => "publishing a page then asking for its latest version returns the page",
    RepublishAddsAVersionNewestFirst => "a republish adds a version, versions list newest first and an older version keeps its content",
    PagesListsPublishedNames => "the page list names each published page",
    UnknownNamesAreNotFound => "an unknown page, version or binding is not found",
    ProvenanceIsReturnedUnchanged => "provenance comes back unchanged from every read",
    AnUnresolvedOriginIsRefused => "an origin that names no existing page version or binding is refused and nothing is stored",
    ABindingRoundTrips => "a saved binding reads back with its pin, fills and provenance",
    ATakenNameAndAnUnknownVersionAreRefused => "a taken binding name and an unknown pinned version are refused and change nothing",
    AMisfitIsRefusedOnSave => "a binding whose records do not fit its page is refused and not saved",
    ARepublishLeavesAPinUnchanged => "a republish leaves a binding's pin where it was",
    MoveBindingChangesOnlyThePin => "moving a binding changes only its pin, toward an older or a newer version",
    MoveBindingRefusesAMisfit => "moving a binding to a version its records do not fit is refused and the pin stays",
    MoveBindingRefusesAVanishedRecord => "moving a binding whose bound record no longer exists is refused and the pin stays",
    BindingsOfListsAPagesBindings => "the bindings of a page lists the bindings pinned to that page",
    AcceptedCredentialGivesTheSamePrincipal => "an accepted credential gives the same principal every time",
    RejectedCredentialIsRejected => "a credential the authenticator does not accept is rejected",
    AccessFollowsTheResource => "a reachable resource is allowed and an unreachable one is denied, for read and for write",
    FixtureIsUnusable => "the fixture holds a value that its type refuses",
    TheStoreFailedUnexpectedly => "the store returned an error that no clause expects",
}

/// A clause that an implementation breaks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("contract clause broken: {clause}: {detail}")]
pub struct Violation {
    clause: Clause,
    detail: String,
}

impl Violation {
    pub(crate) fn new(clause: Clause, detail: impl fmt::Debug) -> Self {
        Self {
            clause,
            detail: format!("{detail:?}"),
        }
    }

    /// The clause that failed.
    #[must_use]
    pub fn clause(&self) -> Clause {
        self.clause
    }

    /// What the implementation did instead.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Returns `Ok` when `condition` holds, and a [`Violation`] of `clause`
/// with `detail` when it does not.
pub(crate) fn ensure(
    condition: bool,
    clause: Clause,
    detail: impl FnOnce() -> String,
) -> Result<(), Violation> {
    if condition {
        Ok(())
    } else {
        Err(Violation::new(clause, detail()))
    }
}

/// Turns a refusal by a fixture value into a [`Violation`].
pub(crate) fn fixture<T, E: fmt::Debug>(parsed: Result<T, E>) -> Result<T, Violation> {
    parsed.map_err(|error| Violation::new(Clause::FixtureIsUnusable, error))
}

/// Turns an error that no clause expects into a [`Violation`].
pub(crate) fn unexpected<T, E: fmt::Debug>(result: Result<T, E>) -> Result<T, Violation> {
    result.map_err(|error| Violation::new(Clause::TheStoreFailedUnexpectedly, error))
}

/// A record that a fixture asks the caller to seed into a new record store.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SeedRecord {
    path: crate::RecordPath,
    record_type: crate::RecordType,
    body: crate::RecordBody,
}

impl SeedRecord {
    /// Builds a seed from a path, a type and a body.
    pub(crate) fn new(
        path: crate::RecordPath,
        record_type: crate::RecordType,
        body: crate::RecordBody,
    ) -> Self {
        Self {
            path,
            record_type,
            body,
        }
    }

    /// Where to seed the record.
    #[must_use]
    pub fn path(&self) -> &crate::RecordPath {
        &self.path
    }

    /// The type to seed.
    #[must_use]
    pub fn record_type(&self) -> &crate::RecordType {
        &self.record_type
    }

    /// The body to seed.
    #[must_use]
    pub fn body(&self) -> &crate::RecordBody {
        &self.body
    }
}

/// Builds a seed from text. A fixture's constants are valid, and a refusal
/// is reported as [`Clause::FixtureIsUnusable`].
pub(crate) fn seed(path: &str, record_type: &str, body: &str) -> Result<SeedRecord, Violation> {
    Ok(SeedRecord::new(
        fixture(crate::RecordPath::try_from(path))?,
        fixture(crate::RecordType::try_from(record_type))?,
        fixture(crate::RecordBody::try_from(body))?,
    ))
}

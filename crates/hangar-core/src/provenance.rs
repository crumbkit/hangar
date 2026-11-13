//! Who made a page version, a binding or a record edit, and from what.

use crate::auth::Principal;
use crate::error::{ParseError, Subject};
use crate::name::BindingName;
use crate::page::PagePin;
use crate::record::{Blob, RecordPath};

/// The largest number of sources in one provenance.
const MAX_SOURCES: usize = 64;

/// The path of a board record that something was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct RecordRef(RecordPath);

impl RecordRef {
    /// Names the record at `path`.
    #[must_use]
    pub fn new(path: RecordPath) -> Self {
        Self(path)
    }

    /// The record's path.
    #[must_use]
    pub fn path(&self) -> &RecordPath {
        &self.0
    }
}

/// What a page version or a binding was made from. The maker states it on
/// purpose, so no default exists.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MadeFrom {
    /// A version of an existing page.
    Page(PagePin),
    /// An existing binding.
    Binding(BindingName),
    /// Neither: the maker started from nothing.
    Original,
}

/// Who made something, from what, and out of which records.
///
/// `made_by` grants nothing by itself. A store keeps the provenance with
/// each page version and each binding, and returns it unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Provenance {
    made_by: Principal,
    made_from: MadeFrom,
    sources: Vec<RecordRef>,
}

impl Provenance {
    /// Builds a provenance. `sources` may be empty but is always stated.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::TooMany`] for more than 64 sources.
    pub fn new(
        made_by: Principal,
        made_from: MadeFrom,
        sources: Vec<RecordRef>,
    ) -> Result<Self, ParseError> {
        if sources.len() > MAX_SOURCES {
            return Err(ParseError::TooMany {
                subject: Subject::Sources,
                max: MAX_SOURCES,
            });
        }
        Ok(Self {
            made_by,
            made_from,
            sources,
        })
    }

    /// Who made it.
    #[must_use]
    pub fn made_by(&self) -> &Principal {
        &self.made_by
    }

    /// What it was made from.
    #[must_use]
    pub fn made_from(&self) -> &MadeFrom {
        &self.made_from
    }

    /// The records it was derived from.
    #[must_use]
    pub fn sources(&self) -> &[RecordRef] {
        &self.sources
    }
}

/// Who edited a record, through which binding, and from which version of the
/// record. A store compares `based_on` with the record's current blob.
///
/// The only constructor takes all three parts, so an edit without
/// provenance does not compile:
///
/// ```compile_fail,E0061
/// # use hangar_core::*;
/// # let based_on = Blob::try_from("a".repeat(40).as_str())?;
/// # let principal = Principal::new(PrincipalId::try_from("ada")?);
/// let _ = WriteProvenance::new(based_on, principal);
/// # Ok::<(), ParseError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WriteProvenance {
    based_on: Blob,
    made_by: Principal,
    through: BindingName,
}

impl WriteProvenance {
    /// Builds the provenance of one record edit.
    #[must_use]
    pub fn new(based_on: Blob, made_by: Principal, through: BindingName) -> Self {
        Self {
            based_on,
            made_by,
            through,
        }
    }

    /// The blob the edit was based on.
    #[must_use]
    pub fn based_on(&self) -> &Blob {
        &self.based_on
    }

    /// Who made the edit.
    #[must_use]
    pub fn made_by(&self) -> &Principal {
        &self.made_by
    }

    /// The binding the edit was made through.
    #[must_use]
    pub fn through(&self) -> &BindingName {
        &self.through
    }
}

#[cfg(test)]
mod tests;

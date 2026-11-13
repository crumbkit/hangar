//! Types, traits and contract tests that every hangar crate shares.
//!
//! Every type with a rule is built only by a parsing constructor that
//! returns a typed error, so the rest of a program never holds an unchecked
//! value. The traits are [`Authenticator`], [`PageStore`] and
//! [`RecordStore`]. [`memory`] holds in-memory stores for tests, and
//! [`contract`] holds one suite per trait that each implementation runs from
//! its own tests.

mod auth;
mod binding;
pub mod contract;
mod error;
pub mod memory;
mod name;
mod page;
mod provenance;
mod record;
mod store;

pub use auth::{Access, AuthError, Authenticator, Credential, Decision, Principal, Resource};
pub use binding::Binding;
pub use error::{ParseError, Subject};
pub use name::{BindingName, FieldName, PageName, PageVersion, PrincipalId, SlotName};
pub use page::{
    ContentFormat, Fills, Misfit, Page, PageContent, PagePin, PublishedPage, Sample, Slot, Slots,
};
pub use provenance::{MadeFrom, Provenance, RecordRef, WriteProvenance};
pub use record::{Blob, MAX_RECORD_BYTES, Record, RecordBody, RecordPath, RecordType};
pub use store::{
    FillTypes, MoveError, PageStore, PageStoreError, ReadError, RecordStore, Rejection, WriteError,
};

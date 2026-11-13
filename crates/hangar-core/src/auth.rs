//! Principals, credentials and the [`Authenticator`] trait.

use crate::name::{BindingName, PageName, PrincipalId};
use crate::record::RecordPath;
use secrecy::SecretString;
use std::future::Future;

/// Someone who makes requests.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Principal {
    id: PrincipalId,
}

impl Principal {
    /// Builds a principal from its id.
    #[must_use]
    pub fn new(id: PrincipalId) -> Self {
        Self { id }
    }

    /// The principal's id.
    #[must_use]
    pub fn id(&self) -> &PrincipalId {
        &self.id
    }
}

/// What a request presents to prove who makes it. Each kind holds its secret
/// in a [`SecretString`], whose `Debug` output is a placeholder.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Credential {
    /// A bearer token.
    Bearer(SecretString),
    /// A session identifier.
    Session(SecretString),
}

impl Credential {
    /// Wraps a bearer token.
    #[must_use]
    pub fn bearer(secret: impl Into<String>) -> Self {
        Self::Bearer(SecretString::from(secret.into()))
    }

    /// Wraps a session identifier.
    #[must_use]
    pub fn session(secret: impl Into<String>) -> Self {
        Self::Session(SecretString::from(secret.into()))
    }
}

/// What a principal does to a resource. A match on it outside this crate
/// needs a wildcard arm, so a later kind of access does not break callers:
///
/// ```compile_fail,E0004
/// # use hangar_core::Access;
/// # fn label(access: Access) -> &'static str {
/// match access {
///     Access::Read => "read",
///     Access::Write => "write",
/// }
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Access {
    /// The principal reads the resource.
    Read,
    /// The principal changes the resource.
    Write,
}

/// Something a principal may reach.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Resource {
    /// A page.
    Page(PageName),
    /// A binding. Writing it moves its pin.
    Binding(BindingName),
    /// A board record.
    Record(RecordPath),
}

/// An authenticator's answer to a request for access. A match on it outside
/// this crate needs a wildcard arm, so a later answer does not break callers:
///
/// ```compile_fail,E0004
/// # use hangar_core::Decision;
/// # fn allowed(decision: Decision) -> bool {
/// match decision {
///     Decision::Allowed => true,
///     Decision::Denied => false,
/// }
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Decision {
    /// The principal may do it.
    Allowed,
    /// The principal may not do it.
    Denied,
}

/// Why an authenticator could not answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AuthError {
    /// The credential is not one the authenticator accepts.
    #[error("the credential was rejected")]
    Rejected,
    /// The authenticator could not reach what it needs to decide.
    #[error("the authenticator is unavailable")]
    Unavailable,
}

/// Turns a credential into a principal and decides what the principal may
/// reach.
///
/// The routes that set up a session, such as launch codes and OAuth
/// callbacks, belong to each implementation.
pub trait Authenticator: Send + Sync {
    /// Identifies the principal that a credential proves.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::Rejected`] for a credential the authenticator does
    /// not accept and [`AuthError::Unavailable`] when it cannot decide.
    fn authenticate(
        &self,
        credential: &Credential,
    ) -> impl Future<Output = Result<Principal, AuthError>> + Send;

    /// Decides whether `principal` may do `access` to `resource`.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::Unavailable`] when the authenticator cannot decide.
    fn authorize(
        &self,
        principal: &Principal,
        access: Access,
        resource: &Resource,
    ) -> impl Future<Output = Result<Decision, AuthError>> + Send;
}

#[cfg(test)]
mod tests;

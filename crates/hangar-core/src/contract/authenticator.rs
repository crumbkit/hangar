//! The [`Authenticator`] contract suite.

use super::{Clause, Violation, ensure, unexpected};
use crate::{Access, AuthError, Authenticator, Credential, Decision, Resource};

/// The credentials and resources that the suite uses. The implementation
/// under test supplies them, because only it knows what it accepts.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AuthFixture {
    accepted: Credential,
    rejected: Credential,
    reachable: Resource,
    unreachable: Resource,
}

impl AuthFixture {
    /// Builds a fixture. `accepted` is a credential the implementation
    /// accepts, `rejected` one it rejects, `reachable` a resource that the
    /// accepted credential's principal may read and write, and
    /// `unreachable` one it may neither read nor write.
    #[must_use]
    pub fn new(
        accepted: Credential,
        rejected: Credential,
        reachable: Resource,
        unreachable: Resource,
    ) -> Self {
        Self {
            accepted,
            rejected,
            reachable,
            unreachable,
        }
    }
}

/// An accepted credential returns the same principal each time.
///
/// # Errors
///
/// Returns [`Clause::AcceptedCredentialGivesTheSamePrincipal`] otherwise.
pub async fn accepted_credential_gives_the_same_principal<A: Authenticator>(
    authenticator: &A,
    contract_fixture: &AuthFixture,
) -> Result<(), Violation> {
    let first = unexpected(authenticator.authenticate(&contract_fixture.accepted).await)?;
    let second = unexpected(authenticator.authenticate(&contract_fixture.accepted).await)?;
    ensure(
        first == second,
        Clause::AcceptedCredentialGivesTheSamePrincipal,
        || format!("{first:?} then {second:?}"),
    )
}

/// A rejected credential returns `Rejected`.
///
/// # Errors
///
/// Returns [`Clause::RejectedCredentialIsRejected`] otherwise.
pub async fn rejected_credential_is_rejected<A: Authenticator>(
    authenticator: &A,
    contract_fixture: &AuthFixture,
) -> Result<(), Violation> {
    let answer = authenticator.authenticate(&contract_fixture.rejected).await;
    ensure(
        answer == Err(AuthError::Rejected),
        Clause::RejectedCredentialIsRejected,
        || format!("{answer:?}"),
    )
}

/// The reachable resource is `Allowed` and the unreachable one `Denied`,
/// for read and for write.
///
/// # Errors
///
/// Returns [`Clause::AccessFollowsTheResource`] otherwise.
pub async fn access_follows_the_resource<A: Authenticator>(
    authenticator: &A,
    contract_fixture: &AuthFixture,
) -> Result<(), Violation> {
    let principal = unexpected(authenticator.authenticate(&contract_fixture.accepted).await)?;
    for access in [Access::Read, Access::Write] {
        let reachable = unexpected(
            authenticator
                .authorize(&principal, access, &contract_fixture.reachable)
                .await,
        )?;
        let unreachable = unexpected(
            authenticator
                .authorize(&principal, access, &contract_fixture.unreachable)
                .await,
        )?;
        ensure(
            reachable == Decision::Allowed && unreachable == Decision::Denied,
            Clause::AccessFollowsTheResource,
            || format!("{access:?}: reachable {reachable:?}, unreachable {unreachable:?}"),
        )?;
    }
    Ok(())
}

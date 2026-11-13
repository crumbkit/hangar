//! An authenticator with a fixed table, for the Authenticator suite.

use hangar_core::{
    Access, AuthError, Authenticator, Credential, Decision, Principal, PrincipalId, Resource,
};
use secrecy::ExposeSecret;

/// Accepts one secret, answers with one principal and lets that principal
/// reach one resource.
pub(crate) struct FixedAuthenticator {
    secret: &'static str,
    principal: Principal,
    reachable: Resource,
}

impl FixedAuthenticator {
    pub(crate) fn new(secret: &'static str, principal_id: &str, reachable: Resource) -> Self {
        let id = PrincipalId::try_from(principal_id).expect("a valid principal id");
        Self {
            secret,
            principal: Principal::new(id),
            reachable,
        }
    }
}

impl Authenticator for FixedAuthenticator {
    async fn authenticate(&self, credential: &Credential) -> Result<Principal, AuthError> {
        match credential {
            Credential::Bearer(secret) | Credential::Session(secret)
                if secret.expose_secret() == self.secret =>
            {
                Ok(self.principal.clone())
            }
            _ => Err(AuthError::Rejected),
        }
    }

    async fn authorize(
        &self,
        principal: &Principal,
        _access: Access,
        resource: &Resource,
    ) -> Result<Decision, AuthError> {
        if *principal == self.principal && *resource == self.reachable {
            Ok(Decision::Allowed)
        } else {
            Ok(Decision::Denied)
        }
    }
}

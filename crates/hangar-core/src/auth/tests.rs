//! Unit tests of credentials and principals.

use super::*;

#[test]
fn a_credential_debug_output_hides_the_secret() {
    for credential in [
        Credential::bearer("hunter2-secret-token"),
        Credential::session("hunter2-secret-token"),
    ] {
        let shown = format!("{credential:?}");
        assert!(!shown.contains("hunter2"), "{shown}");
    }
}

#[test]
fn a_credential_remembers_which_kind_it_is() {
    assert!(matches!(Credential::bearer("x"), Credential::Bearer(_)));
    assert!(matches!(Credential::session("x"), Credential::Session(_)));
}

#[test]
fn a_principal_exposes_its_id() {
    let id = PrincipalId::try_from("ada").unwrap();
    assert_eq!(Principal::new(id.clone()).id(), &id);
}

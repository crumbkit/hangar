//! Runs each contract suite against a deliberately wrong implementation and
//! asserts the clause that fails, so a suite that stops checking a clause
//! fails these tests.
#![cfg(test)]

#[path = "support/block_on.rs"]
mod block_on;
#[path = "support/fixed_authenticator.rs"]
mod fixed_authenticator;
#[path = "support/flawed_stores.rs"]
mod flawed_stores;

use block_on::block_on;
use fixed_authenticator::FixedAuthenticator;
use flawed_stores::{
    AllowsEverything, Flaw, FlawedPageStore, IgnoresExpectedBlob, PrefixedVersions,
};
use hangar_core::contract::authenticator::{self, AuthFixture};
use hangar_core::contract::page_store::{self, PageFixture};
use hangar_core::contract::record_store::{self, RecordFixture};
use hangar_core::contract::{Clause, SeedRecord};
use hangar_core::memory::{MemoryPageStore, MemoryRecordStore};
use hangar_core::{Credential, PageName, Resource};
use std::sync::Arc;

fn seeded_records(seeds: &[SeedRecord]) -> MemoryRecordStore {
    let store = MemoryRecordStore::new();
    for seed in seeds {
        store
            .seed(
                seed.path().clone(),
                seed.record_type().clone(),
                seed.body().clone(),
            )
            .unwrap();
    }
    store
}

#[test]
fn a_record_store_that_ignores_the_expected_blob_fails_the_stale_blob_clause() {
    let fixture = RecordFixture::standard().unwrap();
    let store = IgnoresExpectedBlob(seeded_records(fixture.seeds()));
    let violation = block_on(record_store::a_stale_blob_conflicts_and_changes_nothing(
        &store, &fixture,
    ))
    .unwrap_err();
    assert_eq!(
        violation.clause(),
        Clause::AStaleBlobConflictsAndChangesNothing
    );
}

#[test]
fn a_record_store_that_ignores_the_expected_blob_fails_the_old_blob_clause() {
    let fixture = RecordFixture::standard().unwrap();
    let store = IgnoresExpectedBlob(seeded_records(fixture.seeds()));
    let violation = block_on(record_store::an_old_blob_conflicts_after_a_write(
        &store, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::AnOldBlobConflictsAfterAWrite);
}

fn flawed(flaw: Flaw, fixture: &PageFixture) -> (FlawedPageStore, Arc<MemoryRecordStore>) {
    let records = Arc::new(seeded_records(fixture.seeds()));
    (FlawedPageStore::new(flaw, Arc::clone(&records)), records)
}

#[test]
fn a_page_store_that_drops_provenance_fails_the_provenance_clause() {
    let fixture = PageFixture::standard().unwrap();
    let (pages, records) = flawed(Flaw::DropsProvenance, &fixture);
    let violation = block_on(page_store::provenance_is_returned_unchanged(
        &pages, &*records, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::ProvenanceIsReturnedUnchanged);
}

#[test]
fn a_page_store_that_accepts_an_unresolved_origin_fails_the_origin_clause() {
    let fixture = PageFixture::standard().unwrap();
    let (pages, records) = flawed(Flaw::AcceptsUnresolvedOrigin, &fixture);
    let violation = block_on(page_store::an_unresolved_origin_is_refused(
        &pages, &*records, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::AnUnresolvedOriginIsRefused);
}

#[test]
fn a_page_store_that_moves_pins_on_republish_fails_the_republish_clause() {
    let fixture = PageFixture::standard().unwrap();
    let (pages, records) = flawed(Flaw::MovesPinsOnRepublish, &fixture);
    let violation = block_on(page_store::a_republish_leaves_a_pin_unchanged(
        &pages, &*records, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::ARepublishLeavesAPinUnchanged);
}

#[test]
fn a_page_store_that_accepts_an_unresolved_page_origin_on_save_fails_the_origin_clause() {
    let fixture = PageFixture::standard().unwrap();
    let (pages, records) = flawed(Flaw::AcceptsUnresolvedPageOriginOnSave, &fixture);
    let violation = block_on(page_store::an_unresolved_origin_is_refused(
        &pages, &*records, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::AnUnresolvedOriginIsRefused);
    assert!(
        violation.detail().contains("save from a missing version"),
        "{}",
        violation.detail()
    );
}

#[test]
fn a_page_store_that_saves_without_the_type_check_fails_the_save_clause_on_the_wrong_type() {
    let fixture = PageFixture::standard().unwrap();
    let (pages, records) = flawed(Flaw::SkipsTypeCheckOnSave, &fixture);
    let violation = block_on(page_store::a_misfit_is_refused_on_save(
        &pages, &*records, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::AMisfitIsRefusedOnSave);
    assert!(
        violation.detail().contains("wrong-view"),
        "{}",
        violation.detail()
    );
}

#[test]
fn a_page_store_that_moves_without_the_type_check_fails_the_move_clause_on_the_wrong_type() {
    let fixture = PageFixture::standard().unwrap();
    let (pages, records) = flawed(Flaw::SkipsTypeCheckOnMove, &fixture);
    let violation = block_on(page_store::move_binding_refuses_a_misfit(
        &pages, &*records, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::MoveBindingRefusesAMisfit);
    assert!(
        violation.detail().contains("another type"),
        "{}",
        violation.detail()
    );
}

#[test]
fn a_page_store_that_moves_without_the_existence_check_fails_the_vanished_record_clause() {
    let fixture = PageFixture::standard().unwrap();
    let (pages, records) = flawed(Flaw::SkipsExistenceCheckOnMove, &fixture);
    let without_records = MemoryRecordStore::new();
    let violation = block_on(page_store::move_binding_refuses_a_vanished_record(
        &pages,
        &*records,
        &without_records,
        &fixture,
    ))
    .unwrap_err();
    assert_eq!(
        violation.clause(),
        Clause::MoveBindingRefusesAVanishedRecord
    );
}

#[test]
fn the_bindings_of_clause_passes_on_a_store_whose_versions_are_not_numbered() {
    let fixture = PageFixture::standard().unwrap();
    let records = seeded_records(fixture.seeds());
    let pages = PrefixedVersions(MemoryPageStore::new());
    block_on(page_store::bindings_of_lists_a_pages_bindings(
        &pages, &records, &fixture,
    ))
    .unwrap();
}

#[test]
fn an_authenticator_that_allows_everything_fails_the_access_clause() {
    let reachable = Resource::Page(PageName::try_from("home").unwrap());
    let unreachable = Resource::Page(PageName::try_from("secret").unwrap());
    let authenticator_under_test = AllowsEverything(FixedAuthenticator::new(
        "right-secret",
        "ada",
        reachable.clone(),
    ));
    let fixture = AuthFixture::new(
        Credential::bearer("right-secret"),
        Credential::bearer("wrong-secret"),
        reachable,
        unreachable,
    );
    let violation = block_on(authenticator::access_follows_the_resource(
        &authenticator_under_test,
        &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::AccessFollowsTheResource);
}

#[test]
fn an_authenticator_that_accepts_every_credential_fails_the_rejection_clause() {
    let reachable = Resource::Page(PageName::try_from("home").unwrap());
    let unreachable = Resource::Page(PageName::try_from("secret").unwrap());
    // The "rejected" credential in this fixture is the secret the double accepts.
    let accepting = FixedAuthenticator::new("wrong-secret", "ada", reachable.clone());
    let fixture = AuthFixture::new(
        Credential::bearer("wrong-secret"),
        Credential::bearer("wrong-secret"),
        reachable,
        unreachable,
    );
    let violation = block_on(authenticator::rejected_credential_is_rejected(
        &accepting, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::RejectedCredentialIsRejected);
}

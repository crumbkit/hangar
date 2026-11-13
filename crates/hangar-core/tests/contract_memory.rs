//! Runs every contract suite against the in-memory stores and the fixed-table
//! authenticator, so each suite is known to pass on a correct implementation.
#![cfg(test)]

#[path = "support/block_on.rs"]
mod block_on;
#[path = "support/fixed_authenticator.rs"]
mod fixed_authenticator;

use block_on::block_on;
use fixed_authenticator::FixedAuthenticator;
use hangar_core::contract::authenticator::{self, AuthFixture};
use hangar_core::contract::page_store::{self, PageFixture};
use hangar_core::contract::record_store::{self, RecordFixture};
use hangar_core::memory::{MemoryPageStore, MemoryRecordStore};
use hangar_core::{
    BindingName, Credential, PageName, Principal, PrincipalId, RecordPath, RecordStore, Resource,
    WriteProvenance,
};

fn seeded_records(seeds: &[hangar_core::contract::SeedRecord]) -> MemoryRecordStore {
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

macro_rules! record_suite {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() {
            let fixture = RecordFixture::standard().unwrap();
            let store = seeded_records(fixture.seeds());
            block_on(record_store::$name(&store, &fixture)).unwrap();
        }
    )*};
}

record_suite!(
    read_returns_what_was_seeded,
    write_returns_a_new_blob_and_replaces_the_body,
    a_stale_blob_conflicts_and_changes_nothing,
    an_old_blob_conflicts_after_a_write,
    an_unknown_path_is_not_found,
);

macro_rules! page_suite {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() {
            let fixture = PageFixture::standard().unwrap();
            let records = seeded_records(fixture.seeds());
            let pages = MemoryPageStore::new();
            block_on(page_store::$name(&pages, &records, &fixture)).unwrap();
        }
    )*};
}

page_suite!(
    publish_then_latest_returns_the_page,
    republish_adds_a_version_newest_first,
    pages_lists_published_names,
    unknown_names_are_not_found,
    provenance_is_returned_unchanged,
    an_unresolved_origin_is_refused,
    a_binding_round_trips,
    a_taken_name_and_an_unknown_version_are_refused,
    a_misfit_is_refused_on_save,
    a_republish_leaves_a_pin_unchanged,
    move_binding_changes_only_the_pin,
    move_binding_refuses_a_misfit,
    bindings_of_lists_a_pages_bindings,
);

#[test]
fn move_binding_refuses_a_vanished_record() {
    let fixture = PageFixture::standard().unwrap();
    let records = seeded_records(fixture.seeds());
    let without_records = MemoryRecordStore::new();
    let pages = MemoryPageStore::new();
    block_on(page_store::move_binding_refuses_a_vanished_record(
        &pages,
        &records,
        &without_records,
        &fixture,
    ))
    .unwrap();
}

fn authenticator_fixture() -> (FixedAuthenticator, AuthFixture) {
    let reachable = Resource::Page(PageName::try_from("home").unwrap());
    let unreachable = Resource::Page(PageName::try_from("secret").unwrap());
    let fixed = FixedAuthenticator::new("right-secret", "ada", reachable.clone());
    let fixture = AuthFixture::new(
        Credential::bearer("right-secret"),
        Credential::bearer("wrong-secret"),
        reachable,
        unreachable,
    );
    (fixed, fixture)
}

macro_rules! authenticator_suite {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() {
            let (fixed, fixture) = authenticator_fixture();
            block_on(authenticator::$name(&fixed, &fixture)).unwrap();
        }
    )*};
}

authenticator_suite!(
    accepted_credential_gives_the_same_principal,
    rejected_credential_is_rejected,
    access_follows_the_resource,
);

fn write_provenance(based_on: hangar_core::Blob) -> WriteProvenance {
    WriteProvenance::new(
        based_on,
        Principal::new(PrincipalId::try_from("ada").unwrap()),
        BindingName::try_from("my-view").unwrap(),
    )
}

#[test]
fn the_memory_record_store_keeps_the_write_provenance_of_the_last_write() {
    let fixture = RecordFixture::standard().unwrap();
    let store = seeded_records(fixture.seeds());
    let seed = fixture.seeds().first().unwrap();
    let path: &RecordPath = seed.path();
    let current = block_on(store.read(path)).unwrap();
    assert_eq!(store.last_write(path), None);

    let provenance = write_provenance(current.blob().clone());
    block_on(store.write(path, seed.body(), &provenance)).unwrap();
    assert_eq!(store.last_write(path), Some(provenance));
}

#[test]
fn a_memory_record_store_hands_out_a_new_blob_for_every_write() {
    let fixture = RecordFixture::standard().unwrap();
    let store = seeded_records(fixture.seeds());
    let seed = fixture.seeds().first().unwrap();
    let first = block_on(store.read(seed.path())).unwrap();
    let second_blob = block_on(store.write(
        seed.path(),
        seed.body(),
        &write_provenance(first.blob().clone()),
    ))
    .unwrap();
    assert_ne!(&second_blob, first.blob());
}

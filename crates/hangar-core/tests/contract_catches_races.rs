//! Runs the concurrency clauses and the record write clauses against stores
//! that break them, and asserts the clause that fails.
#![cfg(test)]

#[path = "support/racy_stores.rs"]
mod racy_stores;
#[path = "support/record_flaws.rs"]
mod record_flaws;

use hangar_core::contract::page_store::{self, PageFixture};
use hangar_core::contract::record_store::{self, RecordFixture};
use hangar_core::contract::{Clause, SeedRecord, Threads, block_on};
use hangar_core::memory::MemoryRecordStore;
use racy_stores::{Race, RacyPageStore, RacyRecordWrites};
use record_flaws::{ContentAddressed, RejectsAndWrites, RejectsEveryWrite};

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
fn a_record_store_that_compares_then_writes_fails_the_concurrent_write_clause() {
    let fixture = RecordFixture::standard().unwrap();
    let store = RacyRecordWrites::new(seeded_records(fixture.seeds()));
    let violation = block_on(
        record_store::concurrent_writes_from_one_blob_give_one_conflict(&store, &fixture, &Threads),
    )
    .unwrap_err();
    assert_eq!(
        violation.clause(),
        Clause::ConcurrentWritesFromOneBlobGiveOneConflict
    );
}

#[test]
fn a_page_store_that_names_versions_from_an_earlier_count_fails_the_concurrent_publish_clause() {
    let fixture = PageFixture::standard().unwrap();
    let records = seeded_records(fixture.seeds());
    let pages = RacyPageStore::new(Race::Publish);
    let violation = block_on(page_store::concurrent_publishes_get_two_versions(
        &pages, &records, &fixture, &Threads,
    ))
    .unwrap_err();
    assert_eq!(
        violation.clause(),
        Clause::ConcurrentPublishesGetTwoVersions
    );
}

#[test]
fn a_page_store_that_trusts_an_earlier_name_check_fails_the_concurrent_save_clause() {
    let fixture = PageFixture::standard().unwrap();
    let records = seeded_records(fixture.seeds());
    let pages = RacyPageStore::new(Race::Save);
    let violation = block_on(
        page_store::concurrent_saves_of_one_name_give_one_name_taken(
            &pages, &records, &fixture, &Threads,
        ),
    )
    .unwrap_err();
    assert_eq!(
        violation.clause(),
        Clause::ConcurrentSavesOfOneNameGiveOneNameTaken
    );
}

#[test]
fn a_page_store_that_moves_from_whatever_pin_it_finds_fails_the_concurrent_move_clause() {
    let fixture = PageFixture::standard().unwrap();
    let records = seeded_records(fixture.seeds());
    let pages = RacyPageStore::new(Race::Move);
    let violation = block_on(page_store::concurrent_moves_from_one_pin_give_one_moved(
        &pages, &records, &fixture, &Threads,
    ))
    .unwrap_err();
    assert_eq!(
        violation.clause(),
        Clause::ConcurrentMovesFromOnePinGiveOneMoved
    );
}

#[test]
fn a_record_store_whose_blob_is_a_content_hash_fails_the_blob_clause() {
    let fixture = RecordFixture::standard().unwrap();
    let store = ContentAddressed::default();
    for seed in fixture.seeds() {
        store
            .seed(
                seed.path().clone(),
                seed.record_type().clone(),
                seed.body().clone(),
            )
            .unwrap();
    }
    let violation = block_on(record_store::a_blob_never_repeats_for_one_record(
        &store, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::ABlobNeverRepeatsForOneRecord);
}

#[test]
fn a_record_store_that_rejects_and_writes_fails_the_rejected_write_clause() {
    let fixture = RecordFixture::standard().unwrap();
    let store = RejectsAndWrites(seeded_records(fixture.seeds()));
    let violation = block_on(record_store::a_rejected_write_changes_nothing(
        &store, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::ARejectedWriteChangesNothing);
}

#[test]
fn a_record_store_that_rejects_before_it_compares_fails_the_rejected_write_clause() {
    let fixture = RecordFixture::standard().unwrap();
    let store = RejectsEveryWrite(seeded_records(fixture.seeds()));
    let violation = block_on(record_store::a_rejected_write_changes_nothing(
        &store, &fixture,
    ))
    .unwrap_err();
    assert_eq!(violation.clause(), Clause::ARejectedWriteChangesNothing);
}

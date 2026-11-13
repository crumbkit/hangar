//! Unit tests of the shared fit rule.

use super::{FillTypes, PageStoreError, ReadError, RecordStore, WriteError};
use crate::contract::block_on;
use crate::memory::MemoryRecordStore;
use crate::name::{PageName, SlotName};
use crate::page::{ContentFormat, Fills, Misfit, Page, PageContent, Slot, Slots};
use crate::provenance::WriteProvenance;
use crate::record::{Blob, Record, RecordBody, RecordPath, RecordType};

fn record_type(text: &str) -> RecordType {
    RecordType::try_from(text).unwrap()
}

fn path(text: &str) -> RecordPath {
    RecordPath::try_from(text).unwrap()
}

fn slot(text: &str) -> SlotName {
    SlotName::try_from(text).unwrap()
}

/// A page with one slot, `tasks`, that accepts `tasks/task/v2`.
fn page() -> Page {
    let slots = Slots::new(vec![
        Slot::new(
            slot("tasks"),
            record_type("tasks/task/v2"),
            Vec::new(),
            None,
        )
        .unwrap(),
    ])
    .unwrap();
    Page::new(
        PageName::try_from("home").unwrap(),
        PageContent::new(ContentFormat::Html, "<p>hi</p>").unwrap(),
        slots,
    )
}

fn fills(slot_name: &str, record_path: &str) -> Fills {
    Fills::new(vec![(slot(slot_name), path(record_path))]).unwrap()
}

fn records() -> MemoryRecordStore {
    let store = MemoryRecordStore::new();
    for (record_path, type_text) in [("/tasks/a", "tasks/task/v2"), ("/notes/n", "notes/note/v1")] {
        store
            .seed(
                path(record_path),
                record_type(type_text),
                RecordBody::try_from("x: 1\n").unwrap(),
            )
            .unwrap();
    }
    store
}

/// A record store that cannot answer.
struct Down;

impl RecordStore for Down {
    async fn read(&self, _path: &RecordPath) -> Result<Record, ReadError> {
        Err(ReadError::Unavailable)
    }

    async fn write(
        &self,
        _path: &RecordPath,
        _body: &RecordBody,
        _provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        Err(WriteError::Unavailable)
    }
}

#[test]
fn fill_types_accept_records_of_the_slot_type() {
    let types = block_on(FillTypes::read(&fills("tasks", "/tasks/a"), &records()));
    assert_eq!(types.check(&page()), Ok(()));
}

#[test]
fn fill_types_report_a_missing_record_as_a_misfit() {
    let types = block_on(FillTypes::read(&fills("tasks", "/tasks/gone"), &records()));
    assert_eq!(
        types.check(&page()),
        Err(PageStoreError::Misfit(Misfit::MissingRecord {
            slot: slot("tasks"),
            path: path("/tasks/gone"),
        }))
    );
}

#[test]
fn fill_types_report_a_record_of_another_type_as_a_misfit() {
    let types = block_on(FillTypes::read(&fills("tasks", "/notes/n"), &records()));
    assert_eq!(
        types.check(&page()),
        Err(PageStoreError::Misfit(Misfit::WrongType {
            slot: slot("tasks"),
            expected: record_type("tasks/task/v2"),
            found: record_type("notes/note/v1"),
        }))
    );
}

#[test]
fn fill_types_report_an_undeclared_slot_as_a_misfit() {
    let types = block_on(FillTypes::read(&fills("gone", "/tasks/a"), &records()));
    assert_eq!(
        types.check(&page()),
        Err(PageStoreError::Misfit(Misfit::UndeclaredSlot {
            slot: slot("gone")
        }))
    );
}

#[test]
fn fill_types_report_an_unavailable_record_store_as_records_unavailable() {
    let types = block_on(FillTypes::read(&fills("tasks", "/tasks/a"), &Down));
    assert_eq!(
        types.check(&page()),
        Err(PageStoreError::RecordsUnavailable)
    );
}

/// A page with the slots `a` and `b`, which both accept `tasks/task/v2`.
fn page_with_two_slots() -> Page {
    let slots = Slots::new(vec![
        Slot::new(slot("a"), record_type("tasks/task/v2"), Vec::new(), None).unwrap(),
        Slot::new(slot("b"), record_type("tasks/task/v2"), Vec::new(), None).unwrap(),
    ])
    .unwrap();
    Page::new(
        PageName::try_from("home").unwrap(),
        PageContent::new(ContentFormat::Html, "<p>hi</p>").unwrap(),
        slots,
    )
}

fn two_fills(path_a: &str, path_b: &str) -> Fills {
    Fills::new(vec![(slot("a"), path(path_a)), (slot("b"), path(path_b))]).unwrap()
}

/// A record store that cannot read one path and reads the rest from the
/// store it wraps.
struct DownAt(RecordPath, MemoryRecordStore);

impl RecordStore for DownAt {
    async fn read(&self, path: &RecordPath) -> Result<Record, ReadError> {
        if *path == self.0 {
            Err(ReadError::Unavailable)
        } else {
            self.1.read(path).await
        }
    }

    async fn write(
        &self,
        _path: &RecordPath,
        _body: &RecordBody,
        _provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        Err(WriteError::Unavailable)
    }
}

#[test]
fn fill_types_report_the_first_failure_in_slot_order_when_a_later_record_is_missing() {
    let fills = two_fills("/notes/n", "/tasks/gone");
    let types = block_on(FillTypes::read(&fills, &records()));
    assert_eq!(
        types.check(&page_with_two_slots()),
        Err(PageStoreError::Misfit(Misfit::WrongType {
            slot: slot("a"),
            expected: record_type("tasks/task/v2"),
            found: record_type("notes/note/v1"),
        }))
    );
}

#[test]
fn fill_types_report_a_missing_record_in_an_earlier_slot_before_a_wrong_type_in_a_later_one() {
    let fills = two_fills("/tasks/gone", "/notes/n");
    let types = block_on(FillTypes::read(&fills, &records()));
    assert_eq!(
        types.check(&page_with_two_slots()),
        Err(PageStoreError::Misfit(Misfit::MissingRecord {
            slot: slot("a"),
            path: path("/tasks/gone"),
        }))
    );
}

#[test]
fn fill_types_report_a_mismatch_in_an_earlier_slot_before_an_unavailable_read_in_a_later_one() {
    let fills = two_fills("/notes/n", "/tasks/a");
    let store = DownAt(path("/tasks/a"), records());
    let types = block_on(FillTypes::read(&fills, &store));
    assert_eq!(
        types.check(&page_with_two_slots()),
        Err(PageStoreError::Misfit(Misfit::WrongType {
            slot: slot("a"),
            expected: record_type("tasks/task/v2"),
            found: record_type("notes/note/v1"),
        }))
    );
}

#[test]
fn fill_types_report_an_undeclared_slot_when_its_record_read_failed() {
    let fills = Fills::new(vec![(slot("gone"), path("/tasks/a"))]).unwrap();
    let store = DownAt(path("/tasks/a"), records());
    let types = block_on(FillTypes::read(&fills, &store));
    assert_eq!(
        types.check(&page()),
        Err(PageStoreError::Misfit(Misfit::UndeclaredSlot {
            slot: slot("gone")
        }))
    );
}

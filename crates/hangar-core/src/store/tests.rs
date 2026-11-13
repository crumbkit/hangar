//! Unit tests of the shared fit rule.

use super::*;
use crate::memory::MemoryRecordStore;
use crate::page::{ContentFormat, PageContent, Slot, Slots};
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::yield_now();
    }
}

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
fn fill_types_report_an_unavailable_record_store_as_unavailable() {
    let types = block_on(FillTypes::read(&fills("tasks", "/tasks/a"), &Down));
    assert_eq!(types.check(&page()), Err(PageStoreError::Unavailable));
}

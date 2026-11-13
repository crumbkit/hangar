//! Unit tests of the in-memory stores: a poisoned lock and the order of a
//! publish.

use super::{MemoryPageStore, MemoryRecordStore};
use crate::auth::Principal;
use crate::binding::Binding;
use crate::contract::block_on;
use crate::name::{BindingName, PageName, PageVersion, PrincipalId, SlotName};
use crate::page::{ContentFormat, Fills, Page, PageContent, PagePin, Slot, Slots};
use crate::provenance::{MadeFrom, Provenance, WriteProvenance};
use crate::record::{Blob, RecordBody, RecordPath, RecordType};
use crate::store::{MoveError, PageStore, PageStoreError, ReadError, RecordStore, WriteError};
use std::sync::{Mutex, PoisonError};

fn poison<T: Send>(lock: &Mutex<T>) {
    std::thread::scope(|scope| {
        let _ = scope
            .spawn(|| {
                let _guard = lock.lock().unwrap();
                panic!("poison the lock");
            })
            .join();
    });
}

fn principal() -> Principal {
    Principal::new(PrincipalId::try_from("ada").unwrap())
}

fn path() -> RecordPath {
    RecordPath::try_from("/tasks/a").unwrap()
}

fn blob() -> Blob {
    Blob::try_from("0".repeat(40).as_str()).unwrap()
}

fn write_provenance() -> WriteProvenance {
    WriteProvenance::new(blob(), principal(), BindingName::try_from("view").unwrap())
}

fn page_name() -> PageName {
    PageName::try_from("home").unwrap()
}

fn version(text: &str) -> PageVersion {
    PageVersion::try_from(text).unwrap()
}

fn page() -> Page {
    let slot = Slot::new(
        SlotName::try_from("tasks").unwrap(),
        RecordType::try_from("tasks/task/v2").unwrap(),
        Vec::new(),
        None,
    )
    .unwrap();
    Page::new(
        page_name(),
        PageContent::new(ContentFormat::Html, "<p>hi</p>").unwrap(),
        Slots::new(vec![slot]).unwrap(),
    )
}

fn original() -> Provenance {
    Provenance::new(principal(), MadeFrom::Original, Vec::new()).unwrap()
}

fn binding() -> Binding {
    Binding::new(
        BindingName::try_from("my-view").unwrap(),
        PagePin::new(page_name(), version("v1")),
        Fills::new(vec![(SlotName::try_from("tasks").unwrap(), path())]).unwrap(),
        original(),
    )
}

#[test]
fn a_record_store_with_a_poisoned_lock_answers_unavailable_and_changes_nothing() {
    let store = MemoryRecordStore::new();
    let seeded = store
        .seed(
            path(),
            RecordType::try_from("tasks/task/v2").unwrap(),
            RecordBody::try_from("title: first\n").unwrap(),
        )
        .unwrap();
    poison(&store.state);
    let body = RecordBody::try_from("title: second\n").unwrap();
    let based_on = WriteProvenance::new(
        seeded.clone(),
        principal(),
        BindingName::try_from("view").unwrap(),
    );

    assert_eq!(block_on(store.read(&path())), Err(ReadError::Unavailable));
    assert_eq!(
        block_on(store.write(&path(), &body, &based_on)),
        Err(WriteError::Unavailable)
    );
    assert_eq!(store.last_write(&path()), Err(WriteError::Unavailable));
    assert_eq!(
        store.seed(
            path(),
            RecordType::try_from("notes/note/v1").unwrap(),
            body.clone()
        ),
        Err(WriteError::Unavailable)
    );
    let state = store.state.lock().unwrap_or_else(PoisonError::into_inner);
    let entry = state.records.get(&path()).unwrap();
    assert_eq!(state.records.len(), 1);
    assert_eq!(
        (
            entry.blob.clone(),
            entry.body.as_str(),
            entry.last_write.clone()
        ),
        (seeded, "title: first\n", None)
    );
}

#[test]
fn a_page_store_with_a_poisoned_lock_answers_unavailable_from_every_method_and_changes_nothing() {
    let store = MemoryPageStore::new();
    let records = MemoryRecordStore::new();
    records
        .seed(
            path(),
            RecordType::try_from("tasks/task/v2").unwrap(),
            RecordBody::try_from("title: first\n").unwrap(),
        )
        .unwrap();
    let name = page_name();
    let pin = PagePin::new(name.clone(), version("v1"));
    let view = BindingName::try_from("my-view").unwrap();
    block_on(store.publish(page(), original())).unwrap();
    block_on(store.save_binding(binding(), &records)).unwrap();
    poison(&store.state);
    let unavailable = || PageStoreError::Unavailable;

    assert_eq!(
        block_on(store.publish(page(), original())),
        Err(unavailable())
    );
    assert_eq!(block_on(store.latest(&name)).err(), Some(unavailable()));
    assert_eq!(block_on(store.at(&pin)).err(), Some(unavailable()));
    assert_eq!(block_on(store.versions(&name)).err(), Some(unavailable()));
    assert_eq!(block_on(store.pages()).err(), Some(unavailable()));
    assert_eq!(
        block_on(store.save_binding(binding(), &records)),
        Err(unavailable())
    );
    assert_eq!(block_on(store.binding(&view)).err(), Some(unavailable()));
    assert_eq!(
        block_on(store.bindings_of(&name)).err(),
        Some(unavailable())
    );
    assert_eq!(
        block_on(store.check_fit(&view, &records)),
        Err(unavailable())
    );
    assert_eq!(
        block_on(store.move_binding(
            &view,
            &version("v1"),
            &version("v2"),
            &records,
            &write_provenance()
        )),
        Err(MoveError::Refused(PageStoreError::Unavailable))
    );
    let state = store.state.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(state.pages.len(), 1);
    assert_eq!(state.pages.get(&name).map(Vec::len), Some(1));
    assert_eq!(state.bindings.len(), 1);
    assert_eq!(state.bindings.get(&view), Some(&binding()));
}

#[test]
fn a_publish_that_cannot_name_its_version_leaves_no_page_behind() {
    let store = MemoryPageStore::new();
    let refused = store.publish_with(page(), original(), |_| None);
    assert_eq!(refused, Err(PageStoreError::Unavailable));
    assert_eq!(block_on(store.pages()), Ok(Vec::new()));
}

#[test]
fn a_publish_names_the_version_after_the_versions_the_page_has() {
    let store = MemoryPageStore::new();
    let first = store
        .publish_with(page(), original(), |existing| {
            assert_eq!(existing, 0);
            Some(version("first"))
        })
        .unwrap();
    let second = store
        .publish_with(page(), original(), |existing| {
            assert_eq!(existing, 1);
            Some(version("second"))
        })
        .unwrap();
    assert_eq!(
        block_on(store.versions(&page_name())),
        Ok(vec![second, first])
    );
}

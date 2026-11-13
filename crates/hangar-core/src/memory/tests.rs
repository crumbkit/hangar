//! Unit tests of the in-memory stores' handling of a poisoned lock.

use super::*;
use crate::auth::Principal;
use crate::name::PrincipalId;
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

#[test]
fn a_record_store_with_a_poisoned_lock_is_unavailable() {
    let store = MemoryRecordStore::new();
    poison(&store.state);
    let path = RecordPath::try_from("/tasks/a").unwrap();
    assert_eq!(block_on(store.read(&path)), Err(ReadError::Unavailable));
    let blob = Blob::try_from("0".repeat(40).as_str()).unwrap();
    let write = WriteProvenance::new(
        blob,
        Principal::new(PrincipalId::try_from("ada").unwrap()),
        BindingName::try_from("view").unwrap(),
    );
    let body = RecordBody::try_from("x").unwrap();
    assert_eq!(
        block_on(store.write(&path, &body, &write)),
        Err(WriteError::Unavailable)
    );
    assert_eq!(store.last_write(&path), None);
}

#[test]
fn a_page_store_with_a_poisoned_lock_is_unavailable() {
    let store = MemoryPageStore::new();
    poison(&store.state);
    let name = PageName::try_from("home").unwrap();
    assert_eq!(block_on(store.pages()), Err(PageStoreError::Unavailable));
    assert_eq!(
        block_on(store.versions(&name)),
        Err(PageStoreError::Unavailable)
    );
}

#[test]
fn seeding_a_record_with_a_poisoned_lock_is_unavailable() {
    let store = MemoryRecordStore::new();
    poison(&store.state);
    let seeded = store.seed(
        RecordPath::try_from("/tasks/a").unwrap(),
        RecordType::try_from("tasks/task/v2").unwrap(),
        RecordBody::try_from("x").unwrap(),
    );
    assert_eq!(seeded, Err(WriteError::Unavailable));
}

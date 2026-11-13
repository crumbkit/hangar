//! Runs the four concurrency clauses with a runner that starts no thread. It
//! creates both futures on the calling thread and polls them in turn, as
//! `join` does on a runtime, so a store whose futures need a runtime can
//! implement `RunTogether` the same way.
#![cfg(test)]

use hangar_core::contract::page_store::{self, PageFixture};
use hangar_core::contract::record_store::{self, RecordFixture};
use hangar_core::contract::{RunTogether, SeedRecord, block_on};
use hangar_core::memory::{MemoryPageStore, MemoryRecordStore};
use std::future::{Future, poll_fn};
use std::pin::pin;
use std::sync::Mutex;
use std::task::Poll;
use std::thread::ThreadId;

/// Polls both futures on one task until both are ready.
async fn join<A, B>(
    first: impl Future<Output = A>,
    second: impl Future<Output = B>,
) -> Option<(A, B)> {
    let mut first = pin!(first);
    let mut second = pin!(second);
    let mut first_answer = None;
    let mut second_answer = None;
    poll_fn(|context| {
        if first_answer.is_none() {
            first_answer = match first.as_mut().poll(context) {
                Poll::Ready(answer) => Some(answer),
                Poll::Pending => None,
            };
        }
        if second_answer.is_none() {
            second_answer = match second.as_mut().poll(context) {
                Poll::Ready(answer) => Some(answer),
                Poll::Pending => None,
            };
        }
        if first_answer.is_some() && second_answer.is_some() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    first_answer.zip(second_answer)
}

/// A runner that creates and polls both futures on the thread that calls it,
/// and notes the thread of each call it makes.
#[derive(Default)]
struct OneThread {
    threads: Mutex<Vec<ThreadId>>,
}

impl OneThread {
    fn note(&self) {
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
    }

    fn all_on_this_thread(&self) -> bool {
        let threads = self.threads.lock().unwrap();
        threads.len() > 2 && threads.iter().all(|id| *id == std::thread::current().id())
    }
}

impl RunTogether for OneThread {
    async fn run_together<A, B, FA, FB>(
        &self,
        first: impl FnOnce() -> FA + Send,
        second: impl FnOnce() -> FB + Send,
    ) -> Option<(A, B)>
    where
        A: Send,
        B: Send,
        FA: Future<Output = A> + Send,
        FB: Future<Output = B> + Send,
    {
        self.note();
        let first = first();
        self.note();
        let second = second();
        join(first, second).await
    }
}

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
fn the_record_write_clause_runs_with_a_runner_that_starts_no_thread() {
    let fixture = RecordFixture::standard().unwrap();
    let store = seeded_records(fixture.seeds());
    let runner = OneThread::default();
    block_on(
        record_store::concurrent_writes_from_one_blob_give_one_conflict(&store, &fixture, &runner),
    )
    .unwrap();
    assert!(runner.all_on_this_thread());
}

macro_rules! page_clause_without_threads {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() {
            let fixture = PageFixture::standard().unwrap();
            let records = seeded_records(fixture.seeds());
            let pages = MemoryPageStore::new();
            let runner = OneThread::default();
            block_on(page_store::$name(&pages, &records, &fixture, &runner)).unwrap();
            assert!(runner.all_on_this_thread());
        }
    )*};
}

page_clause_without_threads!(
    concurrent_publishes_get_two_versions,
    concurrent_saves_of_one_name_give_one_name_taken,
    concurrent_moves_from_one_pin_give_one_moved,
);

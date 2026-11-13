//! A small executor, and a runner for two calls that run together.

use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Barrier};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::Thread;

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Polls `future` on the calling thread until it is ready, and parks the
/// thread while the future waits to be woken.
///
/// Shortcut: this executor has no reactor and no timer. It runs the futures
/// of the in-memory stores, and of any store whose futures wake their waker
/// from another thread. It misses a store whose futures need the context of
/// a runtime, and that store runs the suites on its own executor, as
/// [`contract`](super) describes.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

/// Drives two calls so that they run at the same moment. The `concurrent`
/// clauses of the contract suites take one and await the future it returns.
///
/// Each call comes as a closure that creates the call's future, so the
/// runner decides when each store method runs. The future resolves to both
/// answers, or to `None` when either call panicked.
///
/// [`Threads`] is the runner for the in-memory stores and for any store whose
/// futures run on a plain thread. It blocks the thread that polls the future
/// while its two threads run. A store whose futures need the context of a
/// runtime implements this trait on its own runner and awaits both futures on
/// that runtime, for example with the runtime's `join`, with no thread
/// blocked. Such a runner interleaves the two calls at their await points,
/// and that exposes a narrower set of races than two threads do.
pub trait RunTogether {
    /// Runs the calls that `first` and `second` create at the same moment and
    /// resolves when both are done.
    fn run_together<A, B, FA, FB>(
        &self,
        first: impl FnOnce() -> FA + Send,
        second: impl FnOnce() -> FB + Send,
    ) -> impl Future<Output = Option<(A, B)>> + Send
    where
        A: Send,
        B: Send,
        FA: Future<Output = A> + Send,
        FB: Future<Output = B> + Send;
}

/// The runner that gives each call its own thread and drives its future with
/// [`block_on`]. Each thread creates its future after the two threads leave a
/// barrier.
#[derive(Debug, Clone, Copy, Default)]
pub struct Threads;

impl RunTogether for Threads {
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
        run_on_threads(|| block_on(first()), || block_on(second()))
    }
}

/// Runs `first` and `second` on two threads that start together, and returns
/// both results. Returns `None` when either call panicked.
///
/// Shortcut: the two threads leave a barrier at the same moment and no more
/// than that. A store whose race window is narrower than the gap between two
/// threads that leave a barrier passes a run by luck, so the concurrency
/// clauses repeat the pair several times.
fn run_on_threads<A: Send, B: Send>(
    first: impl FnOnce() -> A + Send,
    second: impl FnOnce() -> B + Send,
) -> Option<(A, B)> {
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        let a = scope.spawn(|| {
            barrier.wait();
            first()
        });
        let b = scope.spawn(|| {
            barrier.wait();
            second()
        });
        let first_result = a.join();
        let second_result = b.join();
        Some((first_result.ok()?, second_result.ok()?))
    })
}

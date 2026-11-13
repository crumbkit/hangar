//! A minimal executor for futures that never wait.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// Polls `future` until it is ready. The in-memory implementations never
/// wait, so each future completes on its first poll.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::yield_now();
    }
}

#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::{
    future::{Future, IntoFuture},
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread,
};

#[cfg(feature = "macro")]
pub use pollster_macro::{main, test};

/// An extension trait that allows blocking on a future in suffix position.
pub trait FutureExt: IntoFuture {
    /// Block the thread until the future is ready.
    ///
    /// # Example
    ///
    /// ```
    /// use pollster::FutureExt as _;
    ///
    /// let my_fut = async {};
    ///
    /// let result = my_fut.block_on();
    /// ```
    fn block_on(self) -> Self::Output
    where
        Self: Sized,
    {
        block_on(self)
    }
}

impl<F: IntoFuture> FutureExt for F {}

struct Signal {
    /// The thread that created the signal.
    thread: thread::Thread,
    notified: AtomicBool,
}

impl Wake for Signal {
    fn wake(self: Arc<Signal>) {
        Self::wake_by_ref(&self);
    }

    fn wake_by_ref(self: &Arc<Signal>) {
        if !self.notified.swap(true, Ordering::Release) {
            self.thread.unpark();
        }
    }
}

/// Block the thread until the future is ready.
///
/// # Example
///
/// ```
/// let my_fut = async {};
/// let result = pollster::block_on(my_fut);
/// ```
pub fn block_on<F: IntoFuture>(fut: F) -> F::Output {
    let mut fut = core::pin::pin!(fut.into_future());

    let sig = Arc::new(Signal { thread: thread::current(), notified: false.into() });
    let waker = Waker::from(sig.clone());
    // Create a context to be passed to the future.
    let mut context = Context::from_waker(&waker);

    // Poll the future to completion.
    loop {
        match fut.as_mut().poll(&mut context) {
            // If the future is not yet ready, park
            Poll::Pending => {
                while !sig.notified.swap(false, Ordering::Acquire) {
                    thread::park();
                }
            }
            Poll::Ready(item) => break item,
        }
    }
}

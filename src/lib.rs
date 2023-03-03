#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread,
};

thread_local! {
    // A local reusable signal for each thread.
    static LOCAL_THREAD_SIGNAL: Arc<Signal> = Arc::new(Signal {
        owning_thread: thread::current(),
    });
}

#[cfg(feature = "macro")]
pub use pollster_macro::{main, test};

/// An extension trait that allows blocking on a future in suffix position.
pub trait FutureExt: Future {
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

impl<F: Future> FutureExt for F {}

struct Signal {
    /// The thread that owns the signal.
    owning_thread: thread::Thread,
}

impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.owning_thread.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.notify();
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

    // A signal used to wake up the thread for polling as the future moves to completion.
    LOCAL_THREAD_SIGNAL.with(|signal| {
        // Create a waker and a context to be passed to the future.
        let waker = Waker::from(Arc::clone(signal));
        let mut context = Context::from_waker(&waker);

        // Poll the future to completion.
        loop {
            match fut.as_mut().poll(&mut context) {
                Poll::Pending => thread::park(),
                Poll::Ready(item) => break item,
            }
        }
    })
}

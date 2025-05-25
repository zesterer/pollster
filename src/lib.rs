#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![feature(thread_raw)]

use std::{
    future::{Future, IntoFuture},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread::{self, Thread},
};

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

static RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    // clone
    |ptr| unsafe {
        // Reify ownership of `Thread`
        // TODO: It is not entirely clear if this is legal in a multi-threaded context
        let thread = Thread::from_raw(ptr);
        // Clone the `Thread`
        let clone = thread.clone();
        // Forget the previously owned instance, since cloning doesn't take ownership
        core::mem::forget(thread);
        RawWaker::new(clone.into_raw(), &RAW_WAKER_VTABLE)
    },
    // wake - take ownership and unpark
    |ptr| unsafe {
        Thread::from_raw(ptr).unpark();
    },
    // wake_by_ref
    |ptr| unsafe {
        // Reify ownership of `Thread`
        // TODO: It is not entirely clear if this is legal in a multi-threaded context
        let thread = Thread::from_raw(ptr);
        // Clone the `Thread`, unpark the clone
        thread.clone().unpark();
        // Forget the previously owned instance, since wake_by_ref doesn't take ownership
        core::mem::forget(thread);
    },
    // drop - take ownership and drop
    |ptr| unsafe {
        Thread::from_raw(ptr);
    },
);

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

    // Create a waker and a context to be passed to the future.
    // SAFETY: Use of `Thread::from_raw` and `Thread::into_raw` is
    // thread-safe and respects ownership requirements.
    let waker = unsafe { Waker::new(thread::current().into_raw(), &RAW_WAKER_VTABLE) };
    let mut context = Context::from_waker(&waker);

    // Poll the future to completion.
    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Pending => thread::park(),
            Poll::Ready(item) => break item,
        }
    }
}

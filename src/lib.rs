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
    thread: std::sync::OnceLock<thread::Thread>,
    notified: AtomicBool,
}

impl Wake for Signal {
    fn wake(self: Arc<Signal>) {
        Self::wake_by_ref(&self);
    }

    fn wake_by_ref(self: &Arc<Signal>) {
        // Note: Unparking only needs to occur if the blocked thread has not already been notified.
        // Q: Can this get away with ever not unparking a blocked thread? A: No, because `notified`
        // would need to have already been `true`, which means the thread's park token has already
        // been made available anyway.
        if !self.notified.swap(true, Ordering::Release) {
            // Unpark the thread, if one is available. If one isn't available, the blocking call
            // *must* re-poll this future, so skipping the unpark is fine: we'll get it on the next
            // pass!
            if let Some(thread) = self.thread.get() {
                thread.unpark();
            }
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

    let waker = Waker::from(Arc::new(Signal {
        thread: std::sync::OnceLock::new(),
        notified: false.into(),
    }));
    // Create a context to be passed to the future.
    let mut context = Context::from_waker(&waker);

    // Perform a speculative initial poll: a lot of futures are ready on first poll, and if this is
    // the case then we get to avoid the expensive `thread::current()` call (which requires a
    // syscall on most platforms!)
    match fut.as_mut().poll(&mut context) {
        Poll::Pending => {
            _ = unsafe { &*waker.data().cast::<Signal>() }
                .thread
                .set(thread::current())
        }
        Poll::Ready(item) => return item,
    }

    // Poll the future to completion.
    loop {
        match fut.as_mut().poll(&mut context) {
            // If the future is not yet ready, park
            Poll::Pending => {
                // To avoid excess polling due to spurious wakeups, only perform polling if
                // `notified` was set. Q: Is it possible for this loop to park the thread despite
                // there having been a notification? A: No, because `park` is only called if
                // notification was not set, and if it became set in the time since the check, the
                // thread token is now available anyway and `park` will immediately return.
                //
                // SAFETY: The waker data is always guaranteed to point to the `Signal` we created
                // above
                while !unsafe { &*waker.data().cast::<Signal>() }
                    .notified
                    .swap(false, Ordering::Acquire)
                {
                    thread::park();
                }
            }
            Poll::Ready(item) => break item,
        }
    }
}

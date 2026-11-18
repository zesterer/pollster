#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::{
    cell::UnsafeCell,
    future::{Future, IntoFuture},
    mem::MaybeUninit,
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

#[repr(align(64))]
struct Signal {
    /// The thread that created the signal.
    thread: UnsafeCell<MaybeUninit<thread::Thread>>,
    thread_is_set: AtomicBool,
    notified: AtomicBool,
}

impl Drop for Signal {
    fn drop(&mut self) {
        // Ensure that `self.thread` gets dropped if occupied
        if *self.thread_is_set.get_mut() {
            // SAFETY: `thread_is_set` is true, therefore `self.thread` is occupied
            unsafe { (&mut *self.thread.get()).assume_init_drop() };
        }
    }
}

// SAFETY: `Signal`'s `thread` field has the same semantics as an atomic `Once`
unsafe impl Send for Signal {}
unsafe impl Sync for Signal {}

impl Wake for Signal {
    fn wake(self: Arc<Signal>) {
        Self::wake_by_ref(&self);
    }

    fn wake_by_ref(self: &Arc<Signal>) {
        // Note: Unparking only needs to occur if the blocked thread has not already been notified.
        // Q: Can this get away with ever not unparking a blocked thread? A: No, because `notified`
        // would need to have already been `true`, which means the thread's park token has already
        // been made available anyway.
        if !self.notified.swap(true, Ordering::SeqCst) {
            // Unpark the thread, if one is available. If one isn't available, the blocking call
            // *must* re-poll this future, so skipping the unpark is fine: we'll get it on the next
            // pass!
            // Note: `SeqCst` is required to ensure that operations on `notified` and
            // `thread_is_set` do not become desynced.
            if self.thread_is_set.load(Ordering::SeqCst) {
                // SAFETY: `thread_is_set` is true, therefore `self.thread` is occupied
                unsafe { (&*self.thread.get()).assume_init_ref() }.unpark();
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

    let sig = Arc::new(Signal {
        thread: UnsafeCell::new(MaybeUninit::uninit()),
        thread_is_set: AtomicBool::new(false),
        notified: AtomicBool::new(false),
    });
    let sig_ptr = Arc::as_ptr(&sig);
    let waker = Waker::from(sig);
    // Create a context to be passed to the future.
    let mut context = Context::from_waker(&waker);

    // Perform a speculative initial poll: a lot of futures are ready on first poll, and if this is
    // the case then we get to avoid the expensive `thread::current()` call (which requires a
    // syscall on most platforms!)
    if let Poll::Ready(item) = fut.as_mut().poll(&mut context) {
        return item;
    }

    // The speculative poll failed: so take the slow path
    //
    // SAFETY: `waker` will always contain a valid `Signal`, and `waker` lives as long as the
    // current scope, so this is valid.
    let sig = unsafe { &*sig_ptr };
    // SAFETY: We created the waker and `thread_is_set` was not previously set by us, so no other
    // thread is permitted to access `sig.thread`. Also, this is synced with the write to
    // `thread_is_set` via `SeqCst` ordering below
    unsafe { sig.thread.get().write(MaybeUninit::new(thread::current())) };
    sig.thread_is_set.store(true, Ordering::SeqCst);

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
                while !sig.notified.swap(false, Ordering::SeqCst) {
                    thread::park();
                }
            }
            Poll::Ready(item) => break item,
        }
    }
}

//! A minimal async executor that lets you block on a future
//!
//! Note that `pollster` will not work for *arbitrary* futures because some require a specific runtime or reactor. See
//! [here](https://rust-lang.github.io/async-book/08_ecosystem/00_chapter.html#determining-ecosystem-compatibility) for
//! more information about when and where `pollster` may be used. However, if you're already pulling in the required
//! dependencies to create such a future in the first place, it's likely that you already have a version of `block_on`
//! in your dependency tree that's designed to poll your future, so use that instead.
//!
//! # Example
//!
//! ```
//! let my_fut = async {};
//! let result = pollster::block_on(my_fut);
//! ```

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, Mutex},
    task::{Context, Poll, Wake, Waker},
};

/// An extension trait that allows blocking on a future in suffix position.
pub trait FutureExt: Future {
    /// Block the thread until the future is ready.
    ///
    /// # Example
    ///
    /// ```
    /// let my_fut = async {};
    /// let result = pollster::block_on(my_fut);
    /// ```
    fn block_on(self) -> Self::Output where Self: Sized { block_on(self) }
}

impl<F: Future> FutureExt for F {}

enum SignalState {
    Empty,
    Waiting,
    Notified,
}

struct Signal {
    state: Mutex<SignalState>,
    cond: Condvar,
}

impl Signal {
    fn new() -> Self {
        Self {
            state: Mutex::new(SignalState::Empty),
            cond: Condvar::new(),
        }
    }

    fn wait(&self) {
        let mut state = self.state.lock().unwrap();
        match *state {
            SignalState::Notified => {
                // Notify() was called before we got here, consume it here without waiting and return immediately.
                *state = SignalState::Empty;
                return;
            }
            // This should not be possible because our signal is created within a function and never handed out to any
            // other threads. If this is the case, we have a serious problem so we panic immediately to avoid anything
            // more problematic happening.
            SignalState::Waiting => {
                unreachable!("Multiple threads waiting on the same signal: Open a bug report!");
            }
            SignalState::Empty => {
                // Nothing has happened yet, and we're the only thread waiting (as should be the case!). Set the state
                // accordingly and begin polling the condvar in a loop until it's no longer telling us to wait. The
                // loop prevents incorrect spurious wakeups.
                *state = SignalState::Waiting;
                while let SignalState::Waiting = *state {
                    state = self.cond.wait(state).unwrap();
                }
            }
        }
    }

    fn notify(&self) {
        let mut state = self.state.lock().unwrap();
        match *state {
            // The signal was already notified, no need to do anything because the thread will be waking up anyway
            SignalState::Notified => {}
            // The signal wasnt notified but a thread isnt waiting on it, so we can avoid doing unnecessary work by
            // skipping the condvar and leaving behind a message telling the thread that a notification has already
            // occurred should it come along in the future.
            SignalState::Empty => *state = SignalState::Notified,
            // The signal wasnt notified and there's a waiting thread. Reset the signal so it can be wait()'ed on again
            // and wake up the thread. Because there should only be a single thread waiting, `notify_all` would also be
            // valid.
            SignalState::Waiting => {
                *state = SignalState::Empty;
                self.cond.notify_one();
            }
        }
    }
}

impl Wake for Signal {
    fn wake(self: Arc<Self>) {
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
pub fn block_on<F: Future>(mut fut: F) -> F::Output {
    // Signal used to wake up the thread for polling as the future moves to completion. We need to use an `Arc`
    // because, although the lifetime of `fut` is limited to this function, the underlying IO abstraction might keep
    // the signal alive for far longer. `Arc` is a thread-safe way to allow this to happen.
    let signal = Arc::new(Signal::new());

    // Create a context that will be passed to the future.
    let waker = Waker::from(Arc::clone(&signal));
    let mut context = Context::from_waker(&waker);

    // Poll the future to completion
    loop {
        // Safe because `fut` isn't going to move until this function returns, at which point we've stopped polling
        // anyway. This is also unwind-safe because `fut` is only required to be pinned for the duration of the
        // function call. If an unwind past this function occurs (i.e: moving `fut` from its position on the stack),
        // then flow control has already passed out of the region in which `fut` is required to be pinned (`fut.poll`).
        // Don't believe me? The `pin_mut!` macro in the crate `pin_utils` does the exact same thing.
        let fut = unsafe { Pin::new_unchecked(&mut fut) };
        match fut.poll(&mut context) {
            Poll::Pending => signal.wait(),
            Poll::Ready(item) => break item,
        }
    }
}

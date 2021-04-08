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
    mem::forget,
    pin::Pin,
    sync::{Arc, Condvar, Mutex},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
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

// This type alias is important! It's put here so that it can be used elsewhere to prevent our pointer casts going out
// of sync and producing references to incorrect types.
type SignalArc = Arc<Signal>;

// Safety: each `signal` is a valid `Arc` with a reference count of at least 1. Ergo, it is safe to turn it back into
// an `Arc` provided we do not decrement the reference count.
static VTABLE: RawWakerVTable = unsafe {
    RawWakerVTable::new(
        /* Clone */
        |signal| {
            // Take ownership of the `Arc` (i.e: don't change its reference count)
            let arc = SignalArc::from_raw(signal as *const _);
            // Clone the `Arc`, increasing the reference count: now we have at least two owners.
            let waker = RawWaker::new(Arc::into_raw(arc.clone()) as *const _, &VTABLE);
            // Forget the original because, although we previously took ownership, this function should behave 'as if'
            // the ownership of the `Arc` did not change. Forgetting the `Arc` will prevent the reference count being
            // decremented, thereby keeping the `Arc` alive.
            forget(arc);
            waker
        },
        /* Wake */
        // Notify and implicitly drop the Arc (`wake` takes ownership)
        |signal| {
            let arc = SignalArc::from_raw(signal as *const _);
            arc.notify();
            // Drop our `Arc`, taking ownership of the inner value and dropping it also if we are the only owner.
            drop(arc);
        },
        /* Wake by ref */
        // Notify without dropping the Arc (`wake_by_ref` does not take ownership). We do this by ignoring the `Arc`
        // abstraction entirely and dereferencing the inner value (which is still owned by the `Arc`).
        |signal| (&*(signal as *const Signal)).notify(),
        /* Drop */
        // Drop the Arc (will deallocate the signal if this is the last `RawWaker`)
        |signal| drop(SignalArc::from_raw(signal as *const Signal)),
    )
};

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

    // Safe because the `Arc` is cloned and is still considered to have an owner until dropped in
    // the `RawWakerVTable` above (`Arc::into_raw` does not decrease the reference count).
    let waker = unsafe {
        Waker::from_raw(RawWaker::new(
            Arc::into_raw(signal.clone()) as *const _,
            &VTABLE,
        ))
    };

    // Poll the future to completion
    loop {
        // Safe because `fut` isn't going to move until this function returns, at which point we've stopped polling
        // anyway. This is also unwind-safe because `fut` is only required to be pinned for the duration of the
        // function call. If an unwind past this function occurs (i.e: moving `fut` from its position on the stack),
        // then flow control has already passed out of the region in which `fut` is required to be pinned (`fut.poll`).
        // Don't believe me? The `pin_mut!` macro in the crate `pin_utils` does the exact same thing.
        let fut = unsafe { Pin::new_unchecked(&mut fut) };
        match fut.poll(&mut Context::from_waker(&waker)) {
            Poll::Pending => signal.wait(),
            Poll::Ready(item) => break item,
        }
    }
}

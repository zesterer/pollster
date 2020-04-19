//! A minimal async executor that lets you block on a future
//!
//! # Example
//!
//! ```
//! let my_future = async {};
//! let result = pollster::block_on(my_future);
//! ```

use std::{
    mem::forget,
    future::Future,
    pin::Pin,
    task::{Poll, Context, Waker, RawWaker, RawWakerVTable},
    sync::{Condvar, Mutex, Arc},
};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
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

        // notify() was called before us, cosume it here without waiting.
        if *state == SignalState::Notified {
            *state = SignalState::Empty;
            return;
        }
        
        // state is either Empty or Waiting.
        // no other thread should be Waiting
        debug_assert_eq!(
            *state,
            SignalState::Empty,
            "Multiple threads waiting on same Signal",
        );

        // Wait on the state until its reset back to Empty.
        // Done in a loop to handle Condvar spurious wakeups
        *state = SignalState::Waiting;
        while *state == SignalState::Waiting {
            state = self.cond.wait(state).unwrap();
        }
    }

    fn notify(&self) {
        let mut state = self.state.lock().unwrap();

        match *state {
            // The signal was already notified
            SignalState::Notified => {},

            // The signal wasnt notified but a thread isnt waiting on it
            // so no need to call into the Condvar to wake one up
            SignalState::Empty => *state = SignalState::Notified,

            // The signal wasnt notified and theres a waiting thread.
            // Reset the signal so it can be wait()'ed on again & wake up the thread.
            SignalState::Waiting => {
                *state = SignalState::Empty;
                self.cond.notify_one();
            },
        }
    }
}

static VTABLE: RawWakerVTable = unsafe { RawWakerVTable::new(
    |signal| {
        let arc = Arc::from_raw(signal);
        let waker = RawWaker::new(Arc::into_raw(arc.clone()) as *const _, &VTABLE);
        // Forget the original `Arc` because we don't actually own it and we don't want to lower
        // its reference count.
        forget(arc);
        waker
    },
    // Notify and implicitly drop the Arc (`wake` takes ownership)
    |signal| Arc::from_raw(signal as *const Signal).notify(),
    // Notify without dropping the Arc (`wake_by_ref` does not take ownership)
    |signal| (&*(signal as *const Signal)).notify(),
    // Drop the Arc (will deallocate the signal if this is the last `RawWaker`)
    |signal| drop(Arc::from_raw(signal as *const Signal)),
) };

/// Block the thread until the future is ready.
///
/// # Example
///
/// ```
/// let my_future = async {};
/// let result = pollster::block_on(my_future);
/// ```
pub fn block_on<F: Future>(mut fut: F) -> F::Output {
    // Signal used to wake up the thread for polling as the future moves to completion.
    let signal = Arc::new(Signal::new());

    // Safe because the `Arc` is cloned and is still considered to have an owner until dropped in
    // the `RawWakerVTable` above.
    let waker = unsafe { Waker::from_raw(RawWaker::new(Arc::into_raw(signal.clone()) as *const _, &VTABLE)) };

    // Poll the future to completion
    loop {
        // Safe because `fut` isn't going to move until this function returns, at which point we've
        // stop polling anyway.
        let fut = unsafe { Pin::new_unchecked(&mut fut) };
        match fut.poll(&mut Context::from_waker(&waker)) {
            Poll::Pending => signal.wait(),
            Poll::Ready(item) => break item,
        }
    }
}

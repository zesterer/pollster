//! A minimal async executor that lets you block on a future
//!
//! # Example
//!
//! ```ignore
//! let result = pollster::block_on(my_future);
//! ```

use std::{
    mem::forget,
    future::Future,
    pin::Pin,
    task::{Poll, Context, Waker, RawWaker, RawWakerVTable},
    sync::{Condvar, Mutex, Arc},
};

struct Signal {
    lock: Mutex<bool>,
    cond: Condvar,
}

impl Signal {
    // Wait for a notification
    fn wait(&self) {
        let mut wakeup = self.lock.lock().unwrap();
        if !*wakeup {
            // Signal was notified since the last wakeup, so don't wait. This flag avoids 'missed'
            // notifications that might occur when we're not waiting.
            wakeup = self.cond.wait(wakeup).unwrap();
        }
        *wakeup = false;
    }

    // Notify the thread that's waiting on this signal
    fn notify(&self) {
        let mut wakeup = self.lock.lock().unwrap();
        *wakeup = true;
        self.cond.notify_one();
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
/// ```ignore
/// let result = pollster::block_on(my_future);
/// ```
pub fn block_on<F: Future>(mut fut: F) -> F::Output {
    // Signal used to wake up the thread for polling as the future moves to completion.
    let signal = Arc::new(Signal {
        lock: Mutex::new(false),
        cond: Condvar::new(),
    });

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

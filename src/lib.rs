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
    fn wait(&self) {
        let mut wakeup = self.lock.lock().unwrap();
        if !*wakeup {
            // Signal was notified since the last wakeup, don't wait
            wakeup = self.cond.wait(wakeup).unwrap();
        }
        *wakeup = false;
    }

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
        forget(arc);
        waker
    },
    // Notify by dropping the Arc (wake)
    |signal| Arc::from_raw(signal as *const Signal).notify(),
    // Notify without dropping the Arc (wake_by_ref)
    |signal| (&*(signal as *const Signal)).notify(),
    // Drop the Arc
    |signal| drop(Arc::from_raw(signal as *const Signal)),
) };

/// Block until the the future is ready.
pub fn block_on<F: Future>(mut fut: F) -> F::Output {
    let signal = Arc::new(Signal {
        lock: Mutex::new(false),
        cond: Condvar::new(),
    });

    let waker = unsafe { Waker::from_raw(RawWaker::new(Arc::into_raw(signal.clone()) as *const _, &VTABLE)) };

    loop {
        match unsafe { Pin::new_unchecked(&mut fut).poll(&mut Context::from_waker(&waker)) } {
            Poll::Pending => signal.wait(),
            Poll::Ready(item) => break item,
        }
    }
}

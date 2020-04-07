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

/// Block until the the future is ready.
pub fn block_on<F: Future>(mut fut: F) -> F::Output {
    static VTABLE: RawWakerVTable = unsafe { RawWakerVTable::new(
        |signal| {
            let arc = Arc::from_raw(signal);
            let waker = RawWaker::new(Arc::into_raw(arc.clone()) as *const _, &VTABLE);
            forget(arc);
            waker
        },
        |signal| {
            let arc = Arc::from_raw(signal as *const (Mutex<()>, Condvar));
            let _guard = arc.0.lock().unwrap(); arc.1.notify_one();
        },
        |signal| {
            let signal = &*(signal as *const (Mutex<()>, Condvar));
            let _guard = signal.0.lock().unwrap(); signal.1.notify_one();
        },
        |signal| drop(Arc::from_raw(signal as *const (Mutex<()>, Condvar))),
    ) };

    let signal = Arc::new((Mutex::new(()), Condvar::new()));
    let waker = unsafe { Waker::from_raw(RawWaker::new(Arc::into_raw(signal.clone()) as *const _, &VTABLE)) };
    let mut lock = signal.0.lock().unwrap();
    loop {
        match unsafe { Pin::new_unchecked(&mut fut).poll(&mut Context::from_waker(&waker)) } {
            Poll::Pending => lock = signal.1.wait(lock).unwrap(),
            Poll::Ready(item) => break item,
        }
    }
}

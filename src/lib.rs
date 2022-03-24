#![doc = include_str!("../README.md")]
#![feature(once_cell, pin_macro)]

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    pin::pin,
    thread::{self, Thread},
    sync::atomic::{AtomicUsize, Ordering},
    mem,
};

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
    fn block_on(self) -> Self::Output where Self: Sized { block_on(self) }
}

impl<F: Future> FutureExt for F {}

struct Signal {
    thread: AtomicUsize,
}

impl Signal {
    fn new() -> Self {
        Self { thread: AtomicUsize::new(0) }
    }

    fn wait(&self) {
        let thread_ptr = unsafe { Arc::into_raw(mem::transmute::<Thread, Arc<()>>(thread::current())) };
        match self.thread.compare_exchange(
            0,
            thread_ptr as usize,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => {
                while self.thread.load(Ordering::Relaxed) == thread_ptr as usize {
                    thread::park();
                }
            },
            Err(_) => {},
        }
    }

    fn notify(&self) {
        match self.thread.swap(1, Ordering::Acquire) {
            0 => {}, // No thread waiting yet
            1 => {}, // Notified twice, no effect
            ptr => unsafe {
                let thread = mem::transmute::<Arc<()>, Thread>(Arc::from_raw(ptr as *mut ()));
                thread.unpark();
                mem::forget(thread);
            },
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
    // Pin the future so that it can be polled.
    let mut fut = pin!(fut);

    // Signal used to wake up the thread for polling as the future moves to completion. We need to use an `Arc`
    // because, although the lifetime of `fut` is limited to this function, the underlying IO abstraction might keep
    // the signal alive for far longer. `Arc` is a thread-safe way to allow this to happen.
    // TODO: Investigate ways to reuse this `Arc<Signal>`... perhaps via a `static`?
    let signal = Arc::new(Signal::new());

    // Create a context that will be passed to the future.
    let waker = Waker::from(Arc::clone(&signal));
    let mut context = Context::from_waker(&waker);

    // Poll the future to completion
    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Pending => signal.wait(),
            Poll::Ready(item) => break item,
        }
    }
}

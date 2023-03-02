#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

use core::{
    future::Future,
    mem,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

#[cfg(feature = "std")]
use std::{
    sync::Arc,
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

    /// Block the thread until the future is ready with custom thread parking implementation.
    ///
    /// This allows one to use custom thread parking mechanisms in `no_std` environments.
    ///
    /// # Example
    ///
    /// ```
    /// use pollster::FutureExt as _;
    ///
    /// let my_fut = async {};
    ///
    /// let result = my_fut.block_on_t::<()>();
    /// ```
    fn block_on_t<T: Parkable>(self) -> Self::Output
    where
        Self: Sized,
    {
        block_on_t::<T, Self>(self)
    }
}

impl<F: Future> FutureExt for F {}

/// Parkable handle.
///
/// This handle allows a thread to potentially be efficiently blocked. This is used in the polling
/// implementation to wait for wakeups.
///
/// The interface models that of `std::thread`, and many functions, such as
/// [`current`](Parkable::current), [`park`](Parkable::park), and [`unpark`](Parkable::unpark)
/// map to `std::thread` equivalents.
pub trait Parkable: Sized + Clone {
    /// Get handle to current thread.
    fn current() -> Self;

    /// Park the current thread.
    fn park();

    /// Unpark specified thread.
    fn unpark(&self);

    /// Convert self into opaque pointer.
    ///
    /// This requires `Self` to either be layout compatible with `*const ()`.
    fn into_opaque(self) -> *const ();

    /// Convert opaque pointer into `Self`.
    ///
    /// # Safety
    ///
    /// The implementation is safe if:
    ///
    /// - `data` argument is a valid park handle created by `Self::into_opaque`.
    /// - `assert_eq!(into_opaque(from_opaque(data)), data)` holds for any data pointer.
    unsafe fn from_opaque(data: *const ()) -> Self;

    /// Create a waker out of `self`
    ///
    /// This function will clone self and build a `Waker` object.
    fn waker(&self) -> Waker {
        let data = self.clone().into_opaque();
        // SAFETY: `RawWaker` created by `raw_waker` builds a waker object out of the raw data and
        // vtable methods of this type which we assume are correct.
        unsafe { Waker::from_raw(raw_waker::<Self>(data)) }
    }
}

pub type DefaultHandle = Arc<Thread>;
#[cfg(not(feature = "std"))]
pub type DefaultHandle = ();

fn raw_waker<T: Parkable>(data: *const ()) -> RawWaker {
    RawWaker::new(
        data,
        &RawWakerVTable::new(
            clone_waker::<T>,
            wake::<T>,
            wake_by_ref::<T>,
            drop_waker::<T>,
        ),
    )
}

unsafe fn clone_waker<T: Parkable>(data: *const ()) -> RawWaker {
    let waker = T::from_opaque(data);
    let _ = T::into_opaque(waker.clone());
    let _ = T::into_opaque(waker);
    raw_waker::<T>(data)
}

unsafe fn wake<T: Parkable>(data: *const ()) {
    let waker = T::from_opaque(data);
    waker.unpark();
}

unsafe fn wake_by_ref<T: Parkable>(data: *const ()) {
    let waker = T::from_opaque(data);
    waker.unpark();
    let _ = T::into_opaque(waker);
}

unsafe fn drop_waker<T: Parkable>(data: *const ()) {
    let _ = T::from_opaque(data);
}

// Needs to be an Arc, because converting Thread to *const () is UB
// See: https://github.com/zesterer/pollster/pull/19
//
// This may be switched once `thread_raw` features is merged and stabilized. See:
// - https://github.com/rust-lang/libs-team/issues/200
// - https://github.com/rust-lang/rust/pull/97524
#[cfg(feature = "std")]
impl Parkable for Arc<Thread> {
    fn current() -> Self {
        thread::current().into()
    }

    fn park() {
        thread::park();
    }

    fn unpark(&self) {
        Thread::unpark(&**self);
    }

    fn into_opaque(self) -> *const () {
        Arc::into_raw(self) as *const ()
    }

    unsafe fn from_opaque(data: *const ()) -> Self {
        Arc::from_raw(data as *const Thread)
    }
}

impl Parkable for () {
    fn current() -> Self {}

    fn park() {
        core::hint::spin_loop()
    }

    fn unpark(&self) {}

    fn into_opaque(self) -> *const () {
        &()
    }

    unsafe fn from_opaque(_: *const ()) -> Self {
        ()
    }
}

/// Block the thread until the future is ready with custom parking implementation.
///
/// This allows one to use custom thread parking mechanisms in `no_std` environments.
///
/// # Example
///
/// ```
/// let my_fut = async {};
/// let result = pollster::block_on_t::<(), _>(my_fut);
/// ```
pub fn block_on_t<T: Parkable, F: Future>(mut fut: F) -> F::Output {
    // Pin the future so that it can be polled.
    // SAFETY: We shadow `fut` so that it cannot be used again. The future is now pinned to the stack and will not be
    // moved until the end of this scope. This is, incidentally, exactly what the `pin_mut!` macro from `pin_utils`
    // does.
    let mut fut = unsafe { std::pin::Pin::new_unchecked(&mut fut) };

    let handle = T::current();

    let waker: Waker = handle.waker();
    let mut context = Context::from_waker(&waker);

    // Poll the future to completion
    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Pending => T::park(),
            Poll::Ready(item) => break item,
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
pub fn block_on<F: Future>(fut: F) -> F::Output {
    return block_on_t::<DefaultHandle, _>(fut);
}

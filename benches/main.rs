use criterion::*;

use core::future::{poll_fn, ready};
use core::task::{Poll, Waker};
use pollster::block_on;

fn benches() {
    let mut c = Criterion::default();

    // Benchmark raw performance of pollster - processing of ready futures
    c.bench_function("poll_ready", |b| {
        b.iter(|| block_on(ready(black_box(false))))
    });

    // Benchmark how pollster fares with futures that return Pending,
    // but immediately wake up the thread.
    c.bench_function("wakeup_and_pending", |b| {
        b.iter(|| {
            let mut polled = false;
            let fut = poll_fn(|cx| {
                if !polled {
                    polled = true;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            });
            block_on(fut)
        })
    });

    // Benchmark how pollster fares with futures that get woken up from another thread.
    let (tx, rx) = std::sync::mpsc::channel::<Waker>();

    let thread = std::thread::spawn(move || {
        for waker in rx.into_iter() {
            waker.wake();
        }
    });

    c.bench_function("wait_for_thread", |b| {
        b.iter(|| {
            let mut polled = false;
            let fut = poll_fn(|cx| {
                if !polled {
                    polled = true;
                    tx.send(cx.waker().clone()).unwrap();
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            });
            block_on(fut)
        })
    });

    core::mem::drop(tx);

    thread.join().unwrap();
}

criterion_main!(benches);

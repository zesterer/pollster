use pollster::FutureExt as _;
use std::{
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
    thread,
};
use tokio::sync::mpsc;

const BOUNDED: usize = 16;
const MESSAGES: usize = 10_000_000;

fn main() {
    let (tx, mut rx) = mpsc::channel(BOUNDED);

    let thread = thread::spawn(move || {
        for i in 0..MESSAGES {
            tx.send(i).block_on().expect("Send on a");
        }
    });

    let sum = AtomicUsize::new(0);
    while sum.fetch_add(1, SeqCst) < MESSAGES {
        black_box(rx.recv().block_on());
    }
    assert_eq!(sum.load(SeqCst), MESSAGES + 1);

    thread.join().expect("join thread");
}

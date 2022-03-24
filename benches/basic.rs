#![feature(test)]
extern crate test;

use test::{black_box, Bencher};

fn simple_fut() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> {
    Box::pin(async {})
}

#[bench]
fn basic(b: &mut Bencher) {
    use pollster::FutureExt as _;
    b.iter(|| {
        black_box(simple_fut().block_on());
    });
}

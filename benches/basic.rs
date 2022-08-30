#![feature(test)]
extern crate test;

use test::{black_box, Bencher};

fn simple_fut() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> {
    Box::pin(tokio::time::sleep(std::time::Duration::from_nanos(1)))
}

#[bench]
fn basic_pollster(b: &mut Bencher) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let guard = rt.enter();

    use pollster::FutureExt as _;
    b.iter(|| {
        black_box(simple_fut().block_on());
    });
}

#[bench]
fn basic_pollster_old(b: &mut Bencher) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let guard = rt.enter();

    use pollster_old::FutureExt as _;
    b.iter(|| {
        black_box(simple_fut().block_on());
    });
}

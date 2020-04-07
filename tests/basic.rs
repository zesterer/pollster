use std::time::{Instant, Duration};
use pollster;

#[test]
fn basic() {
    let make_fut = || async_std::future::ready(42);

    // Immediately ready
    assert_eq!(pollster::block_on(make_fut()), 42);

    // Ready after a timeout
    let then = Instant::now();
    pollster::block_on(futures_timer::Delay::new(Duration::from_millis(250)));
    assert!(Instant::now().duration_since(then) > Duration::from_millis(250));
}

extern crate pollster as reexported_pollster;

use std::future::ready;

#[pollster::test]
async fn basic() {
    ready(42).await;
}

#[pollster::test]
async fn result() -> Result<(), std::io::Error> {
    if ready(42).await == 42 {
        Ok(())
    } else {
        unreachable!()
    }
}

#[pollster::test(crate = "reexported_pollster")]
async fn crate_() {
    ready(42).await;
}

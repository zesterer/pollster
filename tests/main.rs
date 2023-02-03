use std::future::ready;

#[pollster::main]
async fn main_basic() {
    ready(42).await;
}

#[test]
fn basic() {
    main_basic();
}

#[pollster::main]
async fn main_result() -> Result<(), std::io::Error> {
    ready(42).await;
    Ok(())
}

#[test]
fn result() {
    main_result().unwrap();
}

#[pollster::main(crate = "pollster")]
async fn main_crate() {
    ready(42).await;
}

#[test]
fn crate_() {
    main_crate();
}

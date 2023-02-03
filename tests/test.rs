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

#[pollster::test(crate = "pollster")]
async fn crate_() {
    ready(42).await;
}

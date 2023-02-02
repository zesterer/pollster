#[pollster::test]
async fn basic() {
    assert_eq!(42, 42);
}

#[pollster::test]
async fn result() -> Result<(), std::io::Error> {
    if 42 == 42 {
        Ok(())
    } else {
        unreachable!()
    }
}

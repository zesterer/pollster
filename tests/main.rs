#[pollster::main]
async fn main_basic() {
    let _ = 42;
}

#[test]
fn basic() {
    main_basic();
}

#[pollster::main]
async fn main_result() -> Result<(), std::io::Error> {
    let _ = 42;
    Ok(())
}

#[test]
fn result() {
    main_result().unwrap();
}

#[pollster::main(crate = "pollster")]
async fn main_crate() {
    let _ = 42;
}

#[test]
fn crate_() {
    main_crate();
}

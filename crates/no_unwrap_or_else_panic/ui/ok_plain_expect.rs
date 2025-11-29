fn main() {
    let value: Result<i32, &str> = Ok(2);
    let _ = value.expect("should succeed");
}

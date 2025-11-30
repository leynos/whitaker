//! UI test: plain `expect` is outside the lint scope.
fn main() {
    let value: Result<i32, &str> = Ok(2);
    let _ = value.expect("should succeed");
}

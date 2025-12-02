//! UI test: plain `unwrap` is outside the lint scope.
fn main() {
    let value = Some(1);
    let _ = value.unwrap();
}

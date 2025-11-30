//! UI test: panicking in `main` is allowed when configured.

fn main() {
    let result: Result<(), &str> = Err("boom");
    let _ = result.unwrap_or_else(|err| panic!("err: {err}", err = err));
}

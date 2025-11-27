#![allow(non_fmt_panics)]

fn main() {
    let result: Result<(), &str> = Err("boom");
    let _ = result.unwrap_or_else(|err| panic!("{err}"));
}

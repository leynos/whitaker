#![deny(no_unwrap_or_else_panic)]

fn demo(value: Result<i32, &str>) -> Result<i32, String> {
    value.map_err(|err| err.to_string())
}

fn main() {
    let _ = demo(Err("boom"));
}

#![deny(no_unwrap_or_else_panic)]

fn demo(flag: bool) -> i32 {
    let value: Result<i32, &str> = if flag { Ok(1) } else { Err("boom") };
    value.unwrap_or_else(|err| panic!("err: {err}", err = err))
}

fn main() {
    let _ = demo(true);
}

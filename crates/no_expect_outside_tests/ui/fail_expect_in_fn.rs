#![deny(no_expect_outside_tests)]

fn process() {
    let value = Some(42);
    let _result = value.expect("value should exist");
}

fn fail_result() {
    let result: Result<(), &'static str> = Err("boom");
    let _ = result.expect("result should be ok");
}

fn main() {
    process();
    fail_result();
}

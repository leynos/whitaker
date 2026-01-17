#![deny(no_expect_outside_tests)]

#[cfg_attr(test, allow(dead_code))]
fn handler() {
    let value = Some(1);
    let _ = value.expect("handler should not ignore errors");
}

fn main() {}

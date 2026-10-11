//! Negative regression: a module merely named `tests` is not test-only code.
//!
//! Only `cfg(test)` evidence marks a companion module as test-only (see
//! `pass_expect_in_file_backed_test_helper`). The module below is compiled into
//! the ordinary crate, so its helper must still trigger the lint even when the
//! crate is built with `--test`. A genuine production `expect()` beside it must
//! fire as well.

#[allow(dead_code)]
mod tests {
    pub fn helper() -> u32 {
        "7".parse::<u32>()
            .expect("a module named tests is not test-only code")
    }
}

fn production() -> u32 {
    std::iter::once(7_u32)
        .next()
        .expect("ordinary production code")
}

#[test]
fn unrelated_test() {
    assert_eq!(1, 1);
}

fn main() {
    let _ = production();
}

//! Negative regression: a module merely named `tests` is not test-only code.
//!
//! Only `cfg(test)` evidence marks a companion module as test-only (see
//! `pass_expect_in_file_backed_test_helper`). The module below is compiled into
//! the ordinary crate, so its helper must still trigger the lint even when the
//! crate is built with `--test`. A genuine production `expect()` in a function
//! and in an inherent method beside it must fire as well.

#[allow(dead_code)]
mod tests {
    //! Ordinary production code that happens to be called `tests`.

    /// Parse a constant, calling `expect` outside any test.
    pub fn helper() -> u32 {
        "7".parse::<u32>()
            .expect("a module named tests is not test-only code")
    }
}

/// Production code that calls `expect` in a free function.
fn production() -> u32 {
    std::iter::once(7_u32)
        .next()
        .expect("ordinary production code")
}

/// A production type whose method calls `expect`.
struct Config;

impl Config {
    /// Load a value, calling `expect` in an ordinary inherent method.
    fn load() -> u32 {
        std::iter::once(7_u32)
            .next()
            .expect("an ordinary method is not test-only code")
    }
}

/// An unrelated test, kept so the crate has a test of its own.
#[test]
fn unrelated_test() {
    assert_eq!(tests::helper(), 7);
}

/// Exercise the production code so nothing is dead.
fn main() {
    let _ = (production(), Config::load());
}

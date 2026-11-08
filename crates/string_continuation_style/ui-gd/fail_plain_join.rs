#![warn(string_continuation_style)]

//! Scottish Gaelic (`gd`) variant of the baseline interior-join fixture.
//!
//! The only difference from `ui/fail_plain_join.rs` is the diagnostic
//! text: `run_with_locale("ui-gd", Some("gd"))` selects the Gaelic Fluent
//! bundle, proving it renders every user-visible part, not English.

const VALUE: &str = "alpha \
    beta";

fn main() {
    assert_eq!(VALUE, "alpha beta");
}

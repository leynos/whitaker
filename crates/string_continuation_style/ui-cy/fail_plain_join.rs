#![warn(string_continuation_style)]

//! Welsh (`cy`) variant of the baseline interior-join fixture.
//!
//! The only difference from `ui/fail_plain_join.rs` is the diagnostic
//! text: `run_with_locale("ui-cy", Some("cy"))` selects the Welsh Fluent
//! bundle, proving it renders every user-visible part, not English.

const VALUE: &str = "alpha \
    beta";

fn main() {
    assert_eq!(VALUE, "alpha beta");
}

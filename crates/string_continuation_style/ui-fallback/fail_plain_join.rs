#![warn(string_continuation_style)]

//! Unknown-locale (`zz`) variant of the baseline interior-join fixture.
//!
//! `run_with_locale("ui-fallback", Some("zz"))` asks for a locale with no
//! bundle of its own, proving the fallback path supplies English for every
//! user-visible part instead of an empty or half-localized one.

const VALUE: &str = "alpha \
    beta";

fn main() {
    assert_eq!(VALUE, "alpha beta");
}

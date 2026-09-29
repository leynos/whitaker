//! Compile-fail contract for removed British-spelled common API names.

use whitaker_common::complexity_signal::rasterise_signal;
use whitaker_common::normalise_locale;

fn main() {
    let _ = normalise_locale(Some("en-GB"));
    let _ = rasterise_signal(1..=1, &[]);
}

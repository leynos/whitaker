//! Compile contract for Oxford-spelled common public APIs.

use whitaker_common::complexity_signal::{LineSegment, rasterize_signal};
use whitaker_common::normalize_locale;

fn main() {
    let locale = normalize_locale(Some(" en-GB "));
    let segments: Vec<_> = LineSegment::new(1, 1, 1.0).into_iter().collect();
    let signal = rasterize_signal(1..=1, &segments);
    assert_eq!(locale, Some("en-GB"));
    assert_eq!(signal.ok(), Some(vec![1.0]));
}

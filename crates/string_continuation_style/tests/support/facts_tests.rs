//! Tests newline detection at the compiler boundary.

use rustc_span::{NormalizedPos, Pos, RelativeBytePos};
use whitaker_string_literals::rewrite::Newline;

use super::dominant_newline;

#[test]
fn dominant_newline_uses_removed_crlf_count_and_ignores_a_bom() {
    let positions = [
        NormalizedPos {
            pos: RelativeBytePos::from_usize(0),
            diff: 3,
        },
        NormalizedPos {
            pos: RelativeBytePos::from_usize(2),
            diff: 4,
        },
        NormalizedPos {
            pos: RelativeBytePos::from_usize(4),
            diff: 5,
        },
    ];

    assert_eq!(dominant_newline("a\nb\nc\n", &positions), Newline::CrLf);
    assert_eq!(dominant_newline("a\nb\nc\nd\ne\n", &positions), Newline::Lf);
}

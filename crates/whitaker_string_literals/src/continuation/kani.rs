//! Bounded proofs for continuation-scanner invariants.
//! Bounded verification of continuation parity and classification.
//!
//! The harness exhausts all bodies of length at most five over the ASCII
//! alphabet `{\\, LF, a}`. This is 364 body/length combinations. It does not
//! cover the scanner's complete post-newline whitespace set; `rstest` and
//! `proptest` cover that larger rule.

use super::{Continuation, ContinuationKind, is_consumed_whitespace, scan_into};

const KANI_MAX_BODY_LEN: usize = 5;
const KANI_MAX_RECORDED_CONTINUATIONS: usize = 3;
const KANI_SCAN_UNWIND: usize = 9;
const _: () = assert!(KANI_SCAN_UNWIND == KANI_MAX_BODY_LEN + KANI_MAX_RECORDED_CONTINUATIONS + 1);
const _: () = assert!(KANI_MAX_RECORDED_CONTINUATIONS >= 2);

fn expected_continuation_count(body: &str) -> usize {
    let bytes = body.as_bytes();
    let mut count = 0_usize;

    for index in 0..bytes.len() {
        let next_index = index.saturating_add(1);
        if bytes.get(index) != Some(&b'\\') || bytes.get(next_index) != Some(&b'\n') {
            continue;
        }

        let mut run_start = index;
        while run_start > 0 && bytes.get(run_start.saturating_sub(1)) == Some(&b'\\') {
            run_start = run_start.saturating_sub(1);
        }
        let run_length = index.saturating_sub(run_start).saturating_add(1);
        if run_length & 1 == 1 {
            count = count.saturating_add(1);
        }
    }

    count
}

fn exactly_one_continuation_kind(continuation: &Continuation) -> bool {
    let is_join = continuation.kind == ContinuationKind::Join;
    let is_leading = continuation.kind == ContinuationKind::LeadingLayoutTrim;
    let is_trailing = continuation.kind == ContinuationKind::TrailingLayoutTrim;

    (is_join && !is_leading && !is_trailing)
        || (!is_join && is_leading && !is_trailing)
        || (!is_join && !is_leading && is_trailing)
}

#[kani::proof]
#[kani::unwind(9)]
fn verify_scanner_matches_bounded_parity_reference() {
    let mut bytes = [0_u8; KANI_MAX_BODY_LEN];
    let length: usize = kani::any();
    kani::assume(length <= KANI_MAX_BODY_LEN);
    for byte in bytes.iter_mut().take(length) {
        *byte = kani::any();
        kani::assume(*byte == b'\\' || *byte == b'\n' || *byte == b'a');
    }

    let Some(prefix) = bytes.get(..length) else {
        kani::assert(false, "bounded body length must fit its backing array");
        return;
    };
    let Ok(body) = core::str::from_utf8(prefix) else {
        kani::assert(false, "the restricted alphabet must be valid UTF-8");
        return;
    };

    let mut recorded = [const { None }; KANI_MAX_RECORDED_CONTINUATIONS];
    let mut recorded_count = 0_usize;
    let mut overflowed = false;
    scan_into(body, |continuation| {
        if let Some(slot) = recorded.get_mut(recorded_count) {
            *slot = Some(continuation);
            recorded_count = recorded_count.saturating_add(1);
        } else {
            overflowed = true;
        }
    });

    kani::assert(
        !overflowed,
        "fixed continuation sink must fit the input bound",
    );
    kani::assert(
        recorded_count == expected_continuation_count(body),
        "scanner continuation count must match independent parity reference",
    );

    for continuation in recorded.iter().flatten() {
        let escape_start = continuation.escape_range.start.get();
        let escape_end = continuation.escape_range.end.get();
        let skipped_start = continuation.skipped_whitespace_range.start.get();
        kani::assert(
            body.as_bytes().get(escape_start..escape_end) == Some(&b"\\\n"[..]),
            "every recorded escape must be one unpaired backslash and LF",
        );
        kani::assert(
            escape_end == skipped_start,
            "the skipped range must start immediately after the escape",
        );
        kani::assert(
            exactly_one_continuation_kind(continuation),
            "each scanned continuation must have exactly one classification",
        );
        let skipped_end = continuation.skipped_whitespace_range.end.get();
        let Some(skipped) = body.as_bytes().get(skipped_start..skipped_end) else {
            kani::assert(false, "the skipped range must be within the bounded body");
            continue;
        };
        for byte in skipped {
            kani::assert(
                is_consumed_whitespace(*byte),
                "the skipped range must contain only consumed whitespace",
            );
        }
    }
}

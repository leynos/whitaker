//! Verus model for removing sorted, disjoint continuation ranges.
//!
//! This proof establishes a sequence-level identity. It does not verify the
//! production Rust `String` renderer, Rust's literal unescaper, or rustc's
//! macro expansion. Those boundaries remain covered by the pure-domain tests
//! and compiler-backed run-rustfix fixtures.

use vstd::prelude::*;

verus! {

struct ByteRange {
    start: nat,
    end: nat,
}

spec fn last_range(ranges: Seq<ByteRange>) -> ByteRange {
    ranges[ranges.len() - 1]
}

spec fn ranges_before_last(ranges: Seq<ByteRange>) -> Seq<ByteRange> {
    ranges.subrange(0, (ranges.len() - 1) as int)
}

spec fn prefix_before_range(body: Seq<u8>, range: ByteRange) -> Seq<u8> {
    body.subrange(0, range.start as int)
}

spec fn suffix_after_range(body: Seq<u8>, range: ByteRange) -> Seq<u8> {
    body.subrange(range.end as int, body.len() as int)
}

/// Requires the final range to be valid and the preceding ranges to fit before it.
spec fn ranges_are_sorted_and_disjoint(body: Seq<u8>, ranges: Seq<ByteRange>) -> bool
    decreases ranges.len()
{
    if ranges.len() == 0 {
        true
    } else {
        let range = last_range(ranges);
        range.start < range.end
            && range.end <= body.len()
            && ranges_are_sorted_and_disjoint(
                prefix_before_range(body, range),
                ranges_before_last(ranges),
            )
    }
}

/// Returns each maximal byte sequence outside the selected continuation ranges.
spec fn split_fragments(body: Seq<u8>, ranges: Seq<ByteRange>) -> Seq<Seq<u8>>
    decreases ranges.len()
{
    if ranges.len() == 0 {
        seq![body]
    } else {
        let range = last_range(ranges);
        split_fragments(
            prefix_before_range(body, range),
            ranges_before_last(ranges),
        ) + seq![suffix_after_range(body, range)]
    }
}

/// Concatenates fragment sequences from the final fragment towards the first.
spec fn concatenate_fragments(fragments: Seq<Seq<u8>>) -> Seq<u8>
    decreases fragments.len()
{
    if fragments.len() == 0 {
        Seq::<u8>::empty()
    } else {
        let last_index = fragments.len() - 1;
        concatenate_fragments(fragments.subrange(0, last_index as int)) + fragments[last_index]
    }
}

/// Removes each selected range by keeping its surrounding maximal fragments.
spec fn remove_ranges(body: Seq<u8>, ranges: Seq<ByteRange>) -> Seq<u8>
    decreases ranges.len()
{
    if ranges.len() == 0 {
        body
    } else {
        let range = last_range(ranges);
        remove_ranges(
            prefix_before_range(body, range),
            ranges_before_last(ranges),
        ) + suffix_after_range(body, range)
    }
}

/// Concatenating all fragments is equivalent to deleting every selected range.
proof fn lemma_concatenating_fragments_removes_ranges(
    body: Seq<u8>,
    ranges: Seq<ByteRange>,
)
    requires
        ranges_are_sorted_and_disjoint(body, ranges),
    ensures
        concatenate_fragments(split_fragments(body, ranges)) =~= remove_ranges(body, ranges),
    decreases ranges.len()
{
    if ranges.len() == 0 {
        reveal_with_fuel(split_fragments, 1);
        reveal_with_fuel(concatenate_fragments, 2);
        reveal_with_fuel(remove_ranges, 1);
        assert(split_fragments(body, ranges) =~= seq![body]);
        assert(concatenate_fragments(seq![body]) =~= body);
        assert(remove_ranges(body, ranges) =~= body);
    } else {
        let range = last_range(ranges);
        let earlier_ranges = ranges_before_last(ranges);
        let prefix = prefix_before_range(body, range);
        let suffix = suffix_after_range(body, range);

        reveal(ranges_are_sorted_and_disjoint);
        reveal(last_range);
        reveal(ranges_before_last);
        reveal(prefix_before_range);
        reveal(suffix_after_range);
        assert(ranges_are_sorted_and_disjoint(prefix, earlier_ranges));
        lemma_concatenating_fragments_removes_ranges(prefix, earlier_ranges);

        let earlier_fragments = split_fragments(prefix, earlier_ranges);
        let all_fragments = earlier_fragments + seq![suffix];
        reveal(split_fragments);
        reveal(concatenate_fragments);
        reveal(remove_ranges);
        assert(all_fragments.len() == earlier_fragments.len() + 1);
        assert(all_fragments.subrange(0, earlier_fragments.len() as int) =~= earlier_fragments);
        assert(all_fragments[earlier_fragments.len() as int] == suffix);
        assert(
            concatenate_fragments(all_fragments)
                =~= concatenate_fragments(earlier_fragments) + suffix
        );
        assert(
            concatenate_fragments(earlier_fragments) =~= remove_ranges(prefix, earlier_ranges)
        );
        assert(
            remove_ranges(body, ranges) =~= remove_ranges(prefix, earlier_ranges) + suffix
        );
    }
}

/// Checks the proof on the concrete range removed from `"abc"`.
proof fn lemma_documented_abc_witness()
    ensures
        split_fragments(
            seq![97u8, 98u8, 99u8],
            seq![ByteRange { start: 1nat, end: 2nat }],
        ) =~= seq![seq![97u8], seq![99u8]],
        concatenate_fragments(
            split_fragments(
                seq![97u8, 98u8, 99u8],
                seq![ByteRange { start: 1nat, end: 2nat }],
            ),
        ) =~= seq![97u8, 99u8],
{
    let body = seq![97u8, 98u8, 99u8];
    let ranges = seq![ByteRange { start: 1nat, end: 2nat }];
    reveal_with_fuel(ranges_are_sorted_and_disjoint, 2);
    reveal(last_range);
    reveal(ranges_before_last);
    reveal(prefix_before_range);
    reveal(suffix_after_range);
    assert(ranges_are_sorted_and_disjoint(body, ranges));
    assert(last_range(ranges) == ByteRange { start: 1nat, end: 2nat }) by (compute);
    assert(ranges_before_last(ranges) =~= Seq::<ByteRange>::empty());
    assert(prefix_before_range(body, last_range(ranges)) =~= seq![97u8]);
    assert(suffix_after_range(body, last_range(ranges)) =~= seq![99u8]);
    lemma_concatenating_fragments_removes_ranges(body, ranges);
    reveal_with_fuel(split_fragments, 2);
    assert(split_fragments(body, ranges) =~= seq![seq![97u8], seq![99u8]]);
    reveal_with_fuel(remove_ranges, 2);
    assert(remove_ranges(body, ranges) =~= seq![97u8, 99u8]);
}

fn main() {
}

} // verus!

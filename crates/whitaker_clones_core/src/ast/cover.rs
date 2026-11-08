//! Pure span-covering helpers for AST adapter selection.

/// Selects the smallest candidate span that covers `target`.
///
/// When covering candidates have equal width, the first candidate encountered
/// is selected. For `root.descendants()` order, this may prefer an ancestor
/// over an equal-width child.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::ast::select_smallest_covering;
///
/// let candidates = [0..10, 2..5, 3..4];
/// assert_eq!(select_smallest_covering(&candidates, &(3..4)), Some(2));
/// ```
#[must_use]
pub fn select_smallest_covering(
    candidates: &[std::ops::Range<u32>],
    target: &std::ops::Range<u32>,
) -> Option<usize> {
    let mut selected: Option<(usize, u32)> = None;
    for (index, candidate) in candidates.iter().enumerate() {
        let selected_width = selected.map(|(_, width)| width);
        if should_select_covering_candidate(candidate, target, selected_width) {
            // The width is carried with the index so selection never indexes
            // back into `candidates` to re-measure the candidate it chose.
            selected = Some((index, candidate_width(candidate)));
        }
    }

    selected.map(|(index, _)| index)
}

/// Width of a range whose bounds are known to be ordered.
///
/// Callers reach this only after [`candidate_covers_target`] has accepted the
/// range, which is what guarantees the subtraction cannot underflow.
const fn candidate_width(candidate: &std::ops::Range<u32>) -> u32 {
    candidate.end - candidate.start
}

/// Return whether `candidate` covers a valid `target` range.
pub(super) const fn candidate_covers_target(
    candidate: &std::ops::Range<u32>,
    target: &std::ops::Range<u32>,
) -> bool {
    candidate.end >= candidate.start
        && target.end >= target.start
        && candidate.start <= target.start
        && candidate.end >= target.end
}

/// Decide whether production selection should replace its current candidate.
///
/// Sharing this predicate keeps the parser adapter's pruned traversal and the
/// parser-independent Kani helper on the same coverage and minimal-width
/// rules. Equal-width candidates retain the first candidate encountered.
pub(super) fn should_select_covering_candidate(
    candidate: &std::ops::Range<u32>,
    target: &std::ops::Range<u32>,
    selected_width: Option<u32>,
) -> bool {
    candidate_covers_target(candidate, target)
        && selected_width.is_none_or(|width| candidate.end - candidate.start < width)
}

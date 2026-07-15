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
    if target.end < target.start {
        return None;
    }

    candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.end >= candidate.start
                && candidate.start <= target.start
                && candidate.end >= target.end
        })
        .min_by_key(|(_, candidate)| candidate.end - candidate.start)
        .map(|(index, _)| index)
}

//! Recognize the `cfg(test)` gate that survives into a late lint pass.
//!
//! `#[cfg(...)]` does not reach a lint as written. The compiler evaluates it
//! during expansion and leaves a parsed `CfgTrace` attribute in its place, so
//! a lint reading only unparsed attributes sees no `cfg(test)` anywhere and
//! treats every test module's contents as production code.
//!
//! Shared rather than duplicated: two lints carried the same private copy of
//! this decision, so the same defect existed twice and was found only when
//! three consuming repositories went red.

use rustc_hir as hir;
use rustc_hir::attrs::CfgEntry;
use rustc_span::sym;

/// Whether a `cfg_trace` attribute makes its item test-only.
///
/// # Parameters
///
/// - `attr`: The HIR attribute to inspect.
///
/// # Returns
///
/// Whether the attribute leaves its item out of every non-test build.
///
/// # Examples
///
/// ```ignore
/// if whitaker::hir::cfg_trace_gates_on_test(attr) {
///     // The item exists only when compiling tests.
/// }
/// ```
#[must_use]
pub fn cfg_trace_gates_on_test(attr: &hir::Attribute) -> bool {
    let hir::Attribute::Parsed(hir::attrs::AttributeKind::CfgTrace(entries)) = attr else {
        return false;
    };
    entries.iter().any(|(entry, _)| gates_on_test(entry))
}

/// Whether a predicate is unsatisfiable when `test` is false.
///
/// The question is not "does `test` appear" but "can this item exist in a
/// build that is not a test build". `any(test, feature = "x")` mentions
/// `test` and is still present in a production build with `x` enabled, so
/// treating it as test-only would exempt production code from a lint.
fn gates_on_test(entry: &CfgEntry) -> bool {
    !can_hold_without_test(entry)
}

/// Whether the predicate could be true in a build where `test` is false.
///
/// Every predicate other than `test` is treated as unknown and assumed
/// satisfiable, because a production build may enable any feature or target.
/// The conservative direction is deliberate: guessing that a predicate is
/// unsatisfiable would exempt code the lint should examine.
fn can_hold_without_test(entry: &CfgEntry) -> bool {
    match entry {
        CfgEntry::NameValue { name, .. } => *name != sym::test,
        CfgEntry::Bool(value, _) => *value,
        CfgEntry::Version(..) => true,
        CfgEntry::Not(inner, _) => can_be_false_without_test(inner),
        CfgEntry::All(entries, _) => entries.iter().all(can_hold_without_test),
        CfgEntry::Any(entries, _) => entries.iter().any(can_hold_without_test),
    }
}

/// Whether the predicate could be false in a build where `test` is false.
///
/// The counterpart needed for negation: `not(p)` holds without `test` exactly
/// when `p` can be false without `test`.
fn can_be_false_without_test(entry: &CfgEntry) -> bool {
    match entry {
        // With `test` false, `test` is false; any other name may be unset, and
        // a version predicate may be unmet. All three can be false without
        // `test`, so they share an arm.
        CfgEntry::NameValue { .. } | CfgEntry::Version(..) => true,
        CfgEntry::Bool(value, _) => !*value,
        CfgEntry::Not(inner, _) => can_hold_without_test(inner),
        CfgEntry::All(entries, _) => entries.iter().any(can_be_false_without_test),
        CfgEntry::Any(entries, _) => entries.iter().all(can_be_false_without_test),
    }
}

#[cfg(test)]
#[path = "cfg_trace_tests.rs"]
mod tests;

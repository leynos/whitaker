//! Classifies `std::fs` usages encountered by the lint into diagnostic inputs.

use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_lint::LateContext;
use rustc_span::sym;
use whitaker_common::SimplePath;

/// Normalized view of a `std::fs` operation for diagnostics and tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdFsUsage {
    operation: String,
}

impl StdFsUsage {
    /// Store the resolved operation label used by diagnostics.
    ///
    /// No usage category is retained; diagnostics are based on this label.
    #[must_use]
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use crate::usage::StdFsUsage;
    /// let usage = StdFsUsage::new(String::from("std::fs::read"));
    /// assert_eq!(usage.operation(), "std::fs::read");
    /// ```
    pub fn new(operation: String) -> Self {
        Self { operation }
    }

    /// Returns the fully qualified operation path (e.g., `std::fs::read`).
    #[must_use]
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use crate::usage::StdFsUsage;
    /// let usage = StdFsUsage::new(String::from("std::fs::remove_file"));
    /// assert_eq!(usage.operation(), "std::fs::remove_file");
    /// ```
    pub fn operation(&self) -> &str {
        &self.operation
    }
}

/// Classify a resolved path (expression, type, or import) into a usage record.
///
/// Return `None` when HIR resolution has no definition or the resolved item is
/// not a `std::fs` path.
#[must_use]
///
/// # Examples
///
/// ```ignore
/// # use rustc_hir as hir;
/// # use rustc_lint::LateContext;
/// # use crate::usage::classify_qpath;
/// # fn example<'tcx>(cx: &LateContext<'tcx>, qpath: &hir::QPath<'tcx>, hir_id: hir::HirId) {
/// let _ = classify_qpath(cx, qpath, hir_id);
/// # }
/// ```
pub fn classify_qpath(
    cx: &LateContext<'_>,
    qpath: &hir::QPath<'_>,
    hir_id: hir::HirId,
) -> Option<StdFsUsage> {
    let res = cx.qpath_res(qpath, hir_id);
    classify_res(cx, res)
}

/// Classify using a `Res` obtained from HIR traversal.
///
/// Return `None` when the resolution has no `DefId` or does not identify a
/// `std::fs` item.
#[must_use]
///
/// # Examples
///
/// ```ignore
/// # use rustc_hir::def::Res;
/// # use rustc_lint::LateContext;
/// # use crate::usage::classify_res;
/// # fn example<'tcx>(cx: &LateContext<'tcx>, res: Res) {
/// let _ = classify_res(cx, res);
/// # }
/// ```
pub fn classify_res(cx: &LateContext<'_>, res: Res) -> Option<StdFsUsage> {
    res.opt_def_id()
        .and_then(|def_id| classify_def_id(cx, def_id))
}

/// Classify a `DefId` by inspecting its fully qualified path.
///
/// Return `None` when the definition belongs to a crate other than `std` or
/// its fully qualified path is outside `std::fs`.
#[must_use]
///
/// # Examples
///
/// ```ignore
/// # use rustc_hir::def_id::DefId;
/// # use rustc_lint::LateContext;
/// # use crate::usage::classify_def_id;
/// # fn example<'tcx>(cx: &LateContext<'tcx>, def_id: DefId) {
/// let _ = classify_def_id(cx, def_id);
/// # }
/// ```
pub fn classify_def_id(cx: &LateContext<'_>, def_id: DefId) -> Option<StdFsUsage> {
    if cx.tcx.crate_name(def_id.krate) != sym::std {
        return None;
    }

    let label = cx.tcx.def_path_str(def_id);

    label_is_std_fs(&label).then(|| StdFsUsage::new(label))
}

/// Return whether a parsed path is rooted at `std::fs`.
///
/// The first two segments must be exactly `std` and `fs`; deeper segments are
/// allowed, while shorter or differently rooted paths return `false`.
fn is_std_fs_path(path: &SimplePath) -> bool {
    let segments = path.segments();
    segments.len() >= 2 && segments[0] == "std" && segments[1] == "fs"
}

/// Returns true if the character should be rejected in a valid std::fs label.
fn is_invalid_label_char(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '(' | ')')
}

/// Validate a resolved definition label as `std::fs` or one of its children.
///
/// Leading/trailing whitespace, empty labels, whitespace or parentheses
/// within the label, and partial-prefix matches are rejected.
pub(crate) fn label_is_std_fs(label: &str) -> bool {
    if label != label.trim() {
        return false;
    }

    if label.is_empty() || label.chars().any(is_invalid_label_char) {
        return false;
    }

    if !label.starts_with("std::fs") {
        return false;
    }

    let remainder = &label["std::fs".len()..];
    if remainder.is_empty() {
        return true;
    }

    if !remainder.starts_with("::") {
        return false;
    }

    let path = SimplePath::parse(label);
    is_std_fs_path(&path)
}

#[cfg(test)]
mod tests;

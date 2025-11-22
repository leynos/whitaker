//! Classifies `std::fs` usages encountered by the lint into diagnostic inputs.

use common::SimplePath;
use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_lint::LateContext;
use rustc_span::sym;

/// Category describing how the `std::fs` item is being used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsageCategory {
    /// `use std::fs::{..}` imports.
    Import,
    /// Type positions referencing `std::fs` types (structs, aliases).
    Type,
    /// Value-level calls, struct literals, or method invocations.
    Call,
}

impl UsageCategory {
    /// Returns a stable &str identifier for use in tests and localisation.
    #[cfg(test)]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Type => "type",
            Self::Call => "call",
        }
    }
}

/// Normalised view of a `std::fs` operation for diagnostics and tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdFsUsage {
    operation: String,
    category: UsageCategory,
}

impl StdFsUsage {
    /// Construct a new usage instance.
    #[must_use]
    pub fn new(operation: String, category: UsageCategory) -> Self {
        Self {
            operation,
            category,
        }
    }

    /// Returns the fully qualified operation path (e.g., `std::fs::read`).
    #[must_use]
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Returns the usage category.
    #[cfg(test)]
    #[must_use]
    pub const fn category(&self) -> UsageCategory {
        self.category
    }
}

/// Classify a resolved path (expression, type, import) into a usage record.
#[must_use]
pub fn classify_qpath(
    cx: &LateContext<'_>,
    qpath: &hir::QPath<'_>,
    hir_id: hir::HirId,
    category: UsageCategory,
) -> Option<StdFsUsage> {
    let res = cx.qpath_res(qpath, hir_id);
    classify_res(cx, res, category)
}

/// Classify using a `Res` obtained from HIR traversal.
#[must_use]
pub fn classify_res(cx: &LateContext<'_>, res: Res, category: UsageCategory) -> Option<StdFsUsage> {
    res.opt_def_id()
        .and_then(|def_id| classify_def_id(cx, def_id, category))
}

/// Classify a `DefId` by inspecting its fully qualified path.
#[must_use]
pub fn classify_def_id(
    cx: &LateContext<'_>,
    def_id: DefId,
    category: UsageCategory,
) -> Option<StdFsUsage> {
    if cx.tcx.crate_name(def_id.krate) != sym::std {
        return None;
    }

    let label = cx.tcx.def_path_str(def_id);

    label_is_std_fs(&label).then(|| StdFsUsage::new(label, category))
}

fn is_std_fs_path(path: &SimplePath) -> bool {
    let segments = path.segments();
    segments.len() >= 2 && segments[0] == "std" && segments[1] == "fs"
}

pub(crate) fn label_is_std_fs(label: &str) -> bool {
    if label != label.trim() {
        return false;
    }

    if label.is_empty()
        || label
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '(' | ')'))
    {
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

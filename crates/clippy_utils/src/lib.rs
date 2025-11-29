//! Minimal `clippy_utils` stub exposing panic detection helpers.
//!
//! The real Clippy utilities crate depends on internal rustc crates that are
//! not bundled in this workspace. This lightweight replacement provides the
//! `macros::is_panic` helper required by Whitaker lints without pulling in
//! additional compiler infrastructure.

#![feature(rustc_private)]

pub mod macros {
    use rustc_hir as hir;
    use rustc_lint::LateContext;

    // Panic entry points mirrored from the main lint panic detector to avoid
    // brittle substring heuristics.
    const PANIC_PATHS: &[&str] = &[
        "core::panicking::panic",
        "core::panicking::panic_fmt",
        "core::panicking::panic_any",
        "core::panicking::begin_panic",
        "std::panicking::panic",
        "std::panicking::panic_fmt",
        "std::panicking::panic_any",
        "std::panicking::begin_panic",
    ];

    /// Best-effort panic detection mirroring Clippy's helper.
    #[must_use]
    pub fn is_panic(cx: &LateContext<'_>, expr: &hir::Expr<'_>) -> bool {
        let hir::ExprKind::Call(callee, _) = expr.kind else {
            return false;
        };

        let def_id = cx
            .typeck_results()
            .type_dependent_def_id(callee.hir_id)
            .or_else(|| match callee.kind {
                hir::ExprKind::Path(qpath) => cx.qpath_res(&qpath, callee.hir_id).opt_def_id(),
                _ => None,
            });

        let Some(def_id) = def_id else {
            return false;
        };

        let path = cx.tcx.def_path_str(def_id);
        PANIC_PATHS
            .iter()
            .any(|candidate| path.as_str() == *candidate)
    }
}

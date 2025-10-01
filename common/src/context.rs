use crate::attrs;
use rustc_hir::{HirId, ItemKind, Node};
use rustc_lint::LateContext;
use rustc_span::sym;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextSignals {
    pub has_test_attr: bool,
    pub has_cfg_test: bool,
    pub ancestor_cfg_test: bool,
}

impl ContextSignals {
    #[must_use]
    pub fn is_test_like(self) -> bool {
        self.has_test_attr || self.has_cfg_test || self.ancestor_cfg_test
    }
}

#[must_use]
pub fn signals_for<'tcx>(cx: &LateContext<'tcx>, hir_id: HirId) -> ContextSignals {
    let owner_def_id = cx.tcx.hir_enclosing_body_owner(hir_id);
    let owner_hir_id = cx.tcx.local_def_id_to_hir_id(owner_def_id);
    let attrs_slice = cx.tcx.hir_attrs(owner_hir_id);
    let has_test_attr = attrs::has_test_marker(attrs_slice);
    let has_cfg_test = attrs::has_cfg_test(attrs_slice);
    let ancestor_cfg_test = cx
        .tcx
        .hir_parent_iter(owner_hir_id)
        .any(|(parent_id, _)| attrs::has_cfg_test(cx.tcx.hir_attrs(parent_id)));

    ContextSignals {
        has_test_attr,
        has_cfg_test,
        ancestor_cfg_test,
    }
}

#[must_use]
pub fn in_test_like_context<'tcx>(cx: &LateContext<'tcx>, hir_id: HirId) -> bool {
    signals_for(cx, hir_id).is_test_like()
}

#[must_use]
pub fn is_test_fn<'tcx>(cx: &LateContext<'tcx>, hir_id: HirId) -> bool {
    let owner_def_id = cx.tcx.hir_enclosing_body_owner(hir_id);
    let owner_hir_id = cx.tcx.local_def_id_to_hir_id(owner_def_id);
    attrs::has_test_marker(cx.tcx.hir_attrs(owner_hir_id))
}

#[must_use]
pub fn is_in_main_fn<'tcx>(cx: &LateContext<'tcx>, hir_id: HirId) -> bool {
    let owner_def_id = cx.tcx.hir_enclosing_body_owner(hir_id);
    let owner_hir_id = cx.tcx.local_def_id_to_hir_id(owner_def_id);

    if let Node::Item(item) = cx.tcx.hir_node(owner_hir_id) {
        if let ItemKind::Fn { .. } = item.kind {
            let (ident, _, _, _) = item.expect_fn();
            return ident.name == sym::main;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(true, false, false, true)]
    #[case(false, true, false, true)]
    #[case(false, false, true, true)]
    #[case(false, false, false, false)]
    fn evaluates_test_like(
        #[case] has_test: bool,
        #[case] has_cfg: bool,
        #[case] ancestor_cfg: bool,
        #[case] expected: bool,
    ) {
        let signals = ContextSignals {
            has_test_attr: has_test,
            has_cfg_test: has_cfg,
            ancestor_cfg_test: ancestor_cfg,
        };
        assert_eq!(signals.is_test_like(), expected);
    }
}

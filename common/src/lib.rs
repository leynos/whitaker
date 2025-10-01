#![feature(rustc_private)]
#![deny(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

pub mod attrs;
pub mod config;
pub mod context;
pub mod diagnostics;
pub mod span;
#[cfg(any(test, feature = "dylint"))]
pub mod testing;

pub use attrs::{
    AttributeOrderViolation, doc_attrs, ensure_doc_attrs_first, first_non_doc_attr, has_cfg_test,
    has_doc_attr, has_test_marker, is_doc_attr, is_inner_doc, is_outer_doc, non_doc_attrs,
};
pub use config::{decode_json_or_default, load_or_default};
pub use context::{ContextSignals, in_test_like_context, is_in_main_fn, is_test_fn, signals_for};
pub use diagnostics::{span_lint, span_lint_and_help, span_lint_and_sugg};
pub use span::{
    LineRange, def_id_of_expr_callee, is_path_to, module_line_count, span_to_line_range,
};
#[cfg(any(test, feature = "dylint"))]
pub use testing::{UiTestHarness, default_ui_harness, harness};

#[cfg(test)]
mod tests;

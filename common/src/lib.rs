#![deny(clippy::expect_used, clippy::unwrap_used)]

pub mod attributes;
pub mod context;
pub mod diagnostics;
pub mod span;

pub use attributes::{
    Attribute, AttributeKind, AttributePath, has_test_like_attribute, outer_attributes,
    split_doc_attributes,
};
pub use context::{ContextEntry, ContextKind, in_test_like_context, is_in_main_fn, is_test_fn};
pub use diagnostics::{Applicability, Diagnostic, DiagnosticBuilder, Suggestion, span_lint};
pub use span::{
    Expr, SimplePath, SourceLocation, SourceSpan, SpanError, def_id_of_expr_callee, is_path_to,
    module_line_count, recv_is_option_or_result, span_to_lines,
};

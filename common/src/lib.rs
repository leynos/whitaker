//! Shared lint infrastructure providing attribute helpers, context tracking,
//! path, expression, span, and diagnostic utilities for Whitaker lints.

pub mod attributes;
pub mod complexity_signal;
pub mod context;
pub mod diagnostics;
pub mod expr;
pub mod i18n;
pub mod path;
pub mod span;
pub mod test_support;

pub use attributes::{
    Attribute, AttributeKind, AttributePath, PARSED_ATTRIBUTE_PLACEHOLDER, has_test_like_attribute,
    has_test_like_attribute_with, outer_attributes, split_doc_attributes,
};
pub use context::{
    ContextEntry, ContextKind, in_test_like_context, in_test_like_context_with, is_in_main_fn,
    is_test_fn, is_test_fn_with,
};
pub use diagnostics::{Applicability, Diagnostic, DiagnosticBuilder, Suggestion, span_lint};
pub use expr::{Expr, def_id_of_expr_callee, is_path_to, recv_is_option_or_result};
pub use i18n::{
    Arguments, FALLBACK_LOCALE, I18nError, LocaleSelection, LocaleSource, Localizer,
    MessageResolution, available_locales, branch_phrase, get_localizer_for_lint, noop_reporter,
    normalise_locale, resolve_localizer, safe_resolve_message_set, supports_locale,
};
pub use path::SimplePath;
pub use span::{SourceLocation, SourceSpan, SpanError, span_line_count, span_to_lines};

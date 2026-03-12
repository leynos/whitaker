//! Shared lint infrastructure providing attribute helpers, context tracking,
//! path, expression, span, diagnostic, cohesion analysis, and brain
//! type/trait metric collection utilities for Whitaker lints.

pub mod attributes;
pub mod brain_trait_metrics;
pub mod brain_type_metrics;
pub mod complexity_signal;
pub mod context;
pub mod decomposition_advice;
pub mod diagnostics;
pub mod expr;
pub mod i18n;
pub mod lcom4;
pub mod path;
pub mod span;
pub mod test_support;

pub use attributes::{
    Attribute, AttributeKind, AttributePath, PARSED_ATTRIBUTE_PLACEHOLDER, has_test_like_attribute,
    has_test_like_attribute_with, outer_attributes, split_doc_attributes,
};
pub use brain_trait_metrics::evaluation::{
    BrainTraitDiagnostic, BrainTraitDisposition, BrainTraitThresholds, BrainTraitThresholdsBuilder,
    evaluate_brain_trait,
};
pub use brain_trait_metrics::{
    TraitItemKind, TraitItemMetrics, TraitMetrics, TraitMetricsBuilder, default_method_cc_sum,
    default_method_count, required_method_count, trait_item_count,
};
pub use brain_type_metrics::evaluation::{
    BrainTypeDiagnostic, BrainTypeDisposition, BrainTypeThresholds, BrainTypeThresholdsBuilder,
    evaluate_brain_type, format_help, format_note, format_primary_message,
};
pub use brain_type_metrics::{
    CognitiveComplexityBuilder, ForeignReferenceSet, MethodMetrics, TypeMetrics,
    TypeMetricsBuilder, brain_methods, foreign_reach_count, weighted_methods_count,
};
pub use context::{
    ContextEntry, ContextKind, in_test_like_context, in_test_like_context_with, is_in_main_fn,
    is_test_fn, is_test_fn_with,
};
pub use decomposition_advice::{
    DecompositionContext, DecompositionSuggestion, MethodProfile, MethodProfileBuilder,
    SubjectKind, SuggestedExtractionKind, suggest_decomposition,
};
pub use diagnostics::{Applicability, Diagnostic, DiagnosticBuilder, Suggestion, span_lint};
pub use expr::{Expr, def_id_of_expr_callee, is_path_to, recv_is_option_or_result};
pub use i18n::{
    Arguments, FALLBACK_LOCALE, I18nError, LocaleSelection, LocaleSource, Localizer,
    MessageResolution, available_locales, branch_phrase, get_localizer_for_lint, noop_reporter,
    normalise_locale, resolve_localizer, safe_resolve_message_set, supports_locale,
};
pub use lcom4::{MethodInfo, MethodInfoBuilder, cohesion_components, collect_method_infos};
pub use path::SimplePath;
pub use span::{SourceLocation, SourceSpan, SpanError, span_line_count, span_to_lines};

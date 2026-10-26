//! Classifies continuations by source context and the content around each escape.

use crate::{
    continuation::{Continuation, ContinuationKind},
    facts::{CaptureStatus, LiteralContext, LiteralFacts},
    rewrite::ConcatRewrite,
};

#[cfg(test)]
mod tests;

/// The policy outcome for a literal that may contain continuations.
///
/// # Examples
///
/// ```
/// use whitaker_string_literals::{
///     classification::{ContinuationDisposition, classify},
///     facts::{LiteralBody, LiteralContext, LiteralFacts},
/// };
///
/// let facts = LiteralFacts::Cooked {
///     body: LiteralBody::new("left\\\nright"),
///     context: LiteralContext::Expression,
/// };
/// assert!(matches!(
///     classify(&facts),
///     ContinuationDisposition::PreferConcat(_)
/// ));
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinuationDisposition<'a> {
    /// A proven rewrite. Construction of `ConcatRewrite` is the proof.
    PreferConcat(ConcatRewrite<'a>),
    /// The source continuation is needed to preserve its surrounding contract.
    RequireContinuation(RequiredReason),
    /// No eligible source continuation can be diagnosed.
    Ignore,
}

/// A reason the original continuation must remain in source.
///
/// # Examples
///
/// ```
/// use whitaker_string_literals::classification::RequiredReason;
///
/// assert_eq!(
///     RequiredReason::ImplicitFormatCapture,
///     RequiredReason::ImplicitFormatCapture
/// );
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequiredReason {
    /// Replacing the format string would disable implicit named capture.
    ImplicitFormatCapture,
    /// `concat!` cannot preserve a byte-string or C-string literal type.
    NonStringLiteralType,
    /// The continuation trims leading or trailing source layout.
    LeadingOrTrailingLayoutTrim,
    /// The source file is generated or outside the editable workspace.
    GeneratedSourceFile,
}

/// Classifies facts about a literal using the pure continuation policy.
///
/// Only a validated rewrite produces `PreferConcat`. If a source body cannot
/// be represented by the rewrite gate, it is ignored rather than suggesting
/// text that has not passed the lexical checks.
///
/// # Examples
///
/// ```
/// use whitaker_string_literals::{
///     classification::{ContinuationDisposition, classify},
///     facts::{LiteralBody, LiteralContext, LiteralFacts},
/// };
///
/// let facts = LiteralFacts::Cooked {
///     body: LiteralBody::new("left\\\nright"),
///     context: LiteralContext::Expression,
/// };
/// assert!(matches!(
///     classify(&facts),
///     ContinuationDisposition::PreferConcat(_)
/// ));
/// ```
#[must_use]
pub fn classify<'a>(facts: &LiteralFacts<'a>) -> ContinuationDisposition<'a> {
    let (body, context) = match facts {
        LiteralFacts::Cooked { body, context } => (*body, *context),
        LiteralFacts::NonStringLiteral => {
            return ContinuationDisposition::RequireContinuation(
                RequiredReason::NonStringLiteralType,
            );
        }
        LiteralFacts::Raw => return ContinuationDisposition::Ignore,
    };

    let continuations = body.scan();
    if continuations.is_empty() {
        return ContinuationDisposition::Ignore;
    }

    match context {
        LiteralContext::GeneratedFormatString => return ContinuationDisposition::Ignore,
        LiteralContext::GeneratedSourceFile => {
            return ContinuationDisposition::RequireContinuation(
                RequiredReason::GeneratedSourceFile,
            );
        }
        LiteralContext::SourceFormatString {
            capture: CaptureStatus::Present,
        } => {
            return ContinuationDisposition::RequireContinuation(
                RequiredReason::ImplicitFormatCapture,
            );
        }
        LiteralContext::Expression
        | LiteralContext::SourceFormatString {
            capture: CaptureStatus::NoneObserved,
        } => {}
    }

    let joins: Vec<Continuation> = continuations
        .into_iter()
        .filter(|continuation| continuation.kind == ContinuationKind::Join)
        .collect();
    if joins.is_empty() {
        return ContinuationDisposition::RequireContinuation(
            RequiredReason::LeadingOrTrailingLayoutTrim,
        );
    }

    ConcatRewrite::build(body, &joins).map_or(ContinuationDisposition::Ignore, |rewrite| {
        ContinuationDisposition::PreferConcat(rewrite)
    })
}

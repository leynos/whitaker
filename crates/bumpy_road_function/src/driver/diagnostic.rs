//! Diagnostic emission for the bumpy road lint.
//!
//! The lint warns when it detects two or more separated bump intervals in the
//! smoothed signal and highlights the two most severe bumps.

use std::{borrow::Cow, ops::RangeInclusive};

use fluent_templates::fluent_bundle::FluentValue;
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};
use whitaker_common::{
    Arguments,
    Localizer,
    MessageResolution,
    i18n::DiagnosticMessageSet,
    noop_reporter,
    safe_resolve_message_set,
};

use super::{
    BUMPY_ROAD_FUNCTION,
    LINT_NAME,
    MESSAGE_KEY,
    numeric::{diagnostic_integer_argument, line_range_byte_offsets},
};
use crate::analysis::{BumpInterval, Settings, top_two_bumps};

/// Payload describing a lint diagnostic to emit.
pub(super) struct DiagnosticInput<'a> {
    pub(super) name: &'a str,
    pub(super) primary_span: Span,
    pub(super) body_span: Span,
    pub(super) function_lines: RangeInclusive<usize>,
    pub(super) bumps: Vec<BumpInterval>,
    pub(super) settings: &'a Settings,
}

/// Emit the localized lint message and labels for the two most severe bumps.
///
/// Missing source snippets omit the affected labels, and oversized numeric
/// arguments are saturated before they are passed to Fluent.
pub(super) fn emit_diagnostic(
    cx: &LateContext<'_>,
    input: DiagnosticInput<'_>,
    localizer: &Localizer,
) {
    let args = diagnostic_arguments(input.name, input.bumps.len(), input.settings.threshold);

    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &args,
    };
    let messages = safe_resolve_message_set(localizer, resolution, noop_reporter, || {
        fallback_messages(input.name, input.bumps.len(), input.settings.threshold)
    });

    let highlighted = top_two_bumps(input.bumps);
    let bump_spans = build_bump_spans(cx, input.body_span, &input.function_lines, &highlighted);

    cx.emit_span_lint(
        BUMPY_ROAD_FUNCTION,
        input.primary_span,
        rustc_lint::errors::DiagDecorator(|lint| {
            lint.primary_message(messages.primary().to_owned());
            lint.span_note(input.primary_span, messages.note().to_owned());

            for (ordinal, interval) in highlighted.iter().enumerate() {
                let Some(span) = bump_spans.get(ordinal).copied().flatten() else {
                    continue;
                };
                let label = resolve_bump_label(
                    localizer,
                    diagnostic_integer_argument(ordinal.saturating_add(1)),
                    diagnostic_integer_argument(interval.len()),
                );
                lint.span_label(span, label);
            }

            lint.help(messages.help().to_owned());
        }),
    );
}

/// Build the localized arguments passed by the production diagnostic path.
fn diagnostic_arguments(name: &str, bump_count: usize, threshold: f64) -> Arguments<'_> {
    let mut args: Arguments<'_> = Arguments::default();
    args.insert(Cow::Borrowed("name"), FluentValue::from(name));
    args.insert(
        Cow::Borrowed("count"),
        FluentValue::from(diagnostic_integer_argument(bump_count)),
    );
    args.insert(Cow::Borrowed("threshold"), FluentValue::from(threshold));
    args
}

fn build_bump_spans(
    cx: &LateContext<'_>,
    body_span: Span,
    function_lines: &RangeInclusive<usize>,
    highlighted: &[BumpInterval],
) -> Vec<Option<Span>> {
    let source_map = cx.tcx.sess.source_map();
    let Ok(snippet) = source_map.span_to_snippet(body_span) else {
        return vec![None; highlighted.len()];
    };

    let line_starts = line_start_offsets(&snippet);
    let body_start_line = *function_lines.start();
    let mapper = LineSpanMapper::new(body_span, snippet.len(), body_start_line, line_starts);

    highlighted
        .iter()
        .map(|interval| {
            let start_line = body_start_line + interval.start_index();
            let end_line = body_start_line + interval.end_index();
            mapper.span_for_range(start_line, end_line)
        })
        .collect()
}

fn line_start_offsets(snippet: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in snippet.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

/// Translates 1-based source line ranges into byte-offset `Span`s.
///
/// The span mapper operates on in-memory snippets, so byte offsets are bounded
/// by the file size. `rustc_span::BytePos` uses `u32`, which is sufficient for
/// typical source files used with this lint (and matches the compiler's span
/// representation).
struct LineSpanMapper {
    base_span: Span,
    snippet_len: usize,
    base_line: usize,
    line_starts: Vec<usize>,
}

impl LineSpanMapper {
    const fn new(
        base_span: Span,
        snippet_len: usize,
        base_line: usize,
        line_starts: Vec<usize>,
    ) -> Self {
        Self {
            base_span,
            snippet_len,
            base_line,
            line_starts,
        }
    }

    /// Map an inclusive source-line range into the snippet's source span.
    ///
    /// Returns `None` when the start line is outside the snippet, the range is
    /// reversed, or a byte offset cannot fit the compiler's `u32` positions.
    fn span_for_range(&self, start_line: usize, end_line: usize) -> Option<Span> {
        let base = self.base_span.shrink_to_lo();
        // `BytePos` is `u32`-backed; offsets beyond 4 GiB cannot be represented
        // as spans, so such (pathological) snippets yield no span.
        let (start_offset, end_offset) = line_range_byte_offsets(
            &self.line_starts,
            self.snippet_len,
            self.base_line,
            start_line..=end_line,
        )?;
        let lo = base.lo() + BytePos(start_offset);
        let mut hi = base.lo() + BytePos(end_offset);
        if hi <= lo {
            hi = lo + BytePos(1);
        }
        Some(base.with_lo(lo).with_hi(hi))
    }
}

fn resolve_bump_label(localizer: &Localizer, index: i64, lines: i64) -> String {
    let mut args: Arguments<'_> = Arguments::default();
    args.insert(Cow::Borrowed("index"), FluentValue::from(index));
    args.insert(Cow::Borrowed("lines"), FluentValue::from(lines));

    // Fluent may inject bidi-safe directional isolates. We strip the resulting
    // control characters (plus replacement characters) to keep output stable in
    // diagnostics and UI golden files.
    localizer
        .attribute_with_args(LINT_NAME, "label", &args)
        .unwrap_or_else(|_| format!("Complexity bump {index} spans {lines} lines."))
        .chars()
        .filter(|ch| !matches!(ch, '\u{2068}' | '\u{2069}' | '\u{FFFD}'))
        .collect()
}

fn fallback_messages(name: &str, count: usize, threshold: f64) -> DiagnosticMessageSet {
    DiagnosticMessageSet::new(
        format!("Multiple clusters of nested conditional logic in `{name}`."),
        format!("Detected {count} complexity bumps above the threshold {threshold}."),
        String::from(
            "Extract helper functions from the highlighted regions to reduce clustered complexity.",
        ),
    )
}

#[cfg(test)]
mod tests {
    //! Verify diagnostic construction and compiler span bounds.

    use fluent_templates::fluent_bundle::FluentValue;
    use rustc_span::DUMMY_SP;

    use super::{LineSpanMapper, diagnostic_arguments};

    /// Verify oversized bump counts saturate in the emitted diagnostic arguments.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn diagnostic_arguments_saturate_oversized_bump_counts() {
        let oversized = usize::try_from(u64::try_from(i64::MAX).expect("i64 max fits u64") + 1)
            .expect("oversized bump count fits usize");

        let args = diagnostic_arguments("example", oversized, 0.5);

        assert_eq!(args.get("count"), Some(&FluentValue::from(i64::MAX)));
    }

    /// Verify span construction omits offsets that rustc cannot represent.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn span_mapping_omits_unrepresentable_offsets() {
        let oversized =
            usize::try_from(u64::from(u32::MAX) + 1).expect("oversized span offset fits usize");
        let mapper = LineSpanMapper::new(DUMMY_SP, oversized, 1, vec![0, oversized]);

        assert_eq!(mapper.span_for_range(1, 1), None);
    }
}

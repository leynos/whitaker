//! Diagnostic emission for the bumpy road lint.
//!
//! The lint warns when it detects two or more separated bump intervals in the
//! smoothed signal and highlights the two most severe bumps.

use std::borrow::Cow;
use std::ops::RangeInclusive;

use crate::analysis::{BumpInterval, Settings, top_two_bumps};
use common::i18n::DiagnosticMessageSet;
use common::{Arguments, Localizer, MessageResolution, safe_resolve_message_set};
use fluent_templates::fluent_bundle::FluentValue;
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};

use super::{BUMPY_ROAD_FUNCTION, LINT_NAME, MESSAGE_KEY};

/// Payload describing a lint diagnostic to emit.
pub(super) struct DiagnosticInput<'a> {
    pub(super) name: &'a str,
    pub(super) primary_span: Span,
    pub(super) body_span: Span,
    pub(super) function_lines: RangeInclusive<usize>,
    pub(super) bumps: Vec<BumpInterval>,
    pub(super) settings: &'a Settings,
}

pub(super) fn emit_diagnostic(
    cx: &LateContext<'_>,
    input: DiagnosticInput<'_>,
    localizer: &Localizer,
) {
    let mut args: Arguments<'_> = Arguments::default();
    args.insert(Cow::Borrowed("name"), FluentValue::from(input.name));
    args.insert(
        Cow::Borrowed("count"),
        FluentValue::from(input.bumps.len() as i64),
    );
    args.insert(
        Cow::Borrowed("threshold"),
        FluentValue::from(input.settings.threshold),
    );

    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &args,
    };
    let messages = safe_resolve_message_set(
        localizer,
        resolution,
        |_message| {},
        || fallback_messages(input.name, input.bumps.len(), input.settings.threshold),
    );

    let highlighted = top_two_bumps(input.bumps);
    let bump_spans = build_bump_spans(cx, input.body_span, &input.function_lines, &highlighted);

    cx.span_lint(BUMPY_ROAD_FUNCTION, input.primary_span, |lint| {
        lint.primary_message(messages.primary().to_string());
        lint.span_note(input.primary_span, messages.note().to_string());

        for (ordinal, interval) in highlighted.iter().enumerate() {
            let Some(span) = bump_spans.get(ordinal).copied().flatten() else {
                continue;
            };
            let label = resolve_bump_label(localizer, (ordinal + 1) as i64, interval.len() as i64);
            lint.span_label(span, label);
        }

        lint.help(messages.help().to_string());
    });
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
    fn new(base_span: Span, snippet_len: usize, base_line: usize, line_starts: Vec<usize>) -> Self {
        Self {
            base_span,
            snippet_len,
            base_line,
            line_starts,
        }
    }

    fn span_for_range(&self, start_line: usize, end_line: usize) -> Option<Span> {
        if start_line < self.base_line || end_line < start_line {
            return None;
        }

        let start_index = start_line - self.base_line;
        let end_index = end_line - self.base_line;

        let start_offset = *self.line_starts.get(start_index)?;
        let end_offset = self
            .line_starts
            .get(end_index + 1)
            .copied()
            .unwrap_or(self.snippet_len);

        let base = self.base_span.shrink_to_lo();
        // `BytePos` is `u32`-backed; the snippet length is expected to fit in
        // 4 GiB for any reasonable Rust source file.
        let lo = base.lo() + BytePos(start_offset as u32);
        let mut hi = base.lo() + BytePos(end_offset as u32);
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

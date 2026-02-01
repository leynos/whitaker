//! Require modules to begin with inner doc comments.
//!
//! `module_must_have_inner_docs` inspects every non-macro module and
//! verifies that the first inner attribute is a doc comment (`//!` or
//! `#![doc = "..."]`, including nested `cfg_attr` wrappers). Modules missing
//! such a comment, or placing other inner attributes before it, trigger a
//! diagnostic that nudges teams to document the module purpose at the top of
//! the file.
use std::borrow::Cow;

use common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    get_localizer_for_lint, noop_reporter, safe_resolve_message_set,
};
use log::debug;
use newt_hype::base_newtype;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext};
#[cfg(test)]
use rustc_span::DUMMY_SP;
use rustc_span::symbol::Ident;
use rustc_span::{BytePos, Span};
use whitaker::{SharedConfig, module_body_span, module_header_span};

mod inner_attr;
mod parser;

base_newtype!(StrWrapper);

pub type SourceSnippet<'a> = StrWrapper<&'a str>;
pub type AttributeBody<'a> = StrWrapper<&'a str>;
pub type ParseInput<'a> = StrWrapper<&'a str>;
pub type MetaList<'a> = StrWrapper<&'a str>;
pub type ModuleName<'a> = StrWrapper<&'a str>;

impl<'a> ParseInput<'a> {
    /// Returns the underlying string slice.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let input = ParseInput::from("example");
    /// assert_eq!(input.as_str(), "example");
    /// ```
    pub fn as_str(&self) -> &'a str {
        **self
    }
}

const LINT_NAME: &str = "module_must_have_inner_docs";
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);

dylint_linting::impl_late_lint! {
    pub MODULE_MUST_HAVE_INNER_DOCS,
    Warn,
    "modules must begin with an inner doc comment",
    ModuleMustHaveInnerDocs::default()
}

/// Lint pass enforcing leading inner doc comments on modules.
pub struct ModuleMustHaveInnerDocs {
    localizer: Localizer,
}

impl Default for ModuleMustHaveInnerDocs {
    fn default() -> Self {
        Self {
            localizer: Localizer::new(None),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for ModuleMustHaveInnerDocs {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        let (ident, module) = match item.kind {
            hir::ItemKind::Mod(ident, module) => (ident, module),
            _ => return,
        };

        if item.span.from_expansion() {
            debug!(
                target: LINT_NAME,
                "skipping module `{}` expanded from a macro", ident.name
            );
            return;
        }

        let module_body = module_body_span(cx, item, module);
        let disposition = detect_module_docs_in_span(cx, module_body);
        if disposition == ModuleDocDisposition::HasLeadingDoc {
            return;
        }

        let primary_span = match disposition {
            ModuleDocDisposition::HasLeadingDoc => return,
            ModuleDocDisposition::MissingDocs => module_body.shrink_to_lo(),
            ModuleDocDisposition::FirstInnerIsNotDoc(span) => span,
        };
        let header_span = module_header_span(item.span, ident.span);
        let context = ModuleDiagnosticContext {
            ident,
            primary_span,
            header_span,
        };

        emit_diagnostic(cx, &context, &self.localizer);
    }
}

/// Indicates whether a module satisfies the lint requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModuleDocDisposition {
    /// The module already begins with an inner doc comment.
    HasLeadingDoc,
    /// No inner attributes are present, so docs are missing entirely.
    MissingDocs,
    /// The first inner attribute is not a doc comment.
    FirstInnerIsNotDoc(Span),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LeadingContent {
    Doc,
    Missing,
    Misordered { offset: usize, len: usize },
}

fn classify_leading_content(snippet: SourceSnippet<'_>) -> LeadingContent {
    let (offset, rest) = parser::skip_leading_whitespace(ParseInput::from(*snippet));
    if rest.is_empty() {
        return LeadingContent::Missing;
    }
    if parser::is_doc_comment(rest) {
        return LeadingContent::Doc;
    }
    check_attribute_order(rest, offset)
}

/// Classifies the leading content after whitespace has been stripped.
///
/// The caller (`classify_leading_content`) strips leading whitespace before
/// invoking this function, so `rest` begins with the first non-whitespace
/// character. Block comments preceding attributes are not stripped; if the
/// module body starts with `/* comment */ #[attr]`, the content is classified
/// as `Missing` because the first token is a comment, not an attribute.
///
/// Span lengths exclude trailing newlines to match rustc's standard diagnostic
/// highlighting behaviour for single-line constructs.
fn check_attribute_order(rest: ParseInput<'_>, offset: usize) -> LeadingContent {
    if rest.starts_with("#[") {
        let len = rest.find(['\n', '\r']).unwrap_or(rest.len());
        return LeadingContent::Misordered { offset, len };
    }
    if !rest.starts_with('#') {
        return LeadingContent::Missing;
    }

    if inner_attr::is_case_incorrect_doc_inner_attr(rest) {
        return LeadingContent::Missing;
    }

    if inner_attr::is_cfg_attr_without_doc(rest) {
        return LeadingContent::Missing;
    }

    if !has_inner_doc(rest) {
        return LeadingContent::Missing;
    }

    let len = rest.find(['\n', '\r']).unwrap_or(rest.len());
    LeadingContent::Misordered { offset, len }
}

fn has_inner_doc(rest: ParseInput<'_>) -> bool {
    let snippet = rest.as_str();
    let mut line_start = 0;

    while line_start < snippet.len() {
        let line_end = snippet[line_start..]
            .find('\n')
            .map(|idx| line_start + idx)
            .unwrap_or(snippet.len());
        let line = &snippet[line_start..line_end];
        if check_line_for_inner_doc(snippet, line, line_start) {
            return true;
        }

        line_start = line_end.saturating_add(1);
    }

    false
}

/// Reports whether a line contains an inner doc marker.
///
/// `snippet` is the full text so we can slice from the computed offset when
/// delegating to the parser. `line` is the current line slice, and
/// `line_start` is the byte offset of that line within `snippet`.
fn check_line_for_inner_doc(snippet: &str, line: &str, line_start: usize) -> bool {
    let (offset, trimmed) = parser::skip_leading_whitespace(ParseInput::from(line));
    if parser::is_doc_comment(trimmed) {
        return true;
    }

    let mut search_start = offset;
    if trimmed.starts_with("#!") {
        search_start = offset.saturating_add(2);
    }

    while let Some(local_idx) = line[search_start..].find("#!") {
        let absolute_idx = search_start + local_idx;
        let offset = line_start + absolute_idx;
        if parser::is_doc_comment(ParseInput::from(&snippet[offset..])) {
            return true;
        }
        search_start = absolute_idx + 2;
    }

    false
}

#[cfg(test)]
fn detect_module_docs_from_snippet(snippet: SourceSnippet<'_>) -> ModuleDocDisposition {
    match classify_leading_content(snippet) {
        LeadingContent::Doc => ModuleDocDisposition::HasLeadingDoc,
        LeadingContent::Missing => ModuleDocDisposition::MissingDocs,
        LeadingContent::Misordered { .. } => ModuleDocDisposition::FirstInnerIsNotDoc(DUMMY_SP),
    }
}

struct ModuleDiagnosticContext {
    ident: Ident,
    primary_span: Span,
    header_span: Span,
}

fn detect_module_docs_in_span(cx: &LateContext<'_>, module_body: Span) -> ModuleDocDisposition {
    let source_map = cx.tcx.sess.source_map();
    let Ok(snippet) = source_map.span_to_snippet(module_body) else {
        return ModuleDocDisposition::MissingDocs;
    };

    match classify_leading_content(SourceSnippet::from(snippet.as_str())) {
        LeadingContent::Doc => ModuleDocDisposition::HasLeadingDoc,
        LeadingContent::Missing => ModuleDocDisposition::MissingDocs,
        LeadingContent::Misordered { offset, len } => {
            ModuleDocDisposition::FirstInnerIsNotDoc(first_token_span(module_body, offset, len))
        }
    }
}

fn first_token_span(module_body: Span, offset: usize, len: usize) -> Span {
    let base = module_body.shrink_to_lo();
    let start = base.lo() + BytePos(offset as u32);
    let hi = start + BytePos(len.max(1) as u32);
    base.with_lo(start).with_hi(hi)
}

fn emit_diagnostic(cx: &LateContext<'_>, context: &ModuleDiagnosticContext, localizer: &Localizer) {
    let mut args: Arguments<'_> = Arguments::default();
    let module_name = ModuleName::from(context.ident.name.as_str());
    args.insert(Cow::Borrowed("module"), FluentValue::from(*module_name));

    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &args,
    };
    let messages = safe_resolve_message_set(localizer, resolution, noop_reporter, || {
        fallback_messages(module_name)
    });

    cx.span_lint(MODULE_MUST_HAVE_INNER_DOCS, context.primary_span, |lint| {
        lint.primary_message(messages.primary().to_string());
        lint.span_note(context.header_span, messages.note().to_string());
        lint.help(messages.help().to_string());
    });
}

type ModuleDocMessages = DiagnosticMessageSet;

fn fallback_messages(module: ModuleName<'_>) -> ModuleDocMessages {
    let primary = format!("Module {} must start with an inner doc comment.", *module);
    let note = String::from("The first item in the module is not a `//!` style comment.");
    let help = format!(
        "Explain the purpose of {} by adding an inner doc comment at the top.",
        *module
    );

    DiagnosticMessageSet::new(primary, note, help)
}

#[cfg(test)]
#[path = "tests/behaviour.rs"]
mod behaviour;

#[cfg(test)]
#[path = "tests/ui.rs"]
mod ui;

#[cfg(test)]
#[path = "tests/classifier.rs"]
mod classifier;

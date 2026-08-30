//! Require modules to begin with inner doc comments.
//!
//! `module_must_have_inner_docs` inspects every non-macro module and
//! verifies that the first inner attribute is a doc comment (`//!` or
//! `#![doc = "..."]`, including nested `cfg_attr` wrappers). Modules missing
//! such a comment, or placing other inner attributes before it, trigger a
//! diagnostic that nudges teams to document the module purpose at the top of
//! the file.
use std::borrow::Cow;

use log::debug;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext};
#[cfg(test)]
use rustc_span::DUMMY_SP;
use rustc_span::source_map::SourceMap;
use rustc_span::symbol::Ident;
use rustc_span::{BytePos, Span};
use whitaker::{SharedConfig, module_body_span, module_header_span};
use whitaker_common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    get_localizer_for_lint, noop_reporter, safe_resolve_message_set,
};

mod inner_attr;
mod parser;

/// Shared string newtype backing the parser's snippet aliases.
///
/// `newt_hype::base_newtype!` emits both `Copy` and an explicit `Clone` impl.
/// The impl is generated inside the external macro, so it has no source
/// location that could be changed to a derive; isolating the invocation keeps
/// the expectation scoped to exactly that generated impl.
mod str_wrapper {
    #![expect(
        clippy::expl_impl_clone_on_copy,
        reason = "newt_hype::base_newtype! emits an explicit Clone impl on a Copy type"
    )]

    use newt_hype::base_newtype;

    base_newtype!(StrWrapper);
}

pub use str_wrapper::StrWrapper;

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
    #[must_use]
    pub fn as_str(&self) -> &'a str {
        **self
    }
}

const LINT_NAME: &str = "module_must_have_inner_docs";
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);

/// Dylint lint declaration and registration glue.
///
/// `impl_late_lint!` expands to the Dylint ABI entry point and the
/// `impl_lint_pass!` accessor, neither of which has a source location that
/// could carry documentation. Isolating the invocation keeps the expectation
/// scoped to exactly those generated items.
mod declaration {
    #![expect(
        missing_docs,
        reason = "dylint_linting macro expansion emits items with no documentable source location"
    )]

    use super::ModuleMustHaveInnerDocs;

    dylint_linting::impl_late_lint! {
        /// Warns when a module body does not open with an inner doc comment.
        pub MODULE_MUST_HAVE_INNER_DOCS,
        Warn,
        "modules must begin with an inner doc comment",
        ModuleMustHaveInnerDocs::default()
    }
}

pub use declaration::MODULE_MUST_HAVE_INNER_DOCS;

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
        let hir::ItemKind::Mod(ident, module) = item.kind else {
            return;
        };

        if item.span.from_expansion() {
            debug!(
                target: LINT_NAME,
                "skipping module `{}` expanded from a macro", ident.name
            );
            return;
        }

        let module_body = module_body_span(cx, item, module);
        let source_map = cx.tcx.sess.source_map();
        let disposition = detect_module_docs_in_span(source_map, module_body);
        let Some(primary_span) = primary_span_for_disposition(disposition, module_body) else {
            return;
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
    /// The module source could not be inspected (e.g., macro-expanded or
    /// generated code with spans that cannot be resolved to source text).
    /// The lint should skip analysis rather than report a false positive.
    Unknown,
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
///
/// # Example
///
/// ```ignore
/// use crate::driver::{check_attribute_order, LeadingContent, ParseInput};
///
/// // An outer attribute before a module doc comment returns Misordered.
/// let input = ParseInput::from("#[cfg(test)]");
/// let result = check_attribute_order(input, 0);
/// assert_eq!(result, LeadingContent::Misordered { offset: 0, len: 12 });
/// ```
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

    // `split_inclusive` keeps each terminator attached, so accumulating the
    // yielded lengths reproduces the byte offset of every line start.
    for line_with_terminator in snippet.split_inclusive('\n') {
        let line = line_with_terminator
            .strip_suffix('\n')
            .unwrap_or(line_with_terminator);
        if check_line_for_inner_doc(snippet, line, line_start) {
            return true;
        }

        line_start = line_start.saturating_add(line_with_terminator.len());
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

    while let Some(local_idx) = line.get(search_start..).and_then(|tail| tail.find("#!")) {
        let absolute_idx = search_start + local_idx;
        let snippet_offset = line_start + absolute_idx;
        let Some(tail) = snippet.get(snippet_offset..) else {
            break;
        };
        if parser::is_doc_comment(ParseInput::from(tail)) {
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

fn detect_module_docs_in_span(source_map: &SourceMap, module_body: Span) -> ModuleDocDisposition {
    let Ok(snippet) = source_map.span_to_snippet(module_body) else {
        return ModuleDocDisposition::Unknown;
    };

    match classify_leading_content(SourceSnippet::from(snippet.as_str())) {
        LeadingContent::Doc => ModuleDocDisposition::HasLeadingDoc,
        LeadingContent::Missing => ModuleDocDisposition::MissingDocs,
        LeadingContent::Misordered { offset, len } => {
            ModuleDocDisposition::FirstInnerIsNotDoc(first_token_span(module_body, offset, len))
        }
    }
}

/// Maps a module doc disposition to the primary diagnostic span.
///
/// Returns `None` for `HasLeadingDoc` and `Unknown`,
/// `Some(module_body.shrink_to_lo())` for `MissingDocs`, and `Some(span)` for
/// `FirstInnerIsNotDoc(span)`.
fn primary_span_for_disposition(
    disposition: ModuleDocDisposition,
    module_body: Span,
) -> Option<Span> {
    match disposition {
        ModuleDocDisposition::HasLeadingDoc | ModuleDocDisposition::Unknown => None,
        ModuleDocDisposition::MissingDocs => Some(module_body.shrink_to_lo()),
        ModuleDocDisposition::FirstInnerIsNotDoc(span) => Some(span),
    }
}

fn first_token_span(module_body: Span, offset: usize, len: usize) -> Span {
    let base = module_body.shrink_to_lo();
    // Source files never exceed `u32::MAX` bytes, so a failed conversion means
    // the caller supplied a nonsensical offset; fall back to the module start.
    let (Ok(byte_offset), Ok(byte_len)) = (u32::try_from(offset), u32::try_from(len.max(1))) else {
        return base;
    };
    let start = base.lo() + BytePos(byte_offset);
    let hi = start + BytePos(byte_len);
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

    cx.emit_span_lint(
        MODULE_MUST_HAVE_INNER_DOCS,
        context.primary_span,
        rustc_lint::errors::DiagDecorator(|lint| {
            lint.primary_message(messages.primary().to_owned());
            lint.span_note(context.header_span, messages.note().to_owned());
            lint.help(messages.help().to_owned());
        }),
    );
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
#[path = "../tests/behaviour.rs"]
mod behaviour;

#[cfg(test)]
#[path = "../tests/ui.rs"]
mod ui;

#[cfg(test)]
#[path = "../tests/classifier.rs"]
mod classifier;

#[cfg(test)]
#[path = "../tests/span_to_snippet.rs"]
mod span_to_snippet;

//! Require modules to begin with inner doc comments.
//!
//! `module_must_have_inner_docs` inspects every non-macro module and
//! verifies that the first inner attribute is a doc comment (`//!` or
//! `#![doc = "..."]`). Modules missing such a comment, or placing other inner
//! attributes before it, trigger a diagnostic that nudges teams to document the
//! module purpose at the top of the file.
use std::borrow::Cow;

use common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    get_localizer_for_lint, safe_resolve_message_set,
};
use log::debug;
use newt_hype::{base_newtype, newtype};
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext};
#[cfg(test)]
use rustc_span::DUMMY_SP;
use rustc_span::symbol::Ident;
use rustc_span::{BytePos, Span};
use whitaker::{SharedConfig, module_body_span, module_header_span};

base_newtype! {
    #[derive(Clone, Copy, Debug)]
    pub StrWrapper<'a>: &'a str;
}

newtype!(SourceSnippet, StrWrapper<'a>: &'a str);
newtype!(AttributeBody, StrWrapper<'a>: &'a str);
newtype!(ParseInput, StrWrapper<'a>: &'a str);
newtype!(MetaList, StrWrapper<'a>: &'a str);
newtype!(ModuleName, StrWrapper<'a>: &'a str);

impl<'a> ParseInput<'a> {
    pub fn as_str(&self) -> &'a str {
        **self
    }
}

impl<'a> ParseInput<'a> {
    fn as_str(self) -> &'a str {
        self.0
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
    let (offset, rest) = skip_leading_whitespace(ParseInput::from(snippet.as_ref()));
    if rest.is_empty() {
        return LeadingContent::Missing;
    }
    if is_doc_comment(rest) {
        return LeadingContent::Doc;
    }
    check_attribute_order(rest, offset)
}

fn skip_leading_whitespace<'a>(snippet: ParseInput<'a>) -> (usize, ParseInput<'a>) {
    let snippet_str = snippet.as_str();
    let bytes = snippet_str.as_bytes();
    let mut offset = 0;
    while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
        offset += 1;
    }
    (offset, ParseInput::from(&snippet_str[offset..]))
}

fn is_doc_comment(rest: ParseInput<'_>) -> bool {
    if rest.starts_with("//!") {
        return true;
    }
    if let Some(after_bang) = rest.strip_prefix("#!") {
        let (_, tail) = skip_leading_whitespace(ParseInput::from(after_bang));
        if let Some(body) = tail.strip_prefix('[') {
            let attr_end = body.find(']').unwrap_or(body.len());
            let attr_body = AttributeBody::from(&body[..attr_end]);
            return is_doc_attr(attr_body);
        }
    }
    false
}

fn is_doc_attr(attr_body: AttributeBody<'_>) -> bool {
    let Some((ident, tail)) = take_ident(ParseInput::from(attr_body.as_ref())) else {
        return false;
    };

    if ident.eq_ignore_ascii_case("doc") {
        return true;
    }

    if ident.eq_ignore_ascii_case("cfg_attr") {
        return cfg_attr_has_doc(tail);
    }

    false
}

fn take_ident<'a>(input: ParseInput<'a>) -> Option<(ParseInput<'a>, ParseInput<'a>)> {
    let (_, trimmed) = skip_leading_whitespace(input);
    let trimmed_str = trimmed.as_str();
    let mut iter = trimmed_str.char_indices();
    let (start, ch) = iter.next()?;
    if !is_ident_start(ch) {
        return None;
    }

    let mut end = start + ch.len_utf8();
    for (idx, ch) in iter {
        if is_ident_continue(ch) {
            end = idx + ch.len_utf8();
        } else {
            break;
        }
    }

    let ident = ParseInput::from(&trimmed_str[..end]);
    Some((ident, ParseInput::from(&trimmed_str[end..])))
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn cfg_attr_has_doc(rest: ParseInput<'_>) -> bool {
    let (_, trimmed) = skip_leading_whitespace(rest);
    let Some(content) = trimmed.strip_prefix('(') else {
        return false;
    };

    let mut depth: usize = 1;
    let mut first_comma = None;
    let mut closing_paren = None;

    for (idx, ch) in content.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    closing_paren = Some(idx);
                    break;
                }
            }
            ',' if depth == 1 && first_comma.is_none() => first_comma = Some(idx),
            _ => {}
        }
    }

    let Some(attr_section_start) = first_comma else {
        return false;
    };
    let Some(close_idx) = closing_paren else {
        return false;
    };

    let args = &content[..close_idx];
    let attr_section = &args[attr_section_start + 1..];
    has_doc_in_meta_list(MetaList::from(attr_section))
}

fn has_doc_in_meta_list(list: MetaList<'_>) -> bool {
    let list_str = list.as_ref();
    let mut depth: usize = 0;
    let mut start = 0;

    for (idx, ch) in list_str.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if segment_is_doc(&list_str[start..idx]) {
                    return true;
                }
                start = idx + 1;
            }
            _ => {}
        }
    }

    segment_is_doc(&list_str[start..])
}

fn segment_is_doc(segment: &str) -> bool {
    let Some((ident, _)) = take_ident(ParseInput::from(segment)) else {
        return false;
    };

    ident.eq_ignore_ascii_case("doc")
}

fn check_attribute_order(rest: ParseInput<'_>, offset: usize) -> LeadingContent {
    if rest.starts_with("#[") {
        return LeadingContent::Missing;
    }
    if rest.starts_with('#') {
        let len = rest.find(['\n', '\r']).unwrap_or(rest.len());
        return LeadingContent::Misordered { offset, len };
    }
    LeadingContent::Missing
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
    args.insert(
        Cow::Borrowed("module"),
        FluentValue::from(module_name.as_ref()),
    );

    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &args,
    };
    let messages = safe_resolve_message_set(
        localizer,
        resolution,
        |message| {
            cx.tcx
                .sess
                .dcx()
                .span_delayed_bug(context.primary_span, message);
        },
        || fallback_messages(module_name),
    );

    cx.span_lint(MODULE_MUST_HAVE_INNER_DOCS, context.primary_span, |lint| {
        lint.primary_message(messages.primary().to_string());
        lint.span_note(context.header_span, messages.note().to_string());
        lint.help(messages.help().to_string());
    });
}

type ModuleDocMessages = DiagnosticMessageSet;

fn fallback_messages(module: ModuleName<'_>) -> ModuleDocMessages {
    let primary = format!(
        "Module {} must start with an inner doc comment.",
        module.as_ref()
    );
    let note = String::from("The first item in the module is not a `//!` style comment.");
    let help = format!(
        "Explain the purpose of {} by adding an inner doc comment at the top.",
        module.as_ref()
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

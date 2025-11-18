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
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext};
#[cfg(test)]
use rustc_span::DUMMY_SP;
use rustc_span::symbol::Ident;
use rustc_span::{BytePos, Span};
use whitaker::{SharedConfig, module_body_span, module_header_span};

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

fn contains_doc_token(attr_body: &str) -> bool {
    attr_body.match_indices("doc").any(|(index, _)| {
        let before = attr_body[..index]
            .chars()
            .rev()
            .find(|ch| !ch.is_whitespace());
        let after = attr_body[index + 3..]
            .chars()
            .find(|ch| !ch.is_whitespace());
        let before_ok = before.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_');
        let after_ok = after.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_');
        before_ok && after_ok
    })
}

fn classify_leading_content(snippet: &str) -> LeadingContent {
    let bytes = snippet.as_bytes();
    let len = bytes.len();
    let mut offset = 0;

    while offset < len && bytes[offset].is_ascii_whitespace() {
        offset += 1;
    }

    if offset >= len {
        return LeadingContent::Missing;
    }

    let rest = &snippet[offset..];
    if rest.starts_with("//!") {
        return LeadingContent::Doc;
    }
    if rest.starts_with("#![") {
        let attr_end = rest.find(']').unwrap_or(rest.len());
        let attr_body = rest[3..attr_end].to_ascii_lowercase();
        if contains_doc_token(&attr_body) {
            return LeadingContent::Doc;
        }
    }

    if rest.starts_with("#[") {
        return LeadingContent::Missing;
    }

    if rest.starts_with('#') {
        let line_len = rest.find(['\n', '\r']).unwrap_or(rest.len());
        return LeadingContent::Misordered {
            offset,
            len: line_len,
        };
    }

    LeadingContent::Missing
}

#[cfg(test)]
fn detect_module_docs_from_snippet(snippet: &str) -> ModuleDocDisposition {
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

    match classify_leading_content(&snippet) {
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
    let module_name = context.ident.name.as_str();
    args.insert(Cow::Borrowed("module"), FluentValue::from(module_name));

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

fn fallback_messages(module: &str) -> ModuleDocMessages {
    let primary = format!("Module {module} must start with an inner doc comment.");
    let note = String::from("The first item in the module is not a `//!` style comment.");
    let help =
        format!("Explain the purpose of {module} by adding an inner doc comment at the top.");

    DiagnosticMessageSet::new(primary, note, help)
}

#[cfg(test)]
#[path = "tests/behaviour.rs"]
mod behaviour;

#[cfg(test)]
#[path = "tests/ui.rs"]
mod ui;

#[cfg(test)]
mod tests {
    use super::{ModuleDocDisposition, detect_module_docs_from_snippet};
    use rstest::rstest;

    #[rstest]
    fn detects_missing_docs_when_no_content() {
        assert_eq!(
            detect_module_docs_from_snippet("\n  \n"),
            ModuleDocDisposition::MissingDocs
        );
    }

    #[rstest]
    fn accepts_leading_inner_doc() {
        assert_eq!(
            detect_module_docs_from_snippet("//! module docs"),
            ModuleDocDisposition::HasLeadingDoc
        );
    }

    #[rstest]
    fn accepts_inner_doc_attribute() {
        assert_eq!(
            detect_module_docs_from_snippet("#![doc = \"text\"]"),
            ModuleDocDisposition::HasLeadingDoc
        );
    }

    #[rstest]
    fn rejects_doc_after_inner_attribute() {
        assert!(matches!(
            detect_module_docs_from_snippet("#![allow(dead_code)]\n//! doc"),
            ModuleDocDisposition::FirstInnerIsNotDoc(_)
        ));
    }

    #[rstest]
    fn outer_docs_do_not_satisfy_requirement() {
        assert_eq!(
            detect_module_docs_from_snippet("/// doc"),
            ModuleDocDisposition::MissingDocs
        );
    }

    #[rstest]
    fn outer_doc_attribute_does_not_satisfy_requirement() {
        assert_eq!(
            detect_module_docs_from_snippet("#[doc = \"module docs\"]\npub fn demo() {}"),
            ModuleDocDisposition::MissingDocs
        );
    }
}

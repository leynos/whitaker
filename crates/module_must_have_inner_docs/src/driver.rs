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
use rustc_ast::AttrStyle;
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::symbol::Ident;
use whitaker::SharedConfig;

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

        let attrs = cx.tcx.hir_attrs(item.hir_id());
        let disposition = detect_module_docs(attrs);
        if disposition == ModuleDocDisposition::HasLeadingDoc {
            return;
        }

        let primary_span = match disposition {
            ModuleDocDisposition::HasLeadingDoc => return,
            ModuleDocDisposition::MissingDocs => module_body_start_span(cx, item, module),
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

/// Simplified attribute interface used by the detector and its tests.
pub(crate) trait ModuleAttribute {
    /// Returns `true` when the attribute is written with `#![...]` syntax.
    fn is_inner(&self) -> bool;
    /// Returns `true` when the attribute is a documentation comment.
    fn is_doc(&self) -> bool;
    /// Provides the attribute span for diagnostics.
    fn span(&self) -> Span;
}

impl ModuleAttribute for hir::Attribute {
    fn is_inner(&self) -> bool {
        matches!(attribute_style(self), AttrStyle::Inner)
    }

    fn is_doc(&self) -> bool {
        self.doc_str().is_some()
    }

    fn span(&self) -> Span {
        self.span()
    }
}

fn attribute_style(attr: &hir::Attribute) -> AttrStyle {
    match attr {
        hir::Attribute::Unparsed(item) => item.style,
        hir::Attribute::Parsed(HirAttributeKind::DocComment { style, .. }) => *style,
        _ => AttrStyle::Outer,
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

/// Determine whether the module begins with an inner doc comment.
#[must_use]
pub(crate) fn detect_module_docs<A: ModuleAttribute>(attrs: &[A]) -> ModuleDocDisposition {
    let mut inner_attributes = attrs.iter().filter(|attr| attr.is_inner());

    match inner_attributes.next() {
        Some(attr) if attr.is_doc() => ModuleDocDisposition::HasLeadingDoc,
        Some(attr) => ModuleDocDisposition::FirstInnerIsNotDoc(attr.span()),
        None => ModuleDocDisposition::MissingDocs,
    }
}

struct ModuleDiagnosticContext {
    ident: Ident,
    primary_span: Span,
    header_span: Span,
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

fn module_body_start_span<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx hir::Item<'tcx>,
    module: &hir::Mod<'tcx>,
) -> Span {
    let inner_span = module.spans.inner_span;
    if !inner_span.is_dummy() {
        return inner_span.shrink_to_lo();
    }

    let def_span = cx.tcx.def_span(item.owner_id.to_def_id());
    if !def_span.is_dummy() {
        return def_span.shrink_to_lo();
    }

    item.span.shrink_to_lo()
}

fn module_header_span(item_span: Span, ident_span: Span) -> Span {
    item_span.with_hi(ident_span.hi())
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::ModuleAttribute;
    use rustc_span::{DUMMY_SP, Span};

    /// Lightweight attribute stub for exercising the detector.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct StubAttribute {
        inner: bool,
        doc: bool,
        span: Span,
    }

    impl StubAttribute {
        /// Construct an inner doc attribute.
        pub fn inner_doc() -> Self {
            Self {
                inner: true,
                doc: true,
                span: DUMMY_SP,
            }
        }

        /// Construct a non-doc inner attribute (for example, `#![allow(..)]`).
        pub fn inner_allow() -> Self {
            Self {
                inner: true,
                doc: false,
                span: DUMMY_SP,
            }
        }

        /// Construct an outer doc attribute.
        pub fn outer_doc() -> Self {
            Self {
                inner: false,
                doc: true,
                span: DUMMY_SP,
            }
        }
    }

    impl ModuleAttribute for StubAttribute {
        fn is_inner(&self) -> bool {
            self.inner
        }

        fn is_doc(&self) -> bool {
            self.doc
        }

        fn span(&self) -> Span {
            self.span
        }
    }
}

#[cfg(test)]
#[path = "tests/behaviour.rs"]
mod behaviour;

#[cfg(test)]
#[path = "tests/ui.rs"]
mod ui;

#[cfg(test)]
mod tests {
    use super::{ModuleDocDisposition, test_support::StubAttribute};
    use rstest::rstest;

    #[rstest]
    fn detects_missing_docs_when_no_inner_attributes() {
        assert_eq!(
            super::detect_module_docs::<StubAttribute>(&[]),
            ModuleDocDisposition::MissingDocs
        );
    }

    #[rstest]
    fn accepts_leading_inner_doc() {
        let attrs = [StubAttribute::inner_doc()];
        assert_eq!(
            super::detect_module_docs(&attrs),
            ModuleDocDisposition::HasLeadingDoc
        );
    }

    #[rstest]
    fn rejects_doc_after_inner_attribute() {
        let attrs = [StubAttribute::inner_allow(), StubAttribute::inner_doc()];
        assert!(matches!(
            super::detect_module_docs(&attrs),
            ModuleDocDisposition::FirstInnerIsNotDoc(_)
        ));
    }

    #[rstest]
    fn outer_docs_do_not_satisfy_requirement() {
        let attrs = [StubAttribute::outer_doc()];
        assert_eq!(
            super::detect_module_docs(&attrs),
            ModuleDocDisposition::MissingDocs
        );
    }
}

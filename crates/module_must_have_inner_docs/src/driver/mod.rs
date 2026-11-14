//! Require modules to begin with inner doc comments.
//!
//! `module_must_have_inner_docs` inspects every non-macro module and verifies
//! that the first inner attribute is documentation (`//!`, `/*! ... */`, or
//! `#![doc = "..."]`, including documentation produced by an active
//! `cfg_attr`). Modules missing such documentation, or placing other inner
//! attributes before it, trigger a diagnostic that nudges teams to document
//! the module purpose at the top of the file.
use std::borrow::Cow;

use log::debug;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Span, symbol::Ident};
use whitaker_common::i18n::{
    Arguments,
    DiagnosticMessageSet,
    FluentValue,
    Localizer,
    MessageKey,
    MessageResolution,
    get_localizer_for_lint,
    noop_reporter,
    safe_resolve_message_set,
};
use whitaker_lint_core::{SharedConfig, module_header_span};

mod hir_attrs;

use hir_attrs::{classify, module_attrs};

/// Shared string newtype backing the localized message arguments.
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

pub type ModuleName<'a> = StrWrapper<&'a str>;

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

        // The attribute list of a real module always has a body span. A dummy
        // or expansion-derived span means the body cannot be inspected, so the
        // lint stays silent rather than guessing at its contents.
        let inner_span = module.spans.inner_span;
        let has_body = !inner_span.is_dummy() && !inner_span.from_expansion();
        let disposition = classify(&module_attrs(cx, item, inner_span), has_body);
        let Some(primary_span) = primary_span_for_disposition(disposition, inner_span) else {
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
    /// No inner doc record is present, so documentation is missing entirely.
    MissingDocs,
    /// The first inner attribute is not a doc comment.
    FirstInnerIsNotDoc(Span),
    /// The module body span is dummy or expansion-derived, so its attributes
    /// cannot be inspected. The lint skips analysis rather than reporting a
    /// false positive.
    Unknown,
}

struct ModuleDiagnosticContext {
    ident: Ident,
    primary_span: Span,
    header_span: Span,
}

/// Maps a module doc disposition to the primary diagnostic span.
///
/// Returns `None` for `HasLeadingDoc` and `Unknown`,
/// `Some(inner_span.shrink_to_lo())` for `MissingDocs`, and `Some(span)` for
/// `FirstInnerIsNotDoc(span)`.
fn primary_span_for_disposition(
    disposition: ModuleDocDisposition,
    inner_span: Span,
) -> Option<Span> {
    match disposition {
        ModuleDocDisposition::HasLeadingDoc | ModuleDocDisposition::Unknown => None,
        ModuleDocDisposition::MissingDocs => Some(inner_span.shrink_to_lo()),
        ModuleDocDisposition::FirstInnerIsNotDoc(span) => Some(span),
    }
}

/// Emit a localized diagnostic for a module missing inner documentation.
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
#[path = "../tests/support.rs"]
mod test_support;

#[cfg(test)]
#[path = "../tests/behaviour.rs"]
mod behaviour;

#[cfg(test)]
#[path = "../tests/ui.rs"]
mod ui;

#[cfg(test)]
#[path = "../tests/classifier.rs"]
mod classifier;

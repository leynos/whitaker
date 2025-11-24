//! Lint crate enforcing configurable module length limits.
//!
//! `module_max_lines` measures the number of source lines occupied by a module
//! and warns when the count exceeds the configurable `max_lines` threshold.
//! The lint uses localisation data sourced from the shared Whitaker
//! infrastructure so diagnostics match the suite's tone across locales.
use common::i18n::{
    Arguments, DiagnosticMessageSet, Localizer, MessageKey, MessageResolution,
    get_localizer_for_lint, safe_resolve_message_set,
};
use log::debug;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::source_map::SourceMap;
use rustc_span::symbol::Ident;
use whitaker::{ModuleMaxLinesConfig, SharedConfig, module_body_span, module_header_span};

const LINT_NAME: &str = "module_max_lines";
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new("module_max_lines");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModuleDisposition {
    Ignore,
    WithinLimit,
    ExceedsLimit,
}

dylint_linting::impl_late_lint! {
    pub MODULE_MAX_LINES,
    Warn,
    "modules should stay within the configured maximum line count",
    ModuleMaxLines::default()
}

/// Lint pass that tracks configuration and localisation state while checking modules.
pub struct ModuleMaxLines {
    max_lines: usize,
    localizer: Localizer,
}

impl Default for ModuleMaxLines {
    fn default() -> Self {
        Self {
            max_lines: ModuleMaxLinesConfig::default().max_lines,
            localizer: Localizer::new(None),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for ModuleMaxLines {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        self.max_lines = load_configuration();
        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        let (ident, module) = match item.kind {
            hir::ItemKind::Mod(ident, module) => (ident, module),
            _ => return,
        };

        let span = module_body_span(cx, item, module);
        let Some(lines) = count_lines(cx.sess().source_map(), span) else {
            debug!(
                target: LINT_NAME,
                "unable to determine line span for module `{}`; skipping",
                ident.name
            );
            return;
        };
        debug!(
            target: LINT_NAME,
            "module `{}` spans {lines} lines (limit {limit}, from_macro: {from_macro})",
            ident.name,
            limit = self.max_lines,
            from_macro = item.span.from_expansion(),
        );

        let disposition = evaluate_module(lines, self.max_lines, item.span.from_expansion());
        if disposition != ModuleDisposition::ExceedsLimit {
            return;
        }

        let info = ModuleDiagnosticInfo {
            ident,
            item_span: item.span,
            lines,
            limit: self.max_lines,
        };
        emit_diagnostic(cx, &info, &self.localizer);
    }
}

fn evaluate_module(lines: usize, limit: usize, from_macro: bool) -> ModuleDisposition {
    if from_macro {
        ModuleDisposition::Ignore
    } else if lines > limit {
        ModuleDisposition::ExceedsLimit
    } else {
        ModuleDisposition::WithinLimit
    }
}

fn load_configuration() -> usize {
    match dylint_linting::config::<ModuleMaxLinesConfig>(LINT_NAME) {
        Ok(Some(config)) => config.max_lines,
        Ok(None) => ModuleMaxLinesConfig::default().max_lines,
        Err(error) => {
            debug!(
                target: LINT_NAME,
                "failed to parse `{}` configuration: {error}; using defaults",
                LINT_NAME
            );
            ModuleMaxLinesConfig::default().max_lines
        }
    }
}

fn count_lines(source_map: &SourceMap, span: Span) -> Option<usize> {
    let info = source_map.span_to_lines(span).ok()?;
    let first = info.lines.first()?;
    let last = info.lines.last()?;

    let contiguous = info
        .lines
        .windows(2)
        .all(|pair| pair[1].line_index == pair[0].line_index + 1);
    if !contiguous {
        debug!(
            target: LINT_NAME,
            "span lines are not contiguous; skipping module length measurement"
        );
        return None;
    }

    Some(last.line_index.saturating_sub(first.line_index) + 1)
}

/// Diagnostic information for a module that exceeds line limits.
struct ModuleDiagnosticInfo {
    ident: Ident,
    item_span: Span,
    lines: usize,
    limit: usize,
}

fn emit_diagnostic(cx: &LateContext<'_>, info: &ModuleDiagnosticInfo, localizer: &Localizer) {
    use fluent_templates::fluent_bundle::FluentValue;
    use std::borrow::Cow;

    let mut args: Arguments<'_> = Arguments::default();
    let module_name = info.ident.name.as_str();
    args.insert(Cow::Borrowed("module"), FluentValue::from(module_name));
    args.insert(Cow::Borrowed("lines"), FluentValue::from(info.lines as i64));
    args.insert(Cow::Borrowed("limit"), FluentValue::from(info.limit as i64));

    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &args,
    };
    let messages = safe_resolve_message_set(
        localizer,
        resolution,
        |message| {
            debug!(
                target: LINT_NAME,
                "missing localisation for `{}`: {message}; using fallback strings",
                LINT_NAME
            );
            cx.tcx.sess.dcx().span_delayed_bug(info.item_span, message);
        },
        || fallback_messages(module_name, info.lines, info.limit),
    );

    cx.span_lint(MODULE_MAX_LINES, info.ident.span, |lint| {
        lint.primary_message(messages.primary().to_string());
        lint.span_note(
            module_header_span(info.item_span, info.ident.span),
            messages.note().to_string(),
        );
        lint.help(messages.help().to_string());
    });
}

fn fallback_messages(module: &str, lines: usize, limit: usize) -> DiagnosticMessageSet {
    DiagnosticMessageSet::new(
        format!("Module {module} spans {lines} lines, exceeding the allowed {limit}."),
        String::from("Large modules are harder to navigate and review."),
        format!("Split {module} into smaller modules or reduce its responsibilities."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(4, 5, false, ModuleDisposition::WithinLimit)]
    #[case(6, 5, false, ModuleDisposition::ExceedsLimit)]
    #[case(5, 5, false, ModuleDisposition::WithinLimit)]
    #[case(10, 1, true, ModuleDisposition::Ignore)]
    fn evaluate_module_behaviour(
        #[case] lines: usize,
        #[case] limit: usize,
        #[case] from_macro: bool,
        #[case] expected: ModuleDisposition,
    ) {
        assert_eq!(evaluate_module(lines, limit, from_macro), expected);
    }
}

#[cfg(test)]
mod behaviour {
    use super::{ModuleDisposition, evaluate_module};
    use rstest::fixture;
    use rstest_bdd_macros::{given, scenario, then, when};
    use std::cell::RefCell;

    #[derive(Default)]
    struct ModuleWorld {
        lines: RefCell<usize>,
        limit: RefCell<usize>,
        from_macro: RefCell<bool>,
        disposition: RefCell<Option<ModuleDisposition>>,
    }

    impl ModuleWorld {
        fn set_lines(&self, value: usize) {
            *self.lines.borrow_mut() = value;
        }

        fn set_limit(&self, value: usize) {
            *self.limit.borrow_mut() = value;
        }

        fn mark_macro(&self) {
            *self.from_macro.borrow_mut() = true;
        }

        fn evaluate(&self) {
            let lines = *self.lines.borrow();
            let limit = *self.limit.borrow();
            let from_macro = *self.from_macro.borrow();
            let result = evaluate_module(lines, limit, from_macro);
            self.disposition.borrow_mut().replace(result);
        }

        fn disposition(&self) -> ModuleDisposition {
            self.disposition
                .borrow()
                .expect("module disposition should be recorded")
        }
    }

    #[fixture]
    fn world() -> ModuleWorld {
        ModuleWorld::default()
    }

    #[given("the maximum module length is {limit}")]
    fn given_limit(world: &ModuleWorld, limit: usize) {
        world.set_limit(limit);
    }

    #[given("a module spans {lines} lines")]
    fn given_lines(world: &ModuleWorld, lines: usize) {
        world.set_lines(lines);
    }

    #[given("the module originates from a macro expansion")]
    fn given_macro(world: &ModuleWorld) {
        world.mark_macro();
    }

    #[when("I evaluate the module length")]
    fn when_evaluate(world: &ModuleWorld) {
        world.evaluate();
    }

    #[then("the module is accepted")]
    fn then_accepted(world: &ModuleWorld) {
        assert_eq!(world.disposition(), ModuleDisposition::WithinLimit);
    }

    #[then("the module is rejected")]
    fn then_rejected(world: &ModuleWorld) {
        assert_eq!(world.disposition(), ModuleDisposition::ExceedsLimit);
    }

    #[then("the module evaluation is ignored")]
    fn then_ignored(world: &ModuleWorld) {
        assert_eq!(world.disposition(), ModuleDisposition::Ignore);
    }

    #[scenario(path = "tests/features/module_length.feature", index = 0)]
    fn scenario_within_limit(world: ModuleWorld) {
        let _ = world;
    }

    #[scenario(path = "tests/features/module_length.feature", index = 1)]
    fn scenario_exceeds_limit(world: ModuleWorld) {
        let _ = world;
    }

    #[scenario(path = "tests/features/module_length.feature", index = 2)]
    fn scenario_exact_limit(world: ModuleWorld) {
        let _ = world;
    }

    #[scenario(path = "tests/features/module_length.feature", index = 3)]
    fn scenario_macro(world: ModuleWorld) {
        let _ = world;
    }
}

#[cfg(test)]
#[path = "lib_ui_tests.rs"]
mod ui;

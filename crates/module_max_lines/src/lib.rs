#![feature(rustc_private)]

//! Lint crate enforcing configurable module length limits.
//!
//! `module_max_lines` measures the number of source lines occupied by a module
//! and warns when the count exceeds the configurable `max_lines` threshold.
//! The lint uses localisation data sourced from the shared Whitaker
//! infrastructure so diagnostics match the suite's tone across locales.

use common::i18n::{Arguments, I18nError, Localiser};
use log::debug;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::source_map::SourceMap;
use rustc_span::symbol::Ident;
use whitaker::ModuleMaxLinesConfig;

const LINT_NAME: &str = "module_max_lines";
const MESSAGE_KEY: &str = "module_max_lines";

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
    localiser: Localiser,
}

impl Default for ModuleMaxLines {
    fn default() -> Self {
        Self {
            max_lines: ModuleMaxLinesConfig::default().max_lines,
            localiser: Localiser::new(None),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for ModuleMaxLines {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        self.max_lines = load_configuration();
        self.localiser = resolve_localiser();
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        let (ident, module) = match item.kind {
            hir::ItemKind::Mod(ident, module) => (ident, module),
            _ => return,
        };

        let span = module_span(module, item.span);
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

        emit_diagnostic(cx, ident, item.span, lines, self.max_lines, &self.localiser);
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

fn resolve_localiser() -> Localiser {
    std::env::var_os("DYLINT_LOCALE")
        .and_then(|value| value.into_string().ok())
        .map(|tag| {
            if common::i18n::supports_locale(&tag) {
                Localiser::new(Some(&tag))
            } else {
                debug!(
                    target: LINT_NAME,
                    "unsupported DYLINT_LOCALE `{tag}`; falling back to en-GB"
                );
                Localiser::new(None)
            }
        })
        .unwrap_or_else(|| Localiser::new(None))
}

fn module_span(module: &hir::Mod<'_>, fallback: Span) -> Span {
    let span = module.spans.inner_span;

    if span.is_dummy() { fallback } else { span }
}

fn count_lines(source_map: &SourceMap, span: Span) -> Option<usize> {
    let Ok(info) = source_map.span_to_lines(span) else {
        return None;
    };

    Some(info.lines.len())
}

fn emit_diagnostic(
    cx: &LateContext<'_>,
    ident: Ident,
    item_span: Span,
    lines: usize,
    limit: usize,
    localiser: &Localiser,
) {
    use fluent_templates::fluent_bundle::FluentValue;
    use std::borrow::Cow;

    let mut args: Arguments<'_> = Arguments::default();
    let module_name = ident.name.as_str();
    args.insert(Cow::Borrowed("module"), FluentValue::from(module_name));
    args.insert(Cow::Borrowed("lines"), FluentValue::from(lines as i64));
    args.insert(Cow::Borrowed("limit"), FluentValue::from(limit as i64));

    let (primary, note, help) = localised_messages(localiser, &args).unwrap_or_else(|error| {
        debug!(
            target: LINT_NAME,
            "missing localisation for `{}`: {error}; using fallback strings",
            LINT_NAME
        );
        fallback_messages(module_name, lines, limit)
    });

    cx.span_lint(MODULE_MAX_LINES, ident.span, |lint| {
        lint.primary_message(primary);
        lint.span_note(module_header_span(item_span, ident.span), note);
        lint.help(help);
    });
}

fn module_header_span(item_span: Span, ident_span: Span) -> Span {
    item_span.with_hi(ident_span.hi())
}

fn localised_messages(
    localiser: &Localiser,
    args: &Arguments<'_>,
) -> Result<(String, String, String), I18nError> {
    let primary = localiser.message_with_args(MESSAGE_KEY, args)?;
    let note = localiser.attribute_with_args(MESSAGE_KEY, "note", args)?;
    let help = localiser.attribute_with_args(MESSAGE_KEY, "help", args)?;

    Ok((primary, note, help))
}

fn fallback_messages(module: &str, lines: usize, limit: usize) -> (String, String, String) {
    let primary =
        format!("Module {module} spans {lines} lines which exceeds the {limit} line limit.");
    let note = String::from("Large modules are harder to navigate and review.");
    let help = format!("Split {module} into smaller modules or reduce its responsibilities.");

    (primary, note, help)
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
mod ui {
    use dylint_testing::ui::Test;

    #[test]
    fn ui() {
        let crate_name = env!("CARGO_PKG_NAME");
        let directory = "ui";
        whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, dir| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut test = Test::src_base(crate_name, dir);
                if let Ok(contents) = std::fs::read_to_string(dir.join("dylint.toml")) {
                    test.dylint_toml(&contents);
                }
                test.run();
            }))
            .map_err(|payload| match payload.downcast::<String>() {
                Ok(message) => *message,
                Err(payload) => match payload.downcast::<&'static str>() {
                    Ok(message) => (*message).to_owned(),
                    Err(_) => String::from("dylint UI tests panicked without a message"),
                },
            })
        })
        .unwrap_or_else(|error| {
            panic!(
                "UI tests should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error} }}"
            )
        });
    }
}

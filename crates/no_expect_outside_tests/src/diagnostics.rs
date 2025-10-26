use crate::NO_EXPECT_OUTSIDE_TESTS;
use crate::context::ContextSummary;
use common::i18n::{Arguments, FluentValue, I18nError, Localiser};
use rustc_hir as hir;
use rustc_lint::{LateContext, LintContext};
use std::borrow::Cow;

pub(crate) fn emit_diagnostic(
    cx: &LateContext<'_>,
    expr: &hir::Expr<'_>,
    receiver: &hir::Expr<'_>,
    summary: &ContextSummary,
    localiser: &Localiser,
) {
    let receiver_ty = cx.typeck_results().expr_ty(receiver).peel_refs();
    let receiver_label = format!("`{}`", receiver_ty);
    let context = context_label(summary);

    let messages = localised_messages(localiser, receiver_label.as_str(), context.as_str())
        .unwrap_or_else(|error| {
            cx.sess().delay_span_bug(
                expr.span,
                format!("missing localisation for `no_expect_outside_tests`: {error}"),
            );
            fallback_messages(receiver_label.as_str(), context.as_str())
        });

    cx.span_lint(NO_EXPECT_OUTSIDE_TESTS, expr.span, |lint| {
        let NoExpectMessages {
            primary,
            note,
            help,
        } = messages;

        lint.primary_message(primary);
        lint.note(note);
        lint.help(help);
    });
}

const MESSAGE_KEY: &str = "no_expect_outside_tests";

fn localised_messages(
    lookup: &impl BundleLookup,
    receiver: &str,
    context: &str,
) -> Result<NoExpectMessages, I18nError> {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("receiver"),
        FluentValue::from(receiver.to_string()),
    );
    args.insert(
        Cow::Borrowed("context"),
        FluentValue::from(context.to_string()),
    );

    let primary = lookup.message(MESSAGE_KEY, &args)?;
    let note = lookup.attribute(MESSAGE_KEY, "note", &args)?;
    let help = lookup.attribute(MESSAGE_KEY, "help", &args)?;

    Ok(NoExpectMessages::new(primary, note, help))
}

fn fallback_messages(receiver: &str, context: &str) -> NoExpectMessages {
    let primary = format!("Avoid calling expect on {receiver} outside test-only code.");
    let note = format!("The call originates within {context} which is not recognised as a test.",);
    let help = format!("Handle the error returned by {receiver} or move the code into a test.",);

    NoExpectMessages::new(primary, note, help)
}

fn context_label(summary: &ContextSummary) -> String {
    summary
        .function_name
        .as_ref()
        .map(|name| format!("function `{name}`"))
        .unwrap_or_else(|| "the surrounding scope".to_string())
}

struct NoExpectMessages {
    primary: String,
    note: String,
    help: String,
}

impl NoExpectMessages {
    fn new(primary: String, note: String, help: String) -> Self {
        Self {
            primary,
            note,
            help,
        }
    }

    fn primary(&self) -> &str {
        &self.primary
    }

    fn note(&self) -> &str {
        &self.note
    }

    fn help(&self) -> &str {
        &self.help
    }
}

trait BundleLookup {
    fn message(&self, key: &str, args: &Arguments<'_>) -> Result<String, I18nError>;
    fn attribute(
        &self,
        key: &str,
        attribute: &str,
        args: &Arguments<'_>,
    ) -> Result<String, I18nError>;
}

impl BundleLookup for Localiser {
    fn message(&self, key: &str, args: &Arguments<'_>) -> Result<String, I18nError> {
        self.message_with_args(key, args)
    }

    fn attribute(
        &self,
        key: &str,
        attribute: &str,
        args: &Arguments<'_>,
    ) -> Result<String, I18nError> {
        self.attribute_with_args(key, attribute, args)
    }
}

#[cfg(test)]
mod localisation {
    use super::{
        Arguments, BundleLookup, I18nError, Localiser, MESSAGE_KEY, NoExpectMessages,
        context_label, fallback_messages, localised_messages,
    };
    use crate::context::ContextSummary;
    use rstest::fixture;
    use rstest_bdd_macros::{given, scenario, then, when};
    use std::cell::RefCell;

    #[derive(Default)]
    struct LocalisationWorld {
        localiser: RefCell<Option<Localiser>>,
        receiver: RefCell<String>,
        summary: RefCell<ContextSummary>,
        failing: RefCell<bool>,
        result: RefCell<Option<Result<NoExpectMessages, I18nError>>>,
    }

    impl LocalisationWorld {
        fn use_localiser(&self, locale: &str) {
            let localiser = Localiser::new(Some(locale));
            *self.localiser.borrow_mut() = Some(localiser);
        }

        fn set_receiver(&self, receiver: &str) {
            *self.receiver.borrow_mut() = receiver.to_string();
        }

        fn set_function(&self, name: Option<&str>) {
            let mut summary = self.summary.borrow_mut();
            summary.function_name = name.map(ToString::to_string);
        }

        fn record_result(&self, value: Result<NoExpectMessages, I18nError>) {
            *self.result.borrow_mut() = Some(value);
        }

        fn messages(&self) -> &NoExpectMessages {
            self.result
                .borrow()
                .as_ref()
                .expect("result recorded")
                .as_ref()
                .expect("expected localisation to succeed")
        }

        fn error(&self) -> &I18nError {
            self.result
                .borrow()
                .as_ref()
                .expect("result recorded")
                .as_ref()
                .expect_err("expected localisation to fail")
        }
    }

    #[fixture]
    fn world() -> LocalisationWorld {
        LocalisationWorld::default()
    }

    #[given("the locale {locale} is selected")]
    fn given_locale(world: &LocalisationWorld, locale: String) {
        world.use_localiser(&locale);
    }

    #[given("the receiver type is {receiver}")]
    fn given_receiver(world: &LocalisationWorld, receiver: String) {
        world.set_receiver(&receiver);
    }

    #[given("the function context is {name}")]
    fn given_function(world: &LocalisationWorld, name: String) {
        let value = if name.is_empty() {
            None
        } else {
            Some(name.as_str())
        };
        world.set_function(value);
    }

    #[given("the call occurs outside any function")]
    fn given_no_function(world: &LocalisationWorld) {
        world.set_function(None);
    }

    #[given("localisation fails")]
    fn given_failure(world: &LocalisationWorld) {
        *world.failing.borrow_mut() = true;
    }

    #[when("I localise the expect diagnostic")]
    fn when_localise(world: &LocalisationWorld) {
        let receiver = world.receiver.borrow().clone();
        let summary = world.summary.borrow().clone();
        let context = context_label(&summary);

        let result = if *world.failing.borrow() {
            localised_messages(&FailingLookup, receiver.as_str(), context.as_str())
        } else {
            let localiser = world
                .localiser
                .borrow()
                .as_ref()
                .expect("a locale must be selected");
            localised_messages(localiser, receiver.as_str(), context.as_str())
        };

        world.record_result(result);
    }

    #[then("the diagnostic mentions {snippet}")]
    fn then_primary(world: &LocalisationWorld, snippet: String) {
        assert!(world.messages().primary().contains(&snippet));
    }

    #[then("the note references {snippet}")]
    fn then_note(world: &LocalisationWorld, snippet: String) {
        assert!(world.messages().note().contains(&snippet));
    }

    #[then("the help references {snippet}")]
    fn then_help(world: &LocalisationWorld, snippet: String) {
        assert!(world.messages().help().contains(&snippet));
    }

    #[then("localisation fails for {key}")]
    fn then_failure(world: &LocalisationWorld, key: String) {
        let error = world.error();
        match error {
            I18nError::MissingMessage { key: missing, .. } => assert_eq!(missing, &key),
        }
    }

    #[scenario(path = "tests/features/localisation.feature", index = 0)]
    fn scenario_fallback(world: LocalisationWorld) {
        let _ = world;
    }

    #[scenario(path = "tests/features/localisation.feature", index = 1)]
    fn scenario_cymraeg(world: LocalisationWorld) {
        let _ = world;
    }

    #[scenario(path = "tests/features/localisation.feature", index = 2)]
    fn scenario_unknown_locale(world: LocalisationWorld) {
        let _ = world;
    }

    #[scenario(path = "tests/features/localisation.feature", index = 3)]
    fn scenario_failure(world: LocalisationWorld) {
        let _ = world;
    }

    #[then("the fallback helper mentions {snippet}")]
    fn then_fallback(world: &LocalisationWorld, snippet: String) {
        let summary = world.summary.borrow().clone();
        let context = context_label(&summary);
        let fallback = fallback_messages(world.receiver.borrow().as_str(), context.as_str());
        assert!(fallback.primary().contains(&snippet));
    }

    struct FailingLookup;

    impl BundleLookup for FailingLookup {
        fn message(&self, _key: &str, _args: &Arguments<'_>) -> Result<String, I18nError> {
            Err(I18nError::MissingMessage {
                key: MESSAGE_KEY.to_string(),
                locale: "test".to_string(),
            })
        }

        fn attribute(
            &self,
            _key: &str,
            _attribute: &str,
            _args: &Arguments<'_>,
        ) -> Result<String, I18nError> {
            Err(I18nError::MissingMessage {
                key: MESSAGE_KEY.to_string(),
                locale: "test".to_string(),
            })
        }
    }
}

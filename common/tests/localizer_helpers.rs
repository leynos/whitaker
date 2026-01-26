//! Behaviour-driven coverage for the localiser and diagnostic helpers.
//!
//! Scenarios validate locale resolution and fallback handling so lints can
//! depend on deterministic localisation outcomes.

use common::i18n::testing::RecordingEmitter;
use common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    get_localizer_for_lint, noop_reporter, safe_resolve_message_set,
};
use common::test_support::LocaleOverride;
use logtest::Logger;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, scenario, then, when};
use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard};

static ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|stripped| stripped.strip_suffix('"'))
        .unwrap_or(value)
}

struct HelperWorld {
    configuration: RefCell<Option<String>>,
    localizer: RefCell<Option<Localizer>>,
    arguments: RefCell<Arguments<'static>>,
    message_key: RefCell<Option<String>>,
    fallback: RefCell<Option<DiagnosticMessageSet>>,
    result: RefCell<Option<DiagnosticMessageSet>>,
    emitter: RecordingEmitter,
    environment_override: RefCell<Option<LocaleOverride>>,
    _guard: MutexGuard<'static, ()>,
}

impl HelperWorld {
    fn new() -> Self {
        let guard = ENVIRONMENT_LOCK
            .lock()
            .unwrap_or_else(|error| panic!("environment lock poisoned: {error}"));

        Self {
            configuration: RefCell::new(None),
            localizer: RefCell::new(None),
            arguments: RefCell::new(Arguments::default()),
            message_key: RefCell::new(None),
            fallback: RefCell::new(None),
            result: RefCell::new(None),
            emitter: RecordingEmitter::default(),
            environment_override: RefCell::new(None),
            _guard: guard,
        }
    }

    fn set_environment(&self, value: Option<String>) {
        let mut guard = self.environment_override.borrow_mut();
        guard.take();
        let override_guard = match value {
            Some(locale) => LocaleOverride::set(locale.as_str()),
            None => LocaleOverride::clear(),
        };
        *guard = Some(override_guard);
    }

    fn set_configuration(&self, locale: Option<String>) {
        *self.configuration.borrow_mut() = locale;
    }

    fn request_localizer(&self, lint: &str) {
        let config = self.configuration.borrow();
        let localizer = get_localizer_for_lint(lint, config.as_deref());
        self.localizer.borrow_mut().replace(localizer);
    }

    fn ensure_localizer(&self) -> Localizer {
        self.localizer
            .borrow()
            .as_ref()
            .cloned()
            .unwrap_or_else(|| panic!("localizer should be initialised"))
    }

    fn assert_locale(&self, expected: &str) {
        let localizer = self.ensure_localizer();
        assert_eq!(localizer.locale(), expected);
    }

    fn set_message_key(&self, key: String) {
        self.message_key.borrow_mut().replace(key);
    }

    fn set_fallback_messages(&self) {
        let fallback = DiagnosticMessageSet::new(
            "Fallback primary".into(),
            "Fallback note".into(),
            "Fallback help".into(),
        );
        self.fallback.borrow_mut().replace(fallback);
    }

    fn ensure_fallback(&self) -> DiagnosticMessageSet {
        self.fallback
            .borrow_mut()
            .get_or_insert_with(|| {
                DiagnosticMessageSet::new(
                    "Fallback primary".into(),
                    "Fallback note".into(),
                    "Fallback help".into(),
                )
            })
            .clone()
    }

    fn clear_arguments(&self) {
        *self.arguments.borrow_mut() = Arguments::default();
    }

    fn prepare_doc_arguments(&self) {
        let mut args: Arguments<'static> = Arguments::default();
        args.insert(Cow::Borrowed("subject"), FluentValue::from("functions"));
        args.insert(
            Cow::Borrowed("attribute"),
            FluentValue::from("#[inline]".to_string()),
        );
        *self.arguments.borrow_mut() = args;
    }

    fn resolve_messages(&self) {
        let localizer = self.ensure_localizer();
        let key = self
            .message_key
            .borrow()
            .clone()
            .unwrap_or_else(|| panic!("a message key should be configured"));
        let args = self.arguments.borrow().clone();
        let fallback = self.ensure_fallback();

        let resolution = MessageResolution {
            lint_name: "helper-tests",
            key: MessageKey::new(key.as_str()),
            args: &args,
        };
        let messages = safe_resolve_message_set(
            &localizer,
            resolution,
            |message| {
                self.emitter.record(message);
            },
            || fallback.clone(),
        );

        self.result.borrow_mut().replace(messages);
    }

    fn resolved_messages(&self) -> DiagnosticMessageSet {
        self.result
            .borrow()
            .as_ref()
            .cloned()
            .unwrap_or_else(|| panic!("diagnostic messages should be resolved"))
    }

    fn recorded_messages(&self) -> Vec<String> {
        self.emitter.recorded_messages()
    }
}

#[fixture]
fn world() -> HelperWorld {
    HelperWorld::new()
}

#[given("DYLINT_LOCALE is not set")]
fn given_env_cleared(world: &HelperWorld) {
    world.set_environment(None);
}

#[given("DYLINT_LOCALE is {locale}")]
fn given_env(world: &HelperWorld, locale: String) {
    world.set_environment(Some(unquote(&locale).to_string()));
}

#[given("no configuration locale is provided")]
fn given_no_config(world: &HelperWorld) {
    world.set_configuration(None);
}

#[given("the configuration locale is {locale}")]
fn given_config(world: &HelperWorld, locale: String) {
    world.set_configuration(Some(unquote(&locale).to_string()));
}

#[when("I request the localizer for {lint}")]
#[given("I have requested the localizer for {lint}")]
fn when_request_localizer(world: &HelperWorld, lint: String) {
    world.request_localizer(unquote(&lint));
}

#[then("the resolved locale is {locale}")]
fn then_locale(world: &HelperWorld, locale: String) {
    world.assert_locale(unquote(&locale));
}

#[given("fallback messages are defined")]
fn given_fallback(world: &HelperWorld) {
    world.set_fallback_messages();
}

#[given("a missing message key {key} is requested")]
fn given_missing_key(world: &HelperWorld, key: String) {
    world.set_message_key(unquote(&key).to_string());
}

#[given("a message key {key} is requested")]
fn given_message_key(world: &HelperWorld, key: String) {
    world.set_message_key(unquote(&key).to_string());
}

#[given("I prepare arguments for the doc attribute diagnostic")]
fn given_doc_arguments(world: &HelperWorld) {
    world.prepare_doc_arguments();
}

#[given("I do not prepare arguments for the doc attribute diagnostic")]
fn given_no_doc_arguments(world: &HelperWorld) {
    world.clear_arguments();
}

#[when("I resolve the diagnostic message set")]
fn when_resolve_messages(world: &HelperWorld) {
    world.resolve_messages();
}

#[then("the fallback primary message contains {snippet}")]
fn then_fallback_primary(world: &HelperWorld, snippet: String) {
    let messages = world.resolved_messages();
    let snippet = unquote(&snippet);
    assert!(messages.primary().contains(snippet));
}

#[then("a delayed bug is recorded mentioning {snippet}")]
fn then_bug_recorded(world: &HelperWorld, snippet: String) {
    let messages = world.recorded_messages();
    let snippet = unquote(&snippet);
    assert!(!messages.is_empty());
    assert!(messages.iter().any(|message| message.contains(snippet)));
}

#[then("the resolved primary message contains {snippet}")]
fn then_primary_message(world: &HelperWorld, snippet: String) {
    let messages = world.resolved_messages();
    let snippet = unquote(&snippet);
    assert!(messages.primary().contains(snippet));
}

#[then("no delayed bug is recorded")]
fn then_no_bug(world: &HelperWorld) {
    assert!(world.recorded_messages().is_empty());
}

#[scenario("tests/features/localizer_helpers.feature", index = 0)]
fn scenario_fallback_to_default(world: HelperWorld) {
    let _ = world;
}

#[scenario("tests/features/localizer_helpers.feature", index = 1)]
fn scenario_environment_locale(world: HelperWorld) {
    let _ = world;
}

#[scenario("tests/features/localizer_helpers.feature", index = 2)]
fn scenario_localisation_fallback(world: HelperWorld) {
    let _ = world;
}

#[scenario("tests/features/localizer_helpers.feature", index = 3)]
fn scenario_localisation_success(world: HelperWorld) {
    let _ = world;
}

#[scenario("tests/features/localizer_helpers.feature", index = 4)]
fn scenario_interpolation_failure(world: HelperWorld) {
    let _ = world;
}

#[test]
fn invalid_locale_warns_and_falls_back() {
    let mut logger = Logger::start();
    let world = HelperWorld::new();
    world.set_environment(Some(String::from("xx-XX")));
    world.set_configuration(None);
    world.request_localizer("function_attrs_follow_docs");
    world.assert_locale("en-GB");

    let mut warned = false;
    while let Some(record) = logger.pop() {
        if record
            .args()
            .to_string()
            .contains("unsupported DYLINT_LOCALE `xx-XX`")
        {
            warned = true;
            break;
        }
    }

    assert!(warned, "expected unsupported locale warning to be logged");
}

#[test]
fn repeated_failures_record_all_bugs() {
    let world = HelperWorld::new();
    world.set_environment(None);
    world.set_configuration(None);
    world.request_localizer("no_expect_outside_tests");
    world.set_fallback_messages();
    world.set_message_key(String::from("missing-key"));

    world.resolve_messages();
    world.resolve_messages();

    let recorded = world.recorded_messages();
    assert_eq!(recorded.len(), 2);
    assert!(
        recorded
            .into_iter()
            .all(|message| message.contains("missing-key"))
    );
}

#[rstest]
fn missing_key_with_noop_reporter_uses_fallback(world: HelperWorld) {
    world.set_environment(None);
    world.set_configuration(None);
    world.request_localizer("no_expect_outside_tests");
    world.set_fallback_messages();

    let localizer = world.ensure_localizer();
    let args = world.arguments.borrow().clone();
    let fallback = world.ensure_fallback();
    let resolution = MessageResolution {
        lint_name: "helper-tests",
        key: MessageKey::new("missing-key"),
        args: &args,
    };

    let messages =
        safe_resolve_message_set(&localizer, resolution, noop_reporter, || fallback.clone());

    assert_eq!(messages.primary(), fallback.primary());
    assert_eq!(messages.note(), fallback.note());
    assert_eq!(messages.help(), fallback.help());
}

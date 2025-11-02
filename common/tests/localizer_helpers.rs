//! Behaviour-driven coverage for the localiser and diagnostic helpers.
//!
//! Scenarios validate locale resolution and fallback handling so lints can
//! depend on deterministic localisation outcomes.

use common::i18n::testing::RecordingEmitter;
use common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    get_localizer_for_lint, safe_resolve_message_set,
};
use once_cell::sync::Lazy;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::borrow::Cow;
use std::cell::RefCell;
use std::env;
use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard};

static ENVIRONMENT_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

struct HelperWorld {
    _guard: MutexGuard<'static, ()>,
    original_env: Option<OsString>,
    configuration: RefCell<Option<String>>,
    localizer: RefCell<Option<Localizer>>,
    arguments: RefCell<Arguments<'static>>,
    message_key: RefCell<Option<String>>,
    fallback: RefCell<Option<DiagnosticMessageSet>>,
    result: RefCell<Option<DiagnosticMessageSet>>,
    emitter: RecordingEmitter,
}

impl HelperWorld {
    fn new() -> Self {
        let guard = ENVIRONMENT_LOCK
            .lock()
            .unwrap_or_else(|error| panic!("environment lock poisoned: {error}"));
        let original_env = env::var_os("DYLINT_LOCALE");

        Self {
            _guard: guard,
            original_env,
            configuration: RefCell::new(None),
            localizer: RefCell::new(None),
            arguments: RefCell::new(Arguments::default()),
            message_key: RefCell::new(None),
            fallback: RefCell::new(None),
            result: RefCell::new(None),
            emitter: RecordingEmitter::default(),
        }
    }

    fn set_environment(&self, value: Option<String>) {
        match value {
            Some(locale) => env::set_var("DYLINT_LOCALE", locale),
            None => env::remove_var("DYLINT_LOCALE"),
        }
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
            .expect("localizer should be initialised")
            .clone()
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
            .expect("a message key should be configured");
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
            .expect("diagnostic messages should be resolved")
    }

    fn recorded_messages(&self) -> Vec<String> {
        self.emitter.recorded_messages()
    }
}

impl Drop for HelperWorld {
    fn drop(&mut self) {
        if let Some(value) = &self.original_env {
            env::set_var("DYLINT_LOCALE", value);
        } else {
            env::remove_var("DYLINT_LOCALE");
        }
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
    world.set_environment(Some(locale));
}

#[given("no configuration locale is provided")]
fn given_no_config(world: &HelperWorld) {
    world.set_configuration(None);
}

#[given("the configuration locale is {locale}")]
fn given_config(world: &HelperWorld, locale: String) {
    world.set_configuration(Some(locale));
}

#[when("I request the localizer for {lint}")]
#[given("I have requested the localizer for {lint}")]
fn when_request_localizer(world: &HelperWorld, lint: String) {
    world.request_localizer(&lint);
}

#[then("the resolved locale is {locale}")]
fn then_locale(world: &HelperWorld, locale: String) {
    world.assert_locale(&locale);
}

#[given("fallback messages are defined")]
fn given_fallback(world: &HelperWorld) {
    world.set_fallback_messages();
}

#[given("a missing message key {key} is requested")]
fn given_missing_key(world: &HelperWorld, key: String) {
    world.set_message_key(key);
}

#[given("a message key {key} is requested")]
fn given_message_key(world: &HelperWorld, key: String) {
    world.set_message_key(key);
}

#[given("I prepare arguments for the doc attribute diagnostic")]
fn given_doc_arguments(world: &HelperWorld) {
    world.prepare_doc_arguments();
}

#[when("I resolve the diagnostic message set")]
fn when_resolve_messages(world: &HelperWorld) {
    world.resolve_messages();
}

#[then("the fallback primary message contains {snippet}")]
fn then_fallback_primary(world: &HelperWorld, snippet: String) {
    let messages = world.resolved_messages();
    assert!(messages.primary().contains(&snippet));
}

#[then("a delayed bug is recorded mentioning {snippet}")]
fn then_bug_recorded(world: &HelperWorld, snippet: String) {
    let messages = world.recorded_messages();
    assert!(!messages.is_empty());
    assert!(messages[0].contains(&snippet));
}

#[then("the resolved primary message contains {snippet}")]
fn then_primary_message(world: &HelperWorld, snippet: String) {
    let messages = world.resolved_messages();
    assert!(messages.primary().contains(&snippet));
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

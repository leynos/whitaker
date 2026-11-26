//! Tests for the localized diagnostics and the configuration warnings.
//!
//! These resolve every message through the same path emission uses, so a
//! catalogue that fell back, lost a frozen token, or was never translated
//! fails here without a compiler run.

use rstest::rstest;
use whitaker_common::i18n::{
    DiagnosticMessageSet,
    FALLBACK_LOCALE,
    FluentValue,
    Localizer,
    available_locales,
    normalize_fluent_output,
    resolve_message_set,
    testing::RecordingEmitter,
};

use super::{
    ConfigPayload,
    ConfigWarning,
    PACKAGE_VERSION,
    StdFsMessages,
    localized_messages,
    resolve_messages,
    sanitize_message,
};
use crate::config::suite_revision;

/// The operation every locale renders into its messages.
const OPERATION: &str = "std::fs::read";

/// Revision the tests pretend the suite was built from.
const TEST_REVISION: &str = "abc1234";

/// Resolve the messages for `locale` through the lint's production path,
/// failing if the lookup falls back to the built-in English messages.
fn production_messages(locale: &str) -> StdFsMessages {
    let localizer = Localizer::new(Some(locale));
    assert!(
        !localizer.used_fallback(),
        "`{locale}` should load its own catalogue"
    );
    let reports = RecordingEmitter::default();
    let messages = resolve_messages(&localizer, OPERATION, |report| reports.record(report));
    assert!(
        reports.recorded_messages().is_empty(),
        "`{locale}` fell back to the built-in messages: {:?}",
        reports.recorded_messages()
    );
    messages
}

/// Check that every shipped locale resolves this lint's messages through
/// the path `emit_diagnostic` uses, keeps its frozen tokens, and is not a
/// copy of the English source. The `ui-*` fixtures cover only some locales
/// through the compiler; this covers the rest without a compiler run.
#[test]
fn every_locale_renders_the_lint_messages() {
    let english = production_messages(FALLBACK_LOCALE);
    for locale in available_locales() {
        let messages = production_messages(locale);
        assert!(
            messages.primary().contains(OPERATION),
            "`{locale}` primary message lost the operation: {}",
            messages.primary()
        );
        assert!(
            messages.help().contains("`cap_std::fs::Dir`"),
            "`{locale}` help lost the capability type: {}",
            messages.help()
        );
        if !locale.starts_with("en") {
            assert_ne!(
                messages.primary(),
                english.primary(),
                "`{locale}` should not render the English source text"
            );
        }
    }
}

/// Check that the production path renders exactly what the lint's
/// lookup resolver does, so it cannot drift to other arguments or keys.
#[test]
fn production_path_matches_the_lookup_resolver() {
    for locale in available_locales() {
        let localizer = Localizer::new(Some(locale));
        let resolved = localized_messages(&localizer, OPERATION)
            .unwrap_or_else(|error| panic!("`{locale}` should resolve: {error}"));
        let production = production_messages(locale);
        assert_eq!(
            production.primary(),
            normalize_fluent_output(resolved.primary().to_owned()),
            "`{locale}` production and resolver primary messages differ"
        );
        assert_eq!(
            production.help(),
            normalize_fluent_output(resolved.help().to_owned()),
            "`{locale}` production and resolver help messages differ"
        );
    }
}

/// Check that directional isolation marks are removed from messages.
#[test]
fn removes_isolation_marks() {
    let raw = "\u{2068}std::fs::read\u{2069}";
    assert_eq!(sanitize_message(raw), "std::fs::read");
}

/// Check that sanitizing plain text leaves it unchanged.
#[test]
fn preserves_clean_text() {
    let raw = "std::fs::File::open";
    assert_eq!(sanitize_message(raw), raw);
}

/// Check that every locale names the payload, the package version, and the
/// suite revision the warning was built from.
#[rstest]
#[case::unknown_keys(
    ConfigPayload::UnknownKeys("\"future_option\"".to_owned()),
    "future_option"
)]
#[case::discarded_section(
    ConfigPayload::DiscardedSection("\"bad value\"".to_owned()),
    "bad value"
)]
fn config_warnings_resolve_in_every_locale(#[case] payload: ConfigPayload, #[case] needle: &str) {
    for shipped in available_locales() {
        let locale = shipped.as_str();
        let messages = resolve_warning(
            &ConfigWarning::with_revision(&payload, TEST_REVISION),
            locale,
        );

        assert!(
            messages.primary().contains("[no_std_fs_operations]"),
            "{locale}: primary message should name the section, got {}",
            messages.primary()
        );
        assert!(
            messages.primary().contains(needle),
            "{locale}: primary message should name {needle}, got {}",
            messages.primary()
        );
        assert!(
            messages.primary().contains(PACKAGE_VERSION),
            "{locale}: primary message should carry the package version"
        );
        assert!(
            messages.primary().contains(TEST_REVISION),
            "{locale}: primary message should carry the suite revision"
        );
        assert!(!messages.note().is_empty(), "{locale}: note is empty");
        assert!(!messages.help().is_empty(), "{locale}: help is empty");
    }
}

/// Check that a non-English catalogue is translated, not copied.
///
/// The shared assertions above pass on an untranslated catalogue, because
/// the section name, the payload, the version, and the revision are all
/// literals a copy would preserve. Comparing each non-English primary
/// message against `en-GB` for the same payload and arguments is what
/// catches a catalogue that was never translated.
#[rstest]
#[case::unknown_keys(
    ConfigPayload::UnknownKeys("\"future_option\"".to_owned())
)]
#[case::discarded_section(
    ConfigPayload::DiscardedSection("\"bad value\"".to_owned())
)]
fn config_warning_translations_differ_from_english(#[case] payload: ConfigPayload) {
    let english = resolve_warning(
        &ConfigWarning::with_revision(&payload, TEST_REVISION),
        FALLBACK_LOCALE,
    );
    // Every shipped locale is checked, not a sample: the parity gate proves
    // structure and placeables, but only rendering catches a copied message.
    // The English variants share the source text by design, so they are exempt.
    for locale in available_locales()
        .iter()
        .map(String::as_str)
        .filter(|locale| !locale.starts_with("en"))
    {
        let messages = resolve_warning(
            &ConfigWarning::with_revision(&payload, TEST_REVISION),
            locale,
        );
        assert_ne!(
            messages.primary(),
            english.primary(),
            "{locale}: primary message is untranslated English"
        );
        assert_ne!(
            messages.note(),
            english.note(),
            "{locale}: note is untranslated English"
        );
        assert_ne!(
            messages.help(),
            english.help(),
            "{locale}: help is untranslated English"
        );
    }
}

/// Resolve one configuration warning through the localized resources.
fn resolve_warning(warning: &ConfigWarning<'_>, locale: &str) -> DiagnosticMessageSet {
    let args = warning.arguments();
    resolve_message_set(
        &Localizer::new(Some(locale)),
        warning.payload.message_key(),
        &args,
    )
    .expect("configuration messages should resolve for every supported locale")
}

/// Carry the compiled revision into a warning built by the production path.
///
/// Every other warning test supplies a revision through `with_revision`, so
/// this is the only check that `ConfigWarning::new` reads the compiled value at
/// all. It fails if `suite_revision` is replaced by a constant that differs
/// from what the build embedded.
#[test]
fn a_production_warning_carries_the_compiled_revision() {
    let payload = ConfigPayload::UnknownKeys("\"future_option\"".to_owned());
    let warning = ConfigWarning::new(&payload);
    let rendered = warning.arguments();
    let revision = rendered
        .get("revision")
        .expect("a production warning should interpolate a revision");

    // The revision is interpolated as a string, so unwrap that arm rather than
    // rendering through Fluent, which would apply bidi isolation.
    let FluentValue::String(text) = revision else {
        panic!("a revision should interpolate as a string, got {revision:?}");
    };
    assert_eq!(
        text.as_ref(),
        suite_revision(),
        "the warning should carry the revision this build compiled in"
    );
}

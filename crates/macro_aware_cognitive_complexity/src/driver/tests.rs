//! Unit tests for the driver's compiler-free message and anchor paths.

use rstest::rstest;
use whitaker_common::{Localizer, i18n::DiagnosticMessageSet};

use super::{LINT_NAME, MESSAGE_KEY, anchor, diagnostic};

/// Verify the message key is the lint name used by the Fluent bundle.
#[test]
fn message_key_matches_lint_name() {
    assert_eq!(
        MESSAGE_KEY.as_ref(),
        LINT_NAME,
        "the message key must match the lint name"
    );
}

/// Verify closures share the documented placeholder subject.
#[test]
fn closure_subject_is_a_stable_placeholder() {
    assert_eq!(
        anchor::CLOSURE_SUBJECT,
        "<closure>",
        "the closure subject is part of the rendered message"
    );
}

/// Verify every supported locale renders a complete, substituted message set.
#[rstest]
#[case("en-GB")]
#[case("cy")]
#[case("gd")]
fn supported_locales_render_complete_messages(#[case] locale: &str) {
    let messages = rendered(locale);

    assert!(
        !messages.primary().is_empty(),
        "primary is empty for {locale}"
    );
    assert!(!messages.note().is_empty(), "note is empty for {locale}");
    assert!(!messages.help().is_empty(), "help is empty for {locale}");
    assert!(
        messages.primary().contains('3') && messages.primary().contains('2'),
        "score or limit missing for {locale}: {}",
        messages.primary()
    );
}

/// Verify the translations are distinct rather than copied English text.
#[test]
fn locales_render_distinct_text() {
    let english = rendered("en-GB");
    let welsh = rendered("cy");
    let gaelic = rendered("gd");

    assert_ne!(
        english.primary(),
        welsh.primary(),
        "Welsh primary text must differ from English"
    );
    assert_ne!(
        english.primary(),
        gaelic.primary(),
        "Gaelic primary text must differ from English"
    );
    assert_ne!(
        welsh.primary(),
        gaelic.primary(),
        "Welsh and Gaelic primary text must differ"
    );
    assert_ne!(
        english.note(),
        welsh.note(),
        "Welsh note must differ from English"
    );
    assert_ne!(
        english.note(),
        gaelic.note(),
        "Gaelic note must differ from English"
    );
    assert_ne!(
        english.help(),
        welsh.help(),
        "Welsh help must differ from English"
    );
    assert_ne!(
        english.help(),
        gaelic.help(),
        "Gaelic help must differ from English"
    );
}

/// Verify an unknown locale falls back to the English render byte for byte.
#[test]
fn unknown_locale_falls_back_to_english() {
    let fallback_localizer = Localizer::new(Some("zz"));
    assert_eq!(
        fallback_localizer.locale(),
        "en-GB",
        "an unsupported locale must resolve to the fallback"
    );

    let fallback = diagnostic::render(&fallback_localizer, "sample", 3, 2);
    let english = rendered("en-GB");

    assert_eq!(
        fallback.primary(),
        english.primary(),
        "unknown locales must reuse the English primary text"
    );
    assert_eq!(
        fallback.note(),
        english.note(),
        "unknown locales must reuse the English note"
    );
    assert_eq!(
        fallback.help(),
        english.help(),
        "unknown locales must reuse the English help text"
    );
}

/// Render the sample diagnostic in one locale.
fn rendered(locale: &str) -> DiagnosticMessageSet {
    diagnostic::render(&Localizer::new(Some(locale)), "sample", 3, 2)
}

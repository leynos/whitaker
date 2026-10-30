//! Tests for the i18n localizer: message lookup, argument substitution,
//! and locale fallback behaviour.

use std::borrow::Cow;

use rstest::rstest;

use super::{
    Arguments,
    AttrKey,
    DiagnosticMessageSet,
    FALLBACK_LOCALE,
    FluentValue,
    LabelResolution,
    Localizer,
    MessageKey,
    MessageResolution,
    available_locales,
    normalize_fluent_output,
    safe_resolve_label,
    supports_locale,
    testing::RecordingEmitter,
};

#[rstest]
#[case(None, FALLBACK_LOCALE, true)]
#[case(Some("en-GB"), "en-GB", false)]
#[case(Some("cy"), "cy", false)]
#[case(Some("gd"), "gd", false)]
#[case(Some("zz"), FALLBACK_LOCALE, true)]
fn resolves_locales(#[case] input: Option<&str>, #[case] expected: &str, #[case] fallback: bool) {
    let localizer = Localizer::new(input);
    assert_eq!(localizer.locale(), expected);
    assert_eq!(localizer.used_fallback(), fallback);
}

#[test]
fn enumerates_available_locales() {
    let locales = available_locales();
    assert!(locales.contains(&"en-GB".to_owned()));
    assert!(locales.contains(&"cy".to_owned()));
    assert!(locales.contains(&"gd".to_owned()));
}

#[test]
fn supports_locale_reports_known_languages() {
    assert!(supports_locale("en-GB"));
    assert!(supports_locale("cy"));
    assert!(supports_locale("gd"));
    assert!(!supports_locale("zz"));
}

#[test]
fn message_lookup_with_arguments_interpolates_values() {
    let localizer = Localizer::new(Some("gd"));
    let mut args = Arguments::new();
    args.insert(
        Cow::Borrowed("lint"),
        FluentValue::from("function_attrs_follow_docs"),
    );
    let message = localizer
        .message_with_args("common-lint-count", &args)
        .expect("message should exist");
    assert!(message.contains("function_attrs_follow_docs"));
}

#[test]
fn safe_resolves_bumpy_label_with_its_fluent_arguments() {
    let localizer = Localizer::new(Some(FALLBACK_LOCALE));
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(Cow::Borrowed("index"), FluentValue::from(2_i64));
    args.insert(Cow::Borrowed("lines"), FluentValue::from(6_i64));
    let resolution = MessageResolution {
        lint_name: "bumpy_road_function",
        key: MessageKey::new("bumpy_road_function"),
        args: &args,
    };
    let report = RecordingEmitter::default();

    let label = safe_resolve_label(
        &localizer,
        LabelResolution {
            message: resolution,
            attribute: AttrKey::new("label"),
        },
        |message| report.record(message),
        || String::from("fallback label"),
    );

    assert_eq!(label, "Complexity bump 2 spans 6 lines.");
    assert!(report.recorded_messages().is_empty());
}

#[test]
fn safe_resolves_missing_attribute_with_fallback_and_report() {
    let localizer = Localizer::new(Some(FALLBACK_LOCALE));
    let args: Arguments<'static> = Arguments::default();
    let resolution = MessageResolution {
        lint_name: "bumpy_road_function",
        key: MessageKey::new("bumpy_road_function"),
        args: &args,
    };
    let report = RecordingEmitter::default();

    let label = safe_resolve_label(
        &localizer,
        LabelResolution {
            message: resolution,
            attribute: AttrKey::new("missing-label"),
        },
        |message| report.record(message),
        || String::from("fall\u{2068}back\u{2069}\u{FFFD}"),
    );

    assert_eq!(label, "fallback\u{FFFD}");
    let reports = report.recorded_messages();
    assert_eq!(reports.len(), 1);
    assert!(
        reports
            .first()
            .is_some_and(|message| message.contains("bumpy_road_function.missing-label"))
    );
}

#[test]
fn normalizes_fluent_isolates_and_preserves_replacement_character() {
    let text = String::from("before\u{2068}value\u{2069}\u{FFFD}after");

    assert_eq!(normalize_fluent_output(text), "beforevalue\u{FFFD}after");
}

#[test]
fn message_set_normalization_preserves_replacement_characters() {
    let messages = DiagnosticMessageSet::new(
        String::from("primary\u{2068}\u{FFFD}\u{2069}"),
        String::from("note\u{2068}\u{FFFD}\u{2069}"),
        String::from("help\u{2068}\u{FFFD}\u{2069}"),
    )
    .strip_isolating_marks();

    assert_eq!(messages.primary(), "primary\u{FFFD}");
    assert_eq!(messages.note(), "note\u{FFFD}");
    assert_eq!(messages.help(), "help\u{FFFD}");
}

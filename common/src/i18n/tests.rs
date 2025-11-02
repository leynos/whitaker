use std::borrow::Cow;

use fluent_bundle::FluentValue;
use rstest::rstest;

use super::{Arguments, FALLBACK_LOCALE, Localizer, available_locales, supports_locale};

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
    assert!(locales.contains(&"en-GB".to_string()));
    assert!(locales.contains(&"cy".to_string()));
    assert!(locales.contains(&"gd".to_string()));
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

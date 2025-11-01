//! Quality gates for localisation resources.
//!
//! These tests keep placeholder usage aligned across locales, ensure Welsh help
//! text remains complete, and exercise language-specific plural forms so we can
//! catch regressions before they reach users.

use common::i18n::Localizer;
use fluent_bundle::FluentValue;
use once_cell::sync::Lazy;
use regex::Regex;
use rstest::rstest;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

#[path = "support/mod.rs"]
mod support;
use support::{FtlEntry, LocaleCode, LocaleContext, MessageId, file_pairs, parse_ftl};

static PLACEABLE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{\s*\$([A-Za-z0-9_]+)").expect("valid placeable regex"));

fn extract_placeables(text: &str) -> BTreeSet<String> {
    PLACEABLE_RE
        .captures_iter(text)
        .map(|captures| captures[1].to_string())
        .collect()
}

fn validate_message_placeables(
    context: &LocaleContext<'_>,
    message_id: &MessageId,
    en_entry: &FtlEntry,
    locale_entry: &FtlEntry,
) {
    let en_placeables = extract_placeables(&en_entry.value);
    let locale_placeables = extract_placeables(&locale_entry.value);
    let locale = context.locale;
    assert_eq!(
        en_placeables,
        locale_placeables,
        "placeables diverged for `{message_id}` in {locale} ({})",
        context.path.display()
    );
}

fn validate_attribute_placeables(
    context: &LocaleContext<'_>,
    message_id: &MessageId,
    en_entry: &FtlEntry,
    locale_entry: &FtlEntry,
) {
    let attribute_names: BTreeSet<_> = en_entry
        .attributes
        .keys()
        .chain(locale_entry.attributes.keys())
        .cloned()
        .collect();

    for attribute in attribute_names {
        if let (Some(en_value), Some(locale_value)) = (
            en_entry.attributes.get(&attribute),
            locale_entry.attributes.get(&attribute),
        ) {
            let en_placeables = extract_placeables(en_value);
            let locale_placeables = extract_placeables(locale_value);
            let locale = context.locale;
            assert_eq!(
                en_placeables,
                locale_placeables,
                "placeables diverged for `{message_id}.{attribute}` in {locale} ({})",
                context.path.display()
            );
        }
    }
}

fn validate_entry_placeables(
    locale: &LocaleCode,
    message_id: &MessageId,
    en_entry: &FtlEntry,
    locale_entries: &BTreeMap<String, FtlEntry>,
    locale_path: &Path,
) {
    let locale_entry = locale_entries.get(message_id.as_str()).unwrap_or_else(|| {
        panic!(
            "{locale} locale missing message `{message_id}` in {}",
            locale_path.display()
        )
    });

    let context = LocaleContext {
        locale: locale.as_str(),
        path: locale_path,
    };

    validate_message_placeables(&context, message_id, en_entry, locale_entry);
    validate_attribute_placeables(&context, message_id, en_entry, locale_entry);
}

fn validate_pluralisation_coverage(locale: &str, max_branches: i64) {
    let localizer = Localizer::new(Some(locale));
    let mut args = HashMap::new();

    for branches in 0..=max_branches {
        args.insert(
            Cow::Borrowed("branches"),
            FluentValue::from(branches as i64),
        );
        let branch_phrase = branch_phrase(locale, branches);
        args.insert(
            Cow::Borrowed("branch_phrase"),
            FluentValue::from(branch_phrase.clone()),
        );
        let note = localizer
            .attribute_with_args("conditional_max_two_branches", "note", &args)
            .expect("conditional note should resolve");
        assert!(
            !note.contains('{'),
            "formatted note should not expose raw placeables: `{note}`"
        );
    }
}

#[test]
fn fluent_placeables_remain_in_sync() {
    for (locale, en_path, locale_path) in file_pairs() {
        let locale_code = LocaleCode::from(locale.as_str());
        let en_entries = parse_ftl(&en_path);
        let locale_entries = parse_ftl(&locale_path);

        for (message_id, en_entry) in &en_entries {
            let message_id = MessageId::from(message_id.as_str());
            validate_entry_placeables(
                &locale_code,
                &message_id,
                en_entry,
                &locale_entries,
                &locale_path,
            );
        }
    }
}

#[test]
fn localised_help_attributes_are_complete() {
    for (locale, en_path, locale_path) in file_pairs() {
        let locale_code = LocaleCode::from(locale.as_str());
        let en_entries = parse_ftl(&en_path);
        let locale_entries = parse_ftl(&locale_path);

        for (message_id, en_entry) in &en_entries {
            if en_entry.attributes.contains_key("help") {
                let message_id = MessageId::from(message_id.as_str());
                let locale_entry = locale_entries.get(message_id.as_str()).unwrap_or_else(|| {
                    panic!(
                        "{locale_code} locale missing message `{message_id}` in {}",
                        locale_path.display()
                    )
                });
                let help = locale_entry.attributes.get("help").unwrap_or_else(|| {
                    panic!(
                        "{locale_code} locale missing `.help` for `{message_id}` in {}",
                        locale_path.display()
                    )
                });
                assert!(
                    !help.trim().is_empty(),
                    ".help for `{message_id}` in {} must not be empty",
                    locale_path.display()
                );
            }
        }
    }
}

#[rstest]
#[case("en-GB", 12)]
#[case("cy", 12)]
#[case("gd", 25)]
fn pluralisation_covers_sample_range(#[case] locale: &str, #[case] max_branches: i64) {
    validate_pluralisation_coverage(locale, max_branches);
}

#[rstest]
#[case(0, "dim canghennau")]
#[case(1, "un gangen")]
#[case(2, "dwy gangen")]
#[case(3, "tri changen")]
#[case(6, "chwe changen")]
#[case(11, "11 canghennau")]
fn welsh_branch_term_declensions(#[case] branches: i64, #[case] expected: &str) {
    let localizer = Localizer::new(Some("cy"));
    let mut args = HashMap::new();
    let branch_phrase = welsh_branch_phrase(branches);
    assert_eq!(branch_phrase, expected);

    args.insert(
        Cow::Borrowed("branch_phrase"),
        FluentValue::from(branch_phrase.clone()),
    );
    args.insert(
        Cow::Borrowed("branches"),
        FluentValue::from(branches as i64),
    );
    let note = localizer
        .attribute_with_args("conditional_max_two_branches", "note", &args)
        .expect("conditional note should resolve");
    let expected_note = format!("Ar hyn o bryd mae {expected} yn y rheol.");
    assert_eq!(note, expected_note);
}

fn branch_phrase(locale: &str, branches: i64) -> String {
    match locale {
        "cy" => welsh_branch_phrase(branches),
        "gd" => gaelic_branch_phrase(branches),
        _ => english_branch_phrase(branches),
    }
}

fn english_branch_phrase(branches: i64) -> String {
    match branches {
        1 => "1 branch".to_string(),
        _ => format!("{branches} branches"),
    }
}

fn gaelic_branch_phrase(branches: i64) -> String {
    match branches {
        1 | 2 => format!("{branches} mheur"),
        3 => format!("{branches} meuran"),
        _ => format!("{branches} meur"),
    }
}

fn welsh_branch_phrase(branches: i64) -> String {
    match branches {
        0 => "dim canghennau".to_string(),
        1 => "un gangen".to_string(),
        2 => "dwy gangen".to_string(),
        3 => "tri changen".to_string(),
        6 => "chwe changen".to_string(),
        4 | 5 => format!("{branches} cangen"),
        _ => format!("{branches} canghennau"),
    }
}

#[test]
fn secondary_locales_fall_back_to_english_for_missing_attribute() {
    for locale in ["cy", "gd"] {
        let localizer = Localizer::new(Some(locale));
        let note = localizer
            .attribute("common-lint-count", "fallback-note")
            .expect("fallback attribute should resolve");
        assert_eq!(note, "Fallback diagnostics default to English.");
    }
}

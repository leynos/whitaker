//! Rule tables and checks for the catalogue parity gates.
//!
//! Each table records a deliberate decision that the translator's guide
//! (`docs/translators-guide.md`) explains. Exceptions are listed by locale and
//! key, so a new divergence from the source catalogue has to be named here
//! instead of passing unnoticed.

use std::{borrow::Cow, collections::BTreeSet};

use whitaker_common::i18n::{Arguments, FluentValue, branch_phrase};

use super::summary::{Catalogue, PatternSummary};

/// Tag of the source catalogue, which is also the loader's fallback locale.
pub const SOURCE_TAG: &str = "en-GB";

/// Every locale Whitaker ships, sorted as the loader reports them.
pub const EXPECTED_TAGS: [&str; 35] = [
    "ar", "cs", "cy", "da", "de", "el", "en-GB", "en-US", "es-419", "es-ES", "fa", "fi", "fr",
    "gd", "he", "hi", "hu", "id", "it", "ja", "ko", "nb", "nl", "pl", "pt-BR", "pt-PT", "ro", "ru",
    "sv", "th", "tr", "uk", "vi", "zh-Hans", "zh-Hant",
];

/// Keys a locale has not translated yet and resolves through the fallback.
const KNOWN_MISSING_KEYS: [(&str, &str); 1] = [("gd", "common-lint-count.fallback-note")];

/// Source variables a shipped translation deliberately leaves out.
///
/// Welsh and Gaelic reword these notes so that an English fragment supplied
/// by the lint (`$name`, `$reason`) need not appear inside translated prose.
const KNOWN_DROPPED_VARIABLES: [(&str, &str, &str); 5] = [
    ("cy", "conditional_max_n_branches.note", "name"),
    ("cy", "test_must_not_have_example.note", "reason"),
    ("cy", "test_must_not_have_example.note", "test"),
    ("gd", "conditional_max_n_branches.note", "name"),
    ("gd", "test_must_not_have_example.note", "reason"),
];

/// Variables a call site supplies beyond those the source pattern uses.
const CALL_SITE_VARIABLES: [(&str, &[&str]); 1] =
    [("conditional_max_n_branches", &["branches", "limit"])];

/// Pre-rendered phrases and the numeric variable that replaces each.
///
/// `branch_phrase` only renders English, Welsh, and Gaelic, so every other
/// locale must select on the number itself.
const PHRASE_SUBSTITUTES: [(&str, &str); 2] =
    [("branch_phrase", "branches"), ("limit_phrase", "limit")];

/// Languages for which `branch_phrase` renders a localized phrase.
const PHRASE_LANGUAGES: [&str; 3] = ["en", "cy", "gd"];

/// Variables whose selects choose a plural category.
const NUMERIC_SELECTORS: [&str; 4] = ["count", "lines", "branches", "limit"];

/// Cardinal plural categories of `intl_pluralrules` 7.0.2 (CLDR 37), by
/// language, for every language that differs from `one`/`other`.
const PLURAL_CATEGORIES: [(&str, &[&str]); 15] = [
    ("ar", &["zero", "one", "two", "few", "many", "other"]),
    ("cs", &["one", "few", "many", "other"]),
    ("cy", &["zero", "one", "two", "few", "many", "other"]),
    ("gd", &["one", "two", "few", "other"]),
    ("he", &["one", "two", "many", "other"]),
    ("id", &["other"]),
    ("ja", &["other"]),
    ("ko", &["other"]),
    ("pl", &["one", "few", "many", "other"]),
    ("ro", &["one", "few", "other"]),
    ("ru", &["one", "few", "many", "other"]),
    ("th", &["other"]),
    ("uk", &["one", "few", "many", "other"]),
    ("vi", &["other"]),
    ("zh", &["other"]),
];

/// Identifiers that appear in source text without backticks.
const BARE_LITERALS: [&str; 4] = ["std::fs", "unwrap_or_else", "camino", "Utf8PathBuf"];

/// Return the language subtag of a locale tag.
fn language(tag: &str) -> &str { tag.split_once('-').map_or(tag, |(primary, _)| primary) }

/// Report whether `tag` is a variant of the source language.
pub fn is_english(tag: &str) -> bool { language(tag) == "en" }

/// Return the message identifier of an `id` or `id.attribute` key.
fn message_id(key: &str) -> &str { key.split_once('.').map_or(key, |(id, _)| id) }

/// Return the plural categories the engine selects for `tag`.
fn plural_categories(tag: &str) -> &'static [&'static str] {
    PLURAL_CATEGORIES
        .iter()
        .find(|(candidate, _)| *candidate == language(tag))
        .map_or(&["one", "other"], |(_, categories)| *categories)
}

/// Report keys that are missing from, or unknown to, the source catalogue.
pub fn check_keys(tag: &str, source: &Catalogue, catalogue: &Catalogue) -> Vec<String> {
    let missing = source
        .keys()
        .filter(|key| !catalogue.contains_key(*key))
        .filter(|key| !KNOWN_MISSING_KEYS.contains(&(tag, key.as_str())))
        .map(|key| format!("{tag}: `{key}` is missing"));
    let orphaned = catalogue
        .keys()
        .filter(|key| !source.contains_key(*key))
        .map(|key| format!("{tag}: `{key}` does not exist in the source catalogue"));
    let stale = KNOWN_MISSING_KEYS
        .iter()
        .filter(|(locale, key)| *locale == tag && catalogue.contains_key(*key))
        .map(|(_, key)| format!("{tag}: `{key}` is now translated; drop its exception"));
    missing.chain(orphaned).chain(stale).collect()
}

/// Report variables a translation invents, or drops without an exception.
pub fn check_variables(tag: &str, source: &Catalogue, catalogue: &Catalogue) -> Vec<String> {
    let mut findings = Vec::new();
    for (key, translated) in catalogue {
        let Some(original) = source.get(key) else {
            continue;
        };
        let supplied = supplied_variables(key, original);
        for variable in translated.variables.difference(&supplied) {
            findings.push(format!(
                "{tag}: `{key}` uses `${variable}`, which no call site supplies"
            ));
        }
        for variable in original.variables.difference(&translated.variables) {
            if !may_drop(tag, key, variable, translated) {
                findings.push(format!("{tag}: `{key}` drops `${variable}`"));
            }
        }
        findings.extend(phrase_findings(tag, key, translated));
    }
    findings
}

/// Report pre-rendered phrases used by a locale the helper cannot serve.
fn phrase_findings(tag: &str, key: &str, translated: &PatternSummary) -> Vec<String> {
    if PHRASE_LANGUAGES.contains(&language(tag)) {
        return Vec::new();
    }
    PHRASE_SUBSTITUTES
        .iter()
        .filter(|(phrase, _)| translated.variables.contains(*phrase))
        .map(|(phrase, number)| {
            format!(
                "{tag}: `{key}` uses `${phrase}`, which is rendered in English; select on \
                 `${number}` instead"
            )
        })
        .collect()
}

/// Return every variable the call site for `key` supplies.
fn supplied_variables(key: &str, original: &PatternSummary) -> BTreeSet<String> {
    let extras = CALL_SITE_VARIABLES
        .iter()
        .filter(|(id, _)| *id == message_id(key))
        .flat_map(|(_, variables)| variables.iter().map(|variable| (*variable).to_owned()));
    original.variables.iter().cloned().chain(extras).collect()
}

/// Report whether a translation may leave `variable` out of `key`.
fn may_drop(tag: &str, key: &str, variable: &str, translated: &PatternSummary) -> bool {
    let is_substituted = PHRASE_SUBSTITUTES
        .iter()
        .any(|(phrase, number)| *phrase == variable && translated.variables.contains(*number));
    is_substituted || KNOWN_DROPPED_VARIABLES.contains(&(tag, key, variable))
}

/// Report plural selects whose keys disagree with the engine's categories.
///
/// A select that spells out number literals with only an `other` default is
/// accepted, because it names every case it distinguishes explicitly.
pub fn check_plural_selects(tag: &str, catalogue: &Catalogue) -> Vec<String> {
    let categories = plural_categories(tag);
    let mut findings = Vec::new();
    for (key, pattern) in catalogue {
        for select in &pattern.selects {
            let Some(selector) = select.selector.as_deref() else {
                continue;
            };
            if !NUMERIC_SELECTORS.contains(&selector) {
                continue;
            }
            let named: Vec<&str> = select
                .keys
                .iter()
                .map(String::as_str)
                .filter(|variant| !variant.starts_with(|first: char| first.is_ascii_digit()))
                .collect();
            let unknown = named.iter().any(|variant| !categories.contains(variant));
            let incomplete =
                named != ["other"] && categories.iter().any(|needed| !named.contains(needed));
            if unknown || incomplete {
                findings.push(format!(
                    "{tag}: `{key}` selects on `${selector}` with {named:?}; the plural engine \
                     uses {categories:?}"
                ));
            }
        }
    }
    findings
}

/// Report frozen literals from the source that a translation altered.
pub fn check_literals(tag: &str, source: &Catalogue, catalogue: &Catalogue) -> Vec<String> {
    let mut findings = Vec::new();
    for (key, translated) in catalogue {
        let Some(original) = source.get(key) else {
            continue;
        };
        for literal in frozen_literals(&original.text) {
            if !translated.text.contains(literal) {
                findings.push(format!("{tag}: `{key}` lost the literal `{literal}`"));
            }
        }
    }
    findings
}

/// Return the code spans and bare identifiers in a source pattern's text.
fn frozen_literals(text: &str) -> Vec<&str> {
    let spans = text
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|span| !span.trim().is_empty());
    let bare = BARE_LITERALS
        .iter()
        .copied()
        .filter(|literal| text.contains(literal));
    spans.chain(bare).collect()
}

/// String arguments shared by every sample, as (variable, value) pairs.
const SAMPLE_STRINGS: [(&str, &str); 11] = [
    ("name", "demo"),
    ("module", "demo"),
    ("function", "demo"),
    ("test", "demo"),
    ("lint", "module_max_lines"),
    ("operation", "std::fs::read"),
    ("attribute", "#[inline]"),
    ("subject", "functions"),
    ("reason", "a fenced code block"),
    ("receiver", "`std::option::Option<i32>`"),
    ("context", "function `demo`"),
];

/// Build one argument set covering every variable any message uses.
pub fn sample_arguments(tag: &str, count: i64, handling: &str) -> Arguments<'static> {
    let phrase = branch_phrase(tag, usize::try_from(count).unwrap_or(usize::MAX));
    let strings = SAMPLE_STRINGS
        .into_iter()
        .map(|(name, value)| (name, FluentValue::from(value)));
    let numbers = sample_numbers(count)
        .into_iter()
        .map(|(name, value)| (name, FluentValue::from(value)));
    let derived = [
        ("handling", FluentValue::from(handling.to_owned())),
        ("threshold", FluentValue::from(2.5_f64)),
        ("branch_phrase", FluentValue::from(phrase.clone())),
        ("limit_phrase", FluentValue::from(phrase)),
    ];
    strings
        .chain(numbers)
        .chain(derived)
        .map(|(name, value)| (Cow::Borrowed(name), value))
        .collect()
}

/// Return the integer arguments, with `count` in every counted position.
const fn sample_numbers(count: i64) -> [(&'static str, i64); 6] {
    [
        ("count", count),
        ("lines", count),
        ("branches", count),
        ("limit", count),
        ("score", 12),
        ("index", 2),
    ]
}

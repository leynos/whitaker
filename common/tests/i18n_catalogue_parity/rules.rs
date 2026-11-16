//! Rule tables and checks for the catalogue parity gates.
//!
//! Each table records a deliberate decision that the translator's guide
//! (`docs/translators-guide.md`) explains. Exceptions are listed by locale and
//! key, so a new divergence from the source catalogue has to be named here
//! instead of passing unnoticed.

use std::collections::BTreeSet;

use super::{
    inputs::LocaleTag,
    summary::{Catalogue, PatternSummary, Select},
};

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

/// Return the message identifier of an `id` or `id.attribute` key.
fn message_id(key: &str) -> &str { key.split_once('.').map_or(key, |(id, _)| id) }

/// Return the plural categories the engine selects for `tag`.
fn plural_categories(tag: LocaleTag<'_>) -> &'static [&'static str] {
    PLURAL_CATEGORIES
        .iter()
        .find(|(candidate, _)| *candidate == tag.language())
        .map_or(&["one", "other"], |(_, categories)| *categories)
}

/// A translated catalogue paired with the source catalogue it must match.
#[derive(Clone, Copy)]
pub struct Comparison<'a> {
    /// The locale being checked.
    pub tag: LocaleTag<'a>,
    /// The `en-GB` source catalogue.
    pub source: &'a Catalogue,
    /// The locale's catalogue.
    pub catalogue: &'a Catalogue,
}

/// One translated pattern, with the locale and key it belongs to.
#[derive(Clone, Copy)]
struct Translated<'a> {
    tag: LocaleTag<'a>,
    key: &'a str,
    pattern: &'a PatternSummary,
}

impl<'a> Comparison<'a> {
    /// Pair each translated pattern with its source pattern, skipping keys the
    /// source lacks; `check_keys` reports those.
    fn matched(self) -> impl Iterator<Item = (Translated<'a>, &'a PatternSummary)> {
        self.catalogue.iter().filter_map(move |(key, pattern)| {
            let translated = Translated {
                tag: self.tag,
                key,
                pattern,
            };
            self.source.get(key).map(|original| (translated, original))
        })
    }

    /// Report whether `key` is a listed exception for this locale.
    fn is_known_missing(self, key: &str) -> bool {
        KNOWN_MISSING_KEYS.contains(&(self.tag.as_str(), key))
    }
}

/// Report keys that are missing from, or unknown to, the source catalogue.
pub fn check_keys(comparison: Comparison<'_>) -> Vec<String> {
    let Comparison {
        tag,
        source,
        catalogue,
    } = comparison;
    let missing = source
        .keys()
        .filter(|key| !catalogue.contains_key(*key) && !comparison.is_known_missing(key))
        .map(|key| format!("{tag}: `{key}` is missing"));
    let orphaned = catalogue
        .keys()
        .filter(|key| !source.contains_key(*key))
        .map(|key| format!("{tag}: `{key}` does not exist in the source catalogue"));
    let stale = KNOWN_MISSING_KEYS
        .iter()
        .filter(|(locale, key)| *locale == tag.as_str() && catalogue.contains_key(*key))
        .map(|(_, key)| format!("{tag}: `{key}` is now translated; drop its exception"));
    missing.chain(orphaned).chain(stale).collect()
}

/// Report variables a translation invents, or drops without an exception.
pub fn check_variables(comparison: Comparison<'_>) -> Vec<String> {
    comparison
        .matched()
        .flat_map(|(translated, original)| {
            let mut findings = invented_variables(translated, original);
            findings.extend(dropped_variables(translated, original));
            findings.extend(phrase_findings(translated));
            findings
        })
        .collect()
}

/// Report variables that no call site supplies for this pattern.
fn invented_variables(translated: Translated<'_>, original: &PatternSummary) -> Vec<String> {
    let Translated { tag, key, pattern } = translated;
    let supplied = supplied_variables(key, original);
    pattern
        .variables
        .difference(&supplied)
        .map(|variable| format!("{tag}: `{key}` uses `${variable}`, which no call site supplies"))
        .collect()
}

/// Report source variables a translation leaves out without an exception.
fn dropped_variables(translated: Translated<'_>, original: &PatternSummary) -> Vec<String> {
    let Translated { tag, key, pattern } = translated;
    original
        .variables
        .difference(&pattern.variables)
        .filter(|variable| !may_drop(translated, variable))
        .map(|variable| format!("{tag}: `{key}` drops `${variable}`"))
        .collect()
}

/// Report pre-rendered phrases used by a locale the helper cannot serve.
fn phrase_findings(translated: Translated<'_>) -> Vec<String> {
    let Translated { tag, key, pattern } = translated;
    if PHRASE_LANGUAGES.contains(&tag.language()) {
        return Vec::new();
    }
    PHRASE_SUBSTITUTES
        .iter()
        .filter(|(phrase, _)| pattern.variables.contains(*phrase))
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

/// Report whether a translation may leave `variable` out of its pattern.
fn may_drop(translated: Translated<'_>, variable: &str) -> bool {
    let is_substituted = PHRASE_SUBSTITUTES.iter().any(|(phrase, number)| {
        *phrase == variable && translated.pattern.variables.contains(*number)
    });
    let exception = (translated.tag.as_str(), translated.key, variable);
    is_substituted || KNOWN_DROPPED_VARIABLES.contains(&exception)
}

/// Report plural selects whose keys disagree with the engine's categories.
///
/// A select that spells out number literals with only an `other` default is
/// accepted, because it names every case it distinguishes explicitly.
pub fn check_plural_selects(comparison: Comparison<'_>) -> Vec<String> {
    let Comparison { tag, catalogue, .. } = comparison;
    let categories = plural_categories(tag);
    catalogue
        .iter()
        .flat_map(|(key, pattern)| {
            pattern
                .selects
                .iter()
                .filter_map(move |select| plural_mismatch(select, categories))
                .map(move |(selector, named)| {
                    format!(
                        "{tag}: `{key}` selects on `${selector}` with {named:?}; the plural \
                         engine uses {categories:?}"
                    )
                })
        })
        .collect()
}

/// Return the selector and named variants of a numeric select whose
/// categories differ from `categories`, or `None` when it agrees.
fn plural_mismatch<'a>(select: &'a Select, categories: &[&str]) -> Option<(&'a str, Vec<&'a str>)> {
    let selector = select.selector.as_deref()?;
    if !NUMERIC_SELECTORS.contains(&selector) {
        return None;
    }
    let named: Vec<&str> = select
        .keys
        .iter()
        .map(String::as_str)
        .filter(|variant| !variant.starts_with(|first: char| first.is_ascii_digit()))
        .collect();
    let is_unknown = named.iter().any(|variant| !categories.contains(variant));
    let is_incomplete =
        named != ["other"] && categories.iter().any(|needed| !named.contains(needed));
    (is_unknown || is_incomplete).then_some((selector, named))
}

/// Report frozen literals from the source that a translation altered.
pub fn check_literals(comparison: Comparison<'_>) -> Vec<String> {
    comparison
        .matched()
        .flat_map(|(translated, original)| {
            let Translated { tag, key, pattern } = translated;
            frozen_literals(&original.text)
                .into_iter()
                .filter(|literal| !pattern.text.contains(literal))
                .map(move |literal| format!("{tag}: `{key}` lost the literal `{literal}`"))
                .collect::<Vec<_>>()
        })
        .collect()
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

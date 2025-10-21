//! Quality gates for localisation resources.
//!
//! These tests keep placeholder usage aligned across locales, ensure Welsh help
//! text remains complete, and exercise language-specific plural forms so we can
//! catch regressions before they reach users.

use common::i18n::{Arguments, Localiser};
use fluent_bundle::FluentValue;
use regex::Regex;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct FtlEntry {
    value: String,
    attributes: HashMap<String, String>,
}

fn locales_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../locales")
}

fn parse_ftl(path: &Path) -> BTreeMap<String, FtlEntry> {
    let mut entries: BTreeMap<String, FtlEntry> = BTreeMap::new();
    let content = fs::read_to_string(path).expect("ftl file should be readable");
    let message_re = Regex::new(r"^([A-Za-z0-9_-]+)\s*=\s*(.*)$").expect("valid message regex");
    let attribute_re =
        Regex::new(r"^\s+\.([A-Za-z0-9_-]+)\s*=\s*(.*)$").expect("valid attribute regex");

    let mut current_id: Option<String> = None;
    let mut current_attribute: Option<String> = None;

    for line in content.lines() {
        if process_message_line(
            line,
            &message_re,
            &mut entries,
            &mut current_id,
            &mut current_attribute,
        ) {
            continue;
        }

        if process_attribute_line(
            line,
            &attribute_re,
            &mut entries,
            &current_id,
            &mut current_attribute,
        ) {
            continue;
        }

        if is_ignorable_line(line) {
            continue;
        }

        process_continuation_line(
            line,
            current_id.as_deref(),
            current_attribute.as_deref(),
            &mut entries,
        );
    }

    entries
}

fn process_message_line(
    line: &str,
    message_re: &Regex,
    entries: &mut BTreeMap<String, FtlEntry>,
    current_id: &mut Option<String>,
    current_attribute: &mut Option<String>,
) -> bool {
    if let Some(captures) = message_re.captures(line) {
        let id = captures[1].to_string();
        let value = captures[2].to_string();
        let entry = entries.entry(id.clone()).or_default();
        entry.value = value;
        *current_id = Some(id);
        *current_attribute = None;
        return true;
    }

    false
}

fn process_attribute_line(
    line: &str,
    attribute_re: &Regex,
    entries: &mut BTreeMap<String, FtlEntry>,
    current_id: &Option<String>,
    current_attribute: &mut Option<String>,
) -> bool {
    if let Some(captures) = attribute_re.captures(line) {
        if let Some(current) = current_id.as_ref() {
            let name = captures[1].to_string();
            let value = captures[2].to_string();
            entries
                .entry(current.clone())
                .or_default()
                .attributes
                .insert(name.clone(), value);
            *current_attribute = Some(name);
        }
        return true;
    }

    false
}

fn is_ignorable_line(line: &str) -> bool {
    line.trim().is_empty() || line.trim_start().starts_with("##")
}

fn process_continuation_line(
    line: &str,
    current_id: Option<&str>,
    current_attribute: Option<&str>,
    entries: &mut BTreeMap<String, FtlEntry>,
) {
    if let Some(id) = current_id {
        if let Some(attribute) = current_attribute {
            append_to_attribute(entries, id, attribute, line);
        } else {
            append_to_message(entries, id, line);
        }
    }
}

fn append_to_attribute(
    entries: &mut BTreeMap<String, FtlEntry>,
    message_id: &str,
    attribute: &str,
    line: &str,
) {
    if let Some(entry) = entries.get_mut(message_id) {
        if let Some(text) = entry.attributes.get_mut(attribute) {
            text.push('\n');
            text.push_str(line.trim());
        }
    }
}

fn append_to_message(entries: &mut BTreeMap<String, FtlEntry>, message_id: &str, line: &str) {
    if let Some(entry) = entries.get_mut(message_id) {
        entry.value.push('\n');
        entry.value.push_str(line.trim());
    }
}

fn extract_placeables(text: &str) -> BTreeSet<String> {
    let re = Regex::new(r"\{\s*\$([A-Za-z0-9_]+)").expect("valid placeable regex");
    re.captures_iter(text)
        .map(|captures| captures[1].to_string())
        .collect()
}

fn secondary_locales() -> Vec<(String, PathBuf)> {
    let root = locales_root();
    let mut locales: Vec<(String, PathBuf)> = fs::read_dir(&root)
        .expect("locales directory should exist")
        .filter_map(|entry| {
            let entry = entry.expect("valid directory entry");
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_dir())
                .and_then(|_| entry.file_name().into_string().ok())
                .and_then(|name| {
                    if name == "en-GB" {
                        None
                    } else {
                        Some((name, entry.path()))
                    }
                })
        })
        .collect();

    locales.sort_by(|left, right| left.0.cmp(&right.0));
    locales
}

fn file_pairs() -> Vec<(String, PathBuf, PathBuf)> {
    let en = locales_root().join("en-GB");
    let locales = secondary_locales();
    let mut pairs: Vec<(String, PathBuf, PathBuf)> = Vec::new();

    for entry in fs::read_dir(&en).expect("en-GB locale should exist") {
        let entry = entry.expect("valid directory entry");
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("ftl") {
            continue;
        }

        let en_path = entry.path();
        let file_name: PathBuf = entry.file_name().into();

        for (locale, directory) in &locales {
            pairs.push((locale.clone(), en_path.clone(), directory.join(&file_name)));
        }
    }

    pairs
}

fn validate_message_placeables(
    locale: &str,
    message_id: &str,
    en_entry: &FtlEntry,
    locale_entry: &FtlEntry,
    locale_path: &Path,
) {
    let en_placeables = extract_placeables(&en_entry.value);
    let locale_placeables = extract_placeables(&locale_entry.value);
    assert_eq!(
        en_placeables,
        locale_placeables,
        "placeables diverged for `{message_id}` in {locale} ({})",
        locale_path.display()
    );
}

fn validate_attribute_placeables(
    locale: &str,
    message_id: &str,
    en_entry: &FtlEntry,
    locale_entry: &FtlEntry,
    locale_path: &Path,
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
            assert_eq!(
                en_placeables,
                locale_placeables,
                "placeables diverged for `{message_id}.{attribute}` in {locale} ({})",
                locale_path.display()
            );
        }
    }
}

fn validate_entry_placeables(
    locale: &str,
    message_id: &str,
    en_entry: &FtlEntry,
    locale_entries: &BTreeMap<String, FtlEntry>,
    locale_path: &Path,
) {
    let locale_entry = locale_entries.get(message_id).unwrap_or_else(|| {
        panic!(
            "{locale} locale missing message `{message_id}` in {}",
            locale_path.display()
        )
    });

    validate_message_placeables(locale, message_id, en_entry, locale_entry, locale_path);
    validate_attribute_placeables(locale, message_id, en_entry, locale_entry, locale_path);
}

#[test]
fn fluent_placeables_remain_in_sync() {
    for (locale, en_path, locale_path) in file_pairs() {
        let en_entries = parse_ftl(&en_path);
        let locale_entries = parse_ftl(&locale_path);

        for (message_id, en_entry) in &en_entries {
            validate_entry_placeables(&locale, message_id, en_entry, &locale_entries, &locale_path);
        }
    }
}

#[test]
fn localised_help_attributes_are_complete() {
    for (locale, en_path, locale_path) in file_pairs() {
        let en_entries = parse_ftl(&en_path);
        let locale_entries = parse_ftl(&locale_path);

        for (message_id, en_entry) in &en_entries {
            if en_entry.attributes.contains_key("help") {
                let locale_entry = locale_entries.get(message_id).unwrap_or_else(|| {
                    panic!(
                        "{locale} locale missing message `{message_id}` in {}",
                        locale_path.display()
                    )
                });
                let help = locale_entry.attributes.get("help").unwrap_or_else(|| {
                    panic!(
                        "{locale} locale missing `.help` for `{message_id}` in {}",
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

#[test]
fn welsh_pluralisation_covers_sample_range() {
    let mut localiser = Localiser::new(Some("cy"));
    let mut args = Arguments::new();

    for branches in 0..=12 {
        args.insert(
            Cow::Borrowed("branches"),
            FluentValue::from(branches as i64),
        );
        let note = localiser
            .attribute_with_args("conditional_max_two_branches", "note", &args)
            .expect("conditional note should resolve");
        assert!(
            !note.contains('{'),
            "formatted note should not expose raw placeables: `{note}`"
        );
    }
}

#[test]
fn gaelic_pluralisation_covers_sample_range() {
    let mut localiser = Localiser::new(Some("gd"));
    let mut args = Arguments::new();

    for branches in 0..=25 {
        args.insert(
            Cow::Borrowed("branches"),
            FluentValue::from(branches as i64),
        );
        let note = localiser
            .attribute_with_args("conditional_max_two_branches", "note", &args)
            .expect("conditional note should resolve");
        assert!(
            !note.contains('{'),
            "formatted note should not expose raw placeables: `{note}`"
        );
    }
}

#[test]
fn secondary_locales_fall_back_to_english_for_missing_attribute() {
    for locale in ["cy", "gd"] {
        let localiser = Localiser::new(Some(locale));
        let note = localiser
            .attribute("common-lint-count", "fallback-note")
            .expect("fallback attribute should resolve");
        assert_eq!(note, "Fallback diagnostics default to English.");
    }
}

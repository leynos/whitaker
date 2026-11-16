//! Parity gates for the Fluent catalogues embedded in `whitaker-common`.
//!
//! `en-GB` is the source catalogue. Every other locale directory must offer
//! the same files, messages, attributes, and terms, honour the variables each
//! call site supplies, list the plural categories the installed plural engine
//! selects, and keep frozen literals intact. The gates read the resources from
//! disk and render them through the embedded loader, so a locale that parses
//! but cannot be selected at runtime still fails.
//!
//! The expected tag list is an independent oracle: it is written out here
//! rather than read back from the loader, so adding or dropping a locale
//! directory by accident fails the suite. The translator's guide
//! (`docs/translators-guide.md`) documents each rule these gates enforce.

mod rules;
mod summary;

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;
use rules::{
    EXPECTED_TAGS,
    SOURCE_TAG,
    check_keys,
    check_literals,
    check_plural_selects,
    check_variables,
    is_english,
    sample_arguments,
};
use summary::{Catalogue, summarize};
use whitaker_common::i18n::{
    Localizer,
    available_locales,
    locales_root,
    normalize_fluent_output,
    supports_locale,
};

/// Bidirectional controls that catalogues must not contain; see the
/// translator's guide, section 8.
const BIDI_CONTROLS: [char; 12] = [
    '\u{200e}', '\u{200f}', '\u{061c}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

/// Integer samples covering every plural category the engine distinguishes.
const SAMPLE_COUNTS: [i64; 9] = [0, 1, 2, 3, 5, 11, 21, 100, 1_000_000];

/// Map each resource file name to its raw text for one locale.
type LocaleFiles = BTreeMap<String, String>;

/// Open the locale tree that the static loader embeds.
fn open_locales() -> Result<Dir, String> {
    let root = Utf8PathBuf::from_path_buf(locales_root())
        .map_err(|path| format!("locale root is not UTF-8: {}", path.display()))?;
    Dir::open_ambient_dir(&root, ambient_authority())
        .map_err(|error| format!("cannot open {root}: {error}"))
}

/// Read every file in the directory for `tag`.
fn read_locale(root: &Dir, tag: &str) -> Result<LocaleFiles, String> {
    let directory = root
        .open_dir(tag)
        .map_err(|error| format!("cannot open locale `{tag}`: {error}"))?;
    let mut files = LocaleFiles::new();
    let entries = directory
        .entries()
        .map_err(|error| format!("cannot list locale `{tag}`: {error}"))?;
    for entry in entries {
        let name = entry
            .and_then(|found| found.file_name())
            .map_err(|error| format!("cannot read an entry of `{tag}`: {error}"))?;
        let text = directory
            .read_to_string(&name)
            .map_err(|error| format!("cannot read `{tag}/{name}`: {error}"))?;
        files.insert(name, text);
    }
    Ok(files)
}

/// List the locale directories present on disk, sorted by tag.
fn locale_tags(root: &Dir) -> Result<Vec<String>, String> {
    let entries = root
        .entries()
        .map_err(|error| format!("cannot list the locale root: {error}"))?;
    let mut tags = Vec::new();
    for entry in entries {
        let name = entry
            .and_then(|found| found.file_name())
            .map_err(|error| format!("cannot read a locale root entry: {error}"))?;
        tags.push(name);
    }
    tags.sort_unstable();
    Ok(tags)
}

/// Parse every file of a locale into one catalogue, reporting syntax errors.
fn parse_locale(tag: &str, files: &LocaleFiles, findings: &mut Vec<String>) -> Catalogue {
    let mut catalogue = Catalogue::new();
    for (name, text) in files {
        match summarize(text) {
            Ok(parsed) => catalogue.extend(parsed),
            Err(errors) => findings.push(format!("{tag}/{name}: syntax errors: {errors}")),
        }
    }
    catalogue
}

/// Collect the findings of every structural gate, across all locales.
fn structural_findings() -> Result<Vec<String>, String> {
    let root = open_locales()?;
    let mut findings = Vec::new();
    let source_files = read_locale(&root, SOURCE_TAG)?;
    let source = parse_locale(SOURCE_TAG, &source_files, &mut findings);
    let source_names: BTreeSet<&String> = source_files.keys().collect();

    for tag in locale_tags(&root)? {
        let files = read_locale(&root, &tag)?;
        let names: BTreeSet<&String> = files.keys().collect();
        if names != source_names {
            findings.push(format!(
                "{tag}: file set differs from {SOURCE_TAG}: {names:?}"
            ));
        }
        for (name, text) in &files {
            if text.contains(BIDI_CONTROLS) {
                findings.push(format!("{tag}/{name}: contains a bidirectional control"));
            }
        }
        if tag == SOURCE_TAG {
            continue;
        }
        let catalogue = parse_locale(&tag, &files, &mut findings);
        findings.extend(check_keys(&tag, &source, &catalogue));
        findings.extend(check_variables(&tag, &source, &catalogue));
        findings.extend(check_plural_selects(&tag, &catalogue));
        findings.extend(check_literals(&tag, &source, &catalogue));
    }
    Ok(findings)
}

/// Render `key` (`id` or `id.attribute`) for `tag` with sample arguments.
fn render(tag: &str, key: &str, count: i64, handling: &str) -> Result<String, String> {
    let localizer = Localizer::new(Some(tag));
    if localizer.used_fallback() {
        return Err(format!("{tag}: the loader fell back to the source locale"));
    }
    let arguments = sample_arguments(tag, count, handling);
    let rendered = match key.split_once('.') {
        Some((id, attribute)) => localizer.attribute_with_args(id, attribute, &arguments),
        None => localizer.message_with_args(key, &arguments),
    };
    rendered
        .map(normalize_fluent_output)
        .map_err(|error| format!("{tag}: {error}"))
}

/// Collect rendering failures for every message of every locale.
fn rendering_findings() -> Result<Vec<String>, String> {
    let root = open_locales()?;
    let mut findings = Vec::new();
    let source_files = read_locale(&root, SOURCE_TAG)?;
    let source = parse_locale(SOURCE_TAG, &source_files, &mut findings);
    let keys: Vec<&String> = source.keys().filter(|key| !key.starts_with('-')).collect();

    for tag in locale_tags(&root)? {
        for key in &keys {
            findings.extend(
                sample_inputs()
                    .filter_map(|(count, handling)| rendering_finding(&tag, key, count, handling)),
            );
        }
    }
    Ok(findings)
}

/// Pair every sample count with every `$handling` selector value.
fn sample_inputs() -> impl Iterator<Item = (i64, &'static str)> {
    SAMPLE_COUNTS.into_iter().flat_map(|count| {
        ["option", "result", "other"]
            .into_iter()
            .map(move |handling| (count, handling))
    })
}

/// Describe a pattern that fails to render, or renders unresolved or empty.
fn rendering_finding(tag: &str, key: &str, count: i64, handling: &str) -> Option<String> {
    match render(tag, key, count, handling) {
        Ok(text) if text.contains(['{', '\u{fffd}']) || text.trim().is_empty() => {
            Some(format!("{tag}: `{key}` rendered as `{text}`"))
        }
        Ok(_) => None,
        Err(error) => Some(error),
    }
}

/// Collect translated patterns whose rendered text equals the source's.
fn untranslated_findings() -> Result<Vec<String>, String> {
    let root = open_locales()?;
    let mut findings = Vec::new();
    for tag in locale_tags(&root)? {
        if is_english(&tag) {
            continue;
        }
        let files = read_locale(&root, &tag)?;
        let catalogue = parse_locale(&tag, &files, &mut findings);
        for key in catalogue.keys().filter(|key| !key.starts_with('-')) {
            let translated = render(&tag, key, 3, "other")?;
            let original = render(SOURCE_TAG, key, 3, "other")?;
            if translated == original {
                findings.push(format!("{tag}: `{key}` is a copy of the source text"));
            }
        }
    }
    Ok(findings)
}

/// Fail with every finding listed, or with the error that stopped the scan.
fn assert_no_findings(outcome: Result<Vec<String>, String>, gate: &str) {
    let findings = outcome.unwrap_or_else(|error| vec![error]);
    assert!(
        findings.is_empty(),
        "{gate} found {} problem(s):\n{}",
        findings.len(),
        findings.join("\n")
    );
}

#[test]
fn registry_matches_the_expected_tag_list() {
    let expected: Vec<String> = EXPECTED_TAGS.iter().map(|tag| (*tag).to_owned()).collect();
    assert_eq!(
        available_locales(),
        expected.as_slice(),
        "the embedded locales must equal the reviewed tag list in `rules.rs`"
    );
}

#[test]
fn locale_directories_match_the_expected_tag_list() {
    let tags = open_locales().and_then(|root| locale_tags(&root));
    let expected: Vec<String> = EXPECTED_TAGS.iter().map(|tag| (*tag).to_owned()).collect();
    assert_eq!(
        tags,
        Ok(expected),
        "`common/locales` must hold one directory per reviewed tag"
    );
}

#[test]
fn catalogues_match_the_source_structure() {
    assert_no_findings(structural_findings(), "the structural gate");
}

#[test]
fn every_pattern_renders_through_the_loader() {
    assert_no_findings(rendering_findings(), "the rendering gate");
}

#[test]
fn translations_are_not_copies_of_the_source() {
    assert_no_findings(untranslated_findings(), "the translation gate");
}

/// Locale selection matches whole tags: a bare language or an unlisted region
/// is unsupported even when a sibling variant ships.
#[rstest]
#[case::exact("pt-BR", true)]
#[case::case_is_normalized("PT-br", true)]
#[case::script_case_is_normalized("zh-hans", true)]
#[case::bare_language_with_two_variants("pt", false)]
#[case::bare_spanish("es", false)]
#[case::bare_chinese("zh", false)]
#[case::bare_english("en", false)]
#[case::region_for_script("zh-CN", false)]
#[case::unlisted_region("fr-CA", false)]
#[case::norwegian_macrolanguage("no", false)]
fn locale_selection_matches_whole_tags(#[case] requested: &str, #[case] supported: bool) {
    assert_eq!(
        supports_locale(requested),
        supported,
        "`{requested}` support must match the documented negotiation policy"
    );
}

/// A key a locale has not translated resolves through the source catalogue.
#[test]
fn untranslated_keys_fall_back_to_the_source_text() {
    assert_eq!(
        render("gd", "common-lint-count.fallback-note", 1, "other"),
        render(SOURCE_TAG, "common-lint-count.fallback-note", 1, "other"),
        "an untranslated Gaelic attribute must render the en-GB text"
    );
}

/// Numbers are interpolated as plain ASCII digits whatever the locale, so a
/// translation owns the words around a number and never its formatting.
#[rstest]
#[case::english("en-GB")]
#[case::german("de")]
#[case::arabic("ar")]
#[case::hindi("hi")]
#[case::persian("fa")]
fn numbers_render_without_locale_formatting(#[case] tag: &str) {
    let rendered = render(tag, "module_max_lines", 1_000_000, "other");
    assert!(
        rendered.as_ref().is_ok_and(|text| text.contains("1000000")),
        "{tag}: expected ungrouped ASCII digits, found {rendered:?}"
    );
}

/// Numbers must reach the plural resolver as numbers: each listed count
/// selects a different category, so the rendered labels must all differ.
#[rstest]
#[case::english("en-GB", &[1, 2])]
#[case::american_english("en-US", &[1, 2])]
#[case::gaelic("gd", &[1, 2])]
#[case::polish("pl", &[1, 2, 5])]
#[case::russian("ru", &[1, 2, 5])]
#[case::ukrainian("uk", &[1, 2, 5])]
#[case::arabic("ar", &[1, 2, 3, 11])]
#[case::regional_spanish("es-419", &[1, 2])]
#[case::regional_portuguese("pt-BR", &[1, 2])]
fn plural_variants_follow_numeric_arguments(#[case] tag: &str, #[case] counts: &[i64]) {
    let labels: Result<BTreeSet<String>, String> = counts
        .iter()
        .map(|count| {
            render(tag, "bumpy_road_function.label", *count, "other")
                .map(|label| label.replace(|character: char| character.is_ascii_digit(), ""))
        })
        .collect();
    assert_eq!(
        labels.map(|distinct| distinct.len()),
        Ok(counts.len()),
        "{tag}: counts {counts:?} must each select their own plural wording"
    );
}

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

mod inputs;
mod rules;
mod summary;
mod tree;

#[cfg(test)]
mod rule_tests;

use std::collections::BTreeSet;

use cap_std::fs_utf8::Dir;
use inputs::{Handling, LocaleTag, Lookup, SampleInput, sample_arguments};
use rstest::rstest;
use rules::{
    Comparison,
    EXPECTED_TAGS,
    SOURCE_TAG,
    check_keys,
    check_literals,
    check_plural_selects,
    check_variables,
};
use summary::Catalogue;
use tree::{
    Source,
    file_findings,
    locale_tags,
    open_locales,
    parse_locale,
    read_locale,
    read_source,
};
use whitaker_common::i18n::{
    Localizer,
    available_locales,
    normalize_fluent_output,
    supports_locale,
};

/// Integer samples covering every plural category the engine distinguishes.
const SAMPLE_COUNTS: [i64; 9] = [0, 1, 2, 3, 5, 11, 21, 100, 1_000_000];

/// Collect the findings of every structural gate, across all locales.
fn structural_findings() -> Result<Vec<String>, String> {
    let root = open_locales()?;
    let mut findings = Vec::new();
    let source = read_source(&root, &mut findings)?;
    for tag in locale_tags(&root)? {
        findings.extend(locale_findings(&root, LocaleTag::new(&tag), &source)?);
    }
    Ok(findings)
}

/// Collect the structural findings for one locale.
fn locale_findings(root: &Dir, tag: LocaleTag<'_>, source: &Source) -> Result<Vec<String>, String> {
    let files = read_locale(root, tag)?;
    let mut findings = file_findings(tag, &files, source);
    if tag.as_str() == SOURCE_TAG {
        return Ok(findings);
    }
    let catalogue = parse_locale(tag, &files, &mut findings);
    let comparison = Comparison {
        tag,
        source: &source.catalogue,
        catalogue: &catalogue,
    };
    findings.extend(check_keys(comparison));
    findings.extend(check_variables(comparison));
    findings.extend(check_plural_selects(comparison));
    findings.extend(check_literals(comparison));
    Ok(findings)
}

/// Render a lookup's key (`id` or `id.attribute`) with sample arguments.
fn render(lookup: Lookup<'_>, input: SampleInput) -> Result<String, String> {
    let Lookup { tag, key } = lookup;
    let localizer = Localizer::new(Some(tag.as_str()));
    if localizer.used_fallback() {
        return Err(format!("{tag}: the loader fell back to the source locale"));
    }
    let arguments = sample_arguments(tag, input);
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
    let source = read_source(&root, &mut findings)?;
    let keys: Vec<&String> = source
        .catalogue
        .keys()
        .filter(|key| !key.starts_with('-'))
        .collect();
    for tag in locale_tags(&root)? {
        for key in &keys {
            let lookup = Lookup {
                tag: LocaleTag::new(&tag),
                key,
            };
            findings.extend(
                sample_inputs().filter_map(|input| rendering_finding(lookup, input, &render)),
            );
        }
    }
    Ok(findings)
}

/// Pair every sample count with every `$handling` selector value.
fn sample_inputs() -> impl Iterator<Item = SampleInput> {
    SAMPLE_COUNTS.into_iter().flat_map(|count| {
        Handling::ALL
            .into_iter()
            .map(move |handling| SampleInput { count, handling })
    })
}

/// Render a lookup with sample inputs; injected so tests can supply defects.
type Renderer<'r> = dyn Fn(Lookup<'_>, SampleInput) -> Result<String, String> + 'r;

/// Describe a pattern that fails to render, or renders unresolved or empty.
fn rendering_finding(
    lookup: Lookup<'_>,
    input: SampleInput,
    render: &Renderer<'_>,
) -> Option<String> {
    match render(lookup, input) {
        Ok(text) if text.contains(['{', '\u{fffd}']) || text.trim().is_empty() => {
            let Lookup { tag, key } = lookup;
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
    let tags = locale_tags(&root)?;
    for tag in tags.iter().map(|tag| LocaleTag::new(tag)) {
        if !tag.is_english() {
            let catalogue = parse_locale(tag, &read_locale(&root, tag)?, &mut findings);
            findings.extend(copied_patterns(tag, &catalogue, &render)?);
        }
    }
    Ok(findings)
}

/// Report the patterns of one locale that render exactly as the source does.
fn copied_patterns(
    tag: LocaleTag<'_>,
    catalogue: &Catalogue,
    render: &Renderer<'_>,
) -> Result<Vec<String>, String> {
    let mut findings = Vec::new();
    let input = SampleInput::counting(3);
    for key in catalogue.keys().filter(|key| !key.starts_with('-')) {
        let source = Lookup {
            tag: LocaleTag::new(SOURCE_TAG),
            key,
        };
        if render(Lookup { tag, key }, input)? == render(source, input)? {
            findings.push(format!("{tag}: `{key}` is a copy of the source text"));
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
    let key = "common-lint-count.fallback-note";
    let gaelic = Lookup {
        tag: LocaleTag::new("gd"),
        key,
    };
    let source = Lookup {
        tag: LocaleTag::new(SOURCE_TAG),
        key,
    };
    let input = SampleInput::counting(1);
    assert_eq!(
        render(gaelic, input),
        render(source, input),
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
    let lookup = Lookup {
        tag: LocaleTag::new(tag),
        key: "module_max_lines",
    };
    let rendered = render(lookup, SampleInput::counting(1_000_000));
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
    let lookup = Lookup {
        tag: LocaleTag::new(tag),
        key: "bumpy_road_function.label",
    };
    let labels: Result<BTreeSet<String>, String> = counts
        .iter()
        .map(|count| {
            render(lookup, SampleInput::counting(*count))
                .map(|label| label.replace(|character: char| character.is_ascii_digit(), ""))
        })
        .collect();
    assert_eq!(
        labels.map(|distinct| distinct.len()),
        Ok(counts.len()),
        "{tag}: counts {counts:?} must each select their own plural wording"
    );
}

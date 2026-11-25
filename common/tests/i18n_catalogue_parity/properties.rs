//! Property tests for plural selection and locale matching.
//!
//! The structural gate checks each plural select against the hand-copied
//! `PLURAL_CATEGORIES` table, and the example tests render a few chosen
//! counts. These tests check the whole chain against the plural engine Fluent
//! uses: for generated and boundary counts in every shipped locale, the engine's
//! category must be listed in the table, and a probe message with one marker
//! per listed category must select exactly that category when rendered through
//! the bundle type the loader uses. They also check that locale selection
//! ignores case and rejects every tag that does not ship.

use std::fmt::Write as _;

use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use fluent_templates::fluent_bundle::{
    FluentArgs,
    FluentResource,
    FluentValue,
    concurrent::FluentBundle,
};
use intl_pluralrules::{PluralCategory, PluralRuleType, PluralRules};
use proptest::prelude::*;
use unic_langid::LanguageIdentifier;
use whitaker_common::i18n::supports_locale;

use super::{
    inputs::{LocaleTag, Lookup, SampleInput},
    render,
    rules::{EXPECTED_TAGS, plural_categories},
};

/// Counts at the edges of the plural rules: zero, one, two, small and large
/// teens, and the hundreds boundaries Arabic and the Slavic rules key on.
const BOUNDARY_COUNTS: [u32; 22] = [
    0, 1, 2, 3, 4, 5, 6, 10, 11, 12, 14, 19, 20, 21, 22, 25, 99, 100, 101, 102, 111, 1_000_000,
];

/// Parse a shipped tag, as the loader does.
fn identifier(tag: LocaleTag<'_>) -> LanguageIdentifier {
    tag.as_str().parse().expect("shipped tags parse")
}

/// Return the category the plural engine assigns to `count` for `tag`.
///
/// `fluent-bundle` negotiates the rules' locale with the lookup strategy and
/// an `en` default, so a script or region tag such as `zh-Hant` uses its
/// language's rules. The oracle repeats that negotiation exactly.
fn engine_category(tag: LocaleTag<'_>, count: u32) -> &'static str {
    let default: LanguageIdentifier = "en".parse().expect("`en` parses");
    let available = PluralRules::get_locales(PluralRuleType::CARDINAL);
    let requested = [identifier(tag)];
    let negotiated = negotiate_languages(
        &requested,
        &available,
        Some(&default),
        NegotiationStrategy::Lookup,
    );
    let chosen = negotiated
        .first()
        .map(|language| (*language).clone())
        .expect("negotiation with a default yields a locale");
    let category = PluralRules::create(chosen, PluralRuleType::CARDINAL)
        .and_then(|rules| rules.select(count))
        .expect("integer counts have a plural category");
    category_key(&category)
}

/// Return the Fluent variant key for a plural category.
const fn category_key(category: &PluralCategory) -> &'static str {
    match category {
        PluralCategory::ZERO => "zero",
        PluralCategory::ONE => "one",
        PluralCategory::TWO => "two",
        PluralCategory::FEW => "few",
        PluralCategory::MANY => "many",
        PluralCategory::OTHER => "other",
    }
}

/// Render a probe select whose variant for each listed category prints that
/// category's name, so the output names the category Fluent selected.
fn selected_category(tag: LocaleTag<'_>, count: u32) -> String {
    let variants = plural_categories(tag)
        .iter()
        .fold(String::new(), |mut text, category| {
            let marker = if *category == "other" { "*" } else { " " };
            writeln!(text, "       {marker}[{category}] {category}")
                .expect("writing to a String cannot fail");
            text
        });
    let source = format!("probe = {{ $count ->\n{variants}    }}\n");
    let resource = FluentResource::try_new(source).expect("the probe parses");
    let mut bundle = FluentBundle::new_concurrent(vec![identifier(tag)]);
    bundle.set_use_isolating(false);
    bundle.add_resource(resource).expect("the probe loads");
    let pattern = bundle
        .get_message("probe")
        .and_then(|message| message.value())
        .expect("the probe has a value");
    let mut arguments = FluentArgs::new();
    arguments.set("count", FluentValue::from(i64::from(count)));
    let mut errors = Vec::new();
    let selected = bundle.format_pattern(pattern, Some(&arguments), &mut errors);
    assert!(
        errors.is_empty(),
        "{tag}: probe errors for {count}: {errors:?}"
    );
    selected.into_owned()
}

/// Check one count in one locale, returning a description of any mismatch.
fn plural_mismatch(tag: LocaleTag<'_>, count: u32) -> Option<String> {
    let expected = engine_category(tag, count);
    if !plural_categories(tag).contains(&expected) {
        return Some(format!(
            "{tag}: the engine puts {count} in `{expected}`, which the parity table omits"
        ));
    }
    let selected = selected_category(tag, count);
    if selected != expected {
        return Some(format!(
            "{tag}: {count} selected `{selected}`, but the engine assigns `{expected}`"
        ));
    }
    let lookup = Lookup {
        tag,
        key: "bumpy_road_function.label",
    };
    match render(lookup, SampleInput::counting(i64::from(count))) {
        Ok(text) if !text.contains(['{', '\u{fffd}']) => None,
        rendered => Some(format!("{tag}: count {count} rendered as {rendered:?}")),
    }
}

/// Check one count in every shipped locale.
fn mismatches_for(count: u32) -> Vec<String> {
    EXPECTED_TAGS
        .iter()
        .filter_map(|tag| plural_mismatch(LocaleTag::new(tag), count))
        .collect()
}

#[test]
fn boundary_counts_select_the_engine_category() {
    let mismatches: Vec<String> = BOUNDARY_COUNTS
        .into_iter()
        .flat_map(mismatches_for)
        .collect();
    assert!(
        mismatches.is_empty(),
        "plural selection disagrees with the engine:\n{}",
        mismatches.join("\n")
    );
}

proptest! {
    /// For a generated count, every shipped locale lists the engine's
    /// category, selects it when rendering, and renders the counted label.
    #[test]
    fn generated_counts_select_the_engine_category(count in 0_u32..=10_000) {
        let mismatches = mismatches_for(count);
        prop_assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    }

    /// Every shipped tag is accepted whatever its letter case.
    #[test]
    fn shipped_tags_match_in_any_case(flips in prop::collection::vec(any::<bool>(), 8)) {
        for tag in EXPECTED_TAGS {
            let recased: String = tag
                .chars()
                .zip(flips.iter().cycle())
                .map(|(character, &is_upper)| {
                    if is_upper { character.to_ascii_uppercase() } else { character.to_ascii_lowercase() }
                })
                .collect();
            prop_assert!(supports_locale(&recased), "`{recased}` should select `{tag}`");
        }
    }

    /// A well-formed tag that does not ship is rejected, so selection moves on
    /// to the next setting instead of guessing a related catalogue.
    #[test]
    fn unshipped_tags_are_rejected(requested in "[a-z]{2,3}(-[A-Z]{2})?") {
        let is_shipped = EXPECTED_TAGS.iter().any(|tag| tag.eq_ignore_ascii_case(&requested));
        prop_assume!(!is_shipped);
        prop_assert!(!supports_locale(&requested), "`{requested}` does not ship and must be rejected");
    }
}

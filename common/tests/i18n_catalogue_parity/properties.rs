//! Property tests for plural selection and locale matching.
//!
//! The structural gate checks each plural select against the hand-copied
//! `PLURAL_CATEGORIES` table, and the example tests render a few chosen
//! counts. These properties check the table itself against the plural engine
//! Fluent uses, over a generated range of counts in every shipped locale, so a
//! mistranscribed category cannot leave a select silently falling through to
//! `*[other]`. They also check that locale selection ignores case and rejects
//! every tag that does not ship.

use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use intl_pluralrules::{PluralCategory, PluralRuleType, PluralRules};
use proptest::prelude::*;
use unic_langid::LanguageIdentifier;
use whitaker_common::i18n::supports_locale;

use super::{
    inputs::{LocaleTag, Lookup, SampleInput},
    render,
    rules::{EXPECTED_TAGS, plural_categories},
};

/// Resolve the plural rules Fluent applies for `tag`.
///
/// `fluent-bundle` negotiates the rules' locale with the lookup strategy and
/// an `en` default, so a script or region tag such as `zh-Hant` uses its
/// language's rules. The oracle repeats that negotiation exactly.
fn engine_rules(tag: LocaleTag<'_>) -> PluralRules {
    let requested: LanguageIdentifier = tag.as_str().parse().expect("shipped tags parse");
    let default: LanguageIdentifier = "en".parse().expect("`en` parses");
    let available = PluralRules::get_locales(PluralRuleType::CARDINAL);
    let negotiated = negotiate_languages(
        &[requested],
        &available,
        Some(&default),
        NegotiationStrategy::Lookup,
    );
    let chosen = negotiated
        .first()
        .map(|identifier| (*identifier).clone())
        .expect("negotiation with a default yields a locale");
    PluralRules::create(chosen, PluralRuleType::CARDINAL).expect("negotiated locales have rules")
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

/// Choose one shipped locale by index.
fn shipped_tag() -> impl Strategy<Value = LocaleTag<'static>> {
    prop::sample::select(EXPECTED_TAGS.to_vec()).prop_map(LocaleTag::new)
}

proptest! {
    /// Every category the engine assigns to a count is one the parity table
    /// lists for that locale, so the structural gate demands a variant for it;
    /// and the counted label renders completely.
    #[test]
    fn engine_categories_are_listed_and_render(tag in shipped_tag(), count in 0_u32..=10_000) {
        let category = engine_rules(tag)
            .select(count)
            .map(|category| category_key(&category))
            .expect("integer counts have a plural category");
        prop_assert!(
            plural_categories(tag).contains(&category),
            "{tag}: the engine puts {count} in `{category}`, which the parity table omits"
        );
        let lookup = Lookup { tag, key: "bumpy_road_function.label" };
        let rendered = render(lookup, SampleInput::counting(i64::from(count)));
        prop_assert!(
            rendered.as_ref().is_ok_and(|text| !text.contains(['{', '\u{fffd}'])),
            "{tag}: count {count} rendered as {rendered:?}"
        );
    }

    /// A shipped tag is accepted whatever its letter case.
    #[test]
    fn shipped_tags_match_in_any_case(tag in shipped_tag(), flips in prop::collection::vec(any::<bool>(), 8)) {
        let recased: String = tag
            .as_str()
            .chars()
            .zip(flips.iter().cycle())
            .map(|(character, &is_flipped)| {
                if is_flipped { character.to_ascii_uppercase() } else { character.to_ascii_lowercase() }
            })
            .collect();
        prop_assert!(supports_locale(&recased), "`{recased}` should select `{tag}`");
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

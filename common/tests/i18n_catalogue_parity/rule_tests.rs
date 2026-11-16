//! Fixture tests proving that each parity rule rejects a defective catalogue.
//!
//! The gate tests in `main.rs` pass on the shipped catalogues, so on their own
//! they cannot show that a rule still fires. These tests feed each rule small
//! in-memory Fluent sources, one with a deliberate defect and one without, and
//! assert the finding the defect must produce. The loader-backed checks take
//! an injected renderer, so a fake can supply the defective rendering.

use rstest::rstest;

use super::{
    copied_patterns,
    inputs::{LocaleTag, Lookup, SampleInput},
    rendering_finding,
    rules::{Comparison, check_keys, check_literals, check_plural_selects, check_variables},
    summary::{Catalogue, summarize},
    tree::{LocaleFiles, Source, file_findings},
};

/// A Fluent fixture source written inline in a test.
#[derive(Clone, Copy, Debug)]
struct Ftl(&'static str);

impl Ftl {
    /// Parse a fixture that the test author wrote to be valid.
    fn parse(self) -> Catalogue { summarize(self.0).expect("fixture Fluent source should parse") }
}

/// How many findings must mention a given phrase.
#[derive(Clone, Copy, Debug)]
struct Expected {
    needle: &'static str,
    count: usize,
}

impl Expected {
    /// Expect no finding mentioning `needle`.
    const fn none(needle: &'static str) -> Self { Self { needle, count: 0 } }

    /// Expect exactly one finding mentioning `needle`.
    const fn one(needle: &'static str) -> Self { Self { needle, count: 1 } }

    /// Assert that exactly `count` findings mention `needle`.
    fn assert(self, findings: &[String]) {
        let matching = findings
            .iter()
            .filter(|finding| finding.contains(self.needle))
            .count();
        assert_eq!(
            matching, self.count,
            "expected {} finding(s) mentioning `{}`, got {findings:?}",
            self.count, self.needle
        );
    }
}

/// A translated fixture and the finding one rule must report for it.
#[derive(Clone, Copy, Debug)]
struct RuleCase {
    translated: Ftl,
    expected: Expected,
}

/// Source fixture with a variable, a code span, a bare literal, and a select.
const SOURCE: Ftl = Ftl(concat!(
    "greeting = Call `expect` on { $name } through std::fs.\n",
    "    .note = Use `concat!()` instead.\n",
    "lines = { $count ->\n",
    "        [one] One line.\n",
    "       *[other] { $count } lines.\n",
    "    }\n",
));

/// The `lines` select of a faithful translation.
macro_rules! faithful_lines {
    () => {
        "lines = { $count ->\n        [one] A single line.\n       *[other] { $count } source \
         lines.\n    }\n"
    };
}

/// A faithful translation of `SOURCE`.
const GOOD: Ftl = Ftl(concat!(
    "greeting = Invoke `expect` for { $name } via std::fs.\n",
    "    .note = Prefer `concat!()`.\n",
    faithful_lines!(),
));

/// `GOOD` without the `lines` message.
const WITHOUT_LINES: Ftl = Ftl(concat!(
    "greeting = Invoke `expect` for { $name } via std::fs.\n",
    "    .note = Prefer `concat!()`.\n",
));

/// `GOOD` with a message the source does not have.
const WITH_EXTRA: Ftl = Ftl(concat!(
    "greeting = Invoke `expect` for { $name } via std::fs.\n",
    "    .note = Prefer `concat!()`.\n",
    faithful_lines!(),
    "extra = Unknown.\n",
));

/// `GOOD` with `$name` replaced by `$nickname`, an invented variable.
const WITH_TYPO: Ftl = Ftl(concat!(
    "greeting = Invoke `expect` for { $nickname } via std::fs.\n",
    "    .note = Prefer `concat!()`.\n",
    faithful_lines!(),
));

/// `GOOD` without `$name`, a dropped variable.
const WITHOUT_NAME: Ftl = Ftl(concat!(
    "greeting = Invoke `expect` via std::fs.\n",
    "    .note = Prefer `concat!()`.\n",
    faithful_lines!(),
));

/// `GOOD` with `concat!()` rewritten, an altered code span.
const WITHOUT_SPAN: Ftl = Ftl(concat!(
    "greeting = Invoke `expect` for { $name } via std::fs.\n",
    "    .note = Prefer `concat()`.\n",
    faithful_lines!(),
));

/// `GOOD` with `std::fs` translated away, an altered bare literal.
const WITHOUT_BARE: Ftl = Ftl(concat!(
    "greeting = Invoke `expect` for { $name } via the filesystem.\n",
    "    .note = Prefer `concat!()`.\n",
    faithful_lines!(),
));

/// `GOOD` with the `[one]` variant deleted.
const WITH_BARE_OTHER: Ftl = Ftl(concat!(
    "greeting = Invoke `expect` for { $name } via std::fs.\n",
    "    .note = Prefer `concat!()`.\n",
    "lines = { $count ->\n       *[other] { $count } source lines.\n    }\n",
));

/// Run `check` for a two-category locale against `SOURCE`.
fn translated_findings(translated: Ftl, check: fn(Comparison<'_>) -> Vec<String>) -> Vec<String> {
    let source = SOURCE.parse();
    let translation = translated.parse();
    check(Comparison {
        tag: LocaleTag::new("de"),
        source: &source,
        catalogue: &translation,
    })
}

#[rstest]
#[case::faithful(RuleCase { translated: GOOD, expected: Expected::none("de") })]
#[case::missing_key(RuleCase {
    translated: WITHOUT_LINES,
    expected: Expected::one("`lines` is missing"),
})]
#[case::orphaned_key(RuleCase {
    translated: WITH_EXTRA,
    expected: Expected::one("`extra` does not exist in the source catalogue"),
})]
fn check_keys_reports_missing_and_orphaned_keys(#[case] case: RuleCase) {
    case.expected
        .assert(&translated_findings(case.translated, check_keys));
}

#[rstest]
#[case::faithful(RuleCase { translated: GOOD, expected: Expected::none("$") })]
#[case::invented(RuleCase {
    translated: WITH_TYPO,
    expected: Expected::one("uses `$nickname`, which no call site supplies"),
})]
#[case::dropped(RuleCase { translated: WITHOUT_NAME, expected: Expected::one("`greeting` drops `$name`") })]
fn check_variables_reports_invented_and_dropped_variables(#[case] case: RuleCase) {
    case.expected
        .assert(&translated_findings(case.translated, check_variables));
}

#[rstest]
#[case::faithful(RuleCase { translated: GOOD, expected: Expected::none("lost the literal") })]
#[case::code_span(RuleCase { translated: WITHOUT_SPAN, expected: Expected::one("lost the literal `concat!()`") })]
#[case::bare_literal(RuleCase { translated: WITHOUT_BARE, expected: Expected::one("lost the literal `std::fs`") })]
fn check_literals_reports_altered_frozen_tokens(#[case] case: RuleCase) {
    case.expected
        .assert(&translated_findings(case.translated, check_literals));
}

#[rstest]
#[case::complete(RuleCase { translated: GOOD, expected: Expected::none("selects on") })]
#[case::bare_other(RuleCase {
    translated: WITH_BARE_OTHER,
    expected: Expected::one("`lines` selects on `$count` with [\"other\"]"),
})]
fn check_plural_selects_wires_the_category_comparison(#[case] case: RuleCase) {
    case.expected
        .assert(&translated_findings(case.translated, check_plural_selects));
}

/// One locale's catalogue checked against its own source fixture.
#[derive(Clone, Copy, Debug)]
struct SourcedCase {
    tag: &'static str,
    source: Ftl,
    translated: Ftl,
}

impl SourcedCase {
    /// Run `check` on this case.
    fn findings(self, check: fn(Comparison<'_>) -> Vec<String>) -> Vec<String> {
        let source = self.source.parse();
        let translation = self.translated.parse();
        check(Comparison {
            tag: LocaleTag::new(self.tag),
            source: &source,
            catalogue: &translation,
        })
    }
}

/// The fallback-note message the Gaelic catalogue lists as a known gap.
const FALLBACK_SOURCE: Ftl = Ftl("common-lint-count = { $lint }\n    .fallback-note = English.\n");

#[test]
fn check_keys_honours_and_retires_listed_exceptions() {
    let gap = SourcedCase {
        tag: "gd",
        source: FALLBACK_SOURCE,
        translated: Ftl("common-lint-count = { $lint }\n"),
    };
    let filled = SourcedCase {
        translated: Ftl("common-lint-count = { $lint }\n    .fallback-note = Translated.\n"),
        ..gap
    };
    Expected::none("is missing").assert(&gap.findings(check_keys));
    Expected::one("drop its exception").assert(&filled.findings(check_keys));
}

/// The branch-count message: its source uses the pre-rendered phrases.
const BRANCH_SOURCE: Ftl = Ftl(concat!(
    "conditional_max_n_branches = Collapse { $name } to { $limit_phrase }.\n",
    "    .note = { $name } contains { $branch_phrase }.\n",
));

/// A branch-count translation that keeps the pre-rendered phrases.
const WITH_PHRASES: Ftl = Ftl(concat!(
    "conditional_max_n_branches = { $name }: { $limit_phrase }.\n",
    "    .note = { $name }: { $branch_phrase }.\n",
));

#[test]
fn check_variables_requires_numbers_where_phrases_are_english() {
    let german = SourcedCase {
        tag: "de",
        source: BRANCH_SOURCE,
        translated: WITH_PHRASES,
    };
    let numbers = SourcedCase {
        translated: Ftl(concat!(
            "conditional_max_n_branches = { $name }: { $limit }.\n",
            "    .note = { $name }: { $branches }.\n",
        )),
        ..german
    };
    let welsh = SourcedCase {
        tag: "cy",
        ..german
    };
    Expected {
        needle: "which is rendered in English",
        count: 2,
    }
    .assert(&german.findings(check_variables));
    Expected::none("$").assert(&numbers.findings(check_variables));
    Expected::none("$").assert(&welsh.findings(check_variables));
}

#[test]
fn check_variables_allows_only_listed_drops() {
    let welsh = SourcedCase {
        tag: "cy",
        source: BRANCH_SOURCE,
        translated: Ftl(concat!(
            "conditional_max_n_branches = { $name }: { $limit_phrase }.\n",
            "    .note = { $branch_phrase }.\n",
        )),
    };
    let irish = SourcedCase { tag: "ga", ..welsh };
    Expected::none("drops").assert(&welsh.findings(check_variables));
    Expected::one("drops `$name`").assert(&irish.findings(check_variables));
}

/// A select that enumerates exact numbers, as Welsh `common-lint-count` does.
macro_rules! enumerated_select {
    () => {
        "{ $count ->\n        [0] none\n        [1] one\n       *[other] { $count }\n    }\n"
    };
}

#[test]
fn check_plural_selects_exempts_only_the_listed_welsh_select() {
    let listed = Ftl(concat!("common-lint-count = ", enumerated_select!()));
    let unlisted = Ftl(concat!("lines = ", enumerated_select!()));
    let case = |fixture: Ftl| SourcedCase {
        tag: "cy",
        source: fixture,
        translated: fixture,
    };
    Expected::none("selects on").assert(&case(listed).findings(check_plural_selects));
    Expected::one("`lines` selects on `$count`")
        .assert(&case(unlisted).findings(check_plural_selects));
}

#[test]
fn summarize_rejects_invalid_syntax() {
    assert!(
        summarize("message = Valid.\n#. Not a Fluent comment.\n").is_err(),
        "a `#.` line must be reported as a syntax error, not dropped as junk"
    );
    assert!(
        summarize("message = Valid.\n# A comment.\n").is_ok(),
        "a `#` comment must parse"
    );
}

#[test]
fn file_findings_reports_bidi_controls_and_file_set_drift() {
    let source = Source {
        catalogue: Catalogue::new(),
        file_names: [String::from("common.ftl")].into(),
    };
    let tag = LocaleTag::new("ar");
    let common = |text: &str| (String::from("common.ftl"), text.to_owned());
    let plain = LocaleFiles::from([common("message = نص")]);
    let marked = LocaleFiles::from([common("message = \u{200f}نص")]);
    let extra = LocaleFiles::from([
        common("message = نص"),
        (String::from("extra.ftl"), String::new()),
    ]);
    Expected::none("ar").assert(&file_findings(tag, &plain, &source));
    Expected::one("contains a bidirectional control").assert(&file_findings(tag, &marked, &source));
    Expected::one("file set differs").assert(&file_findings(tag, &extra, &source));
}

/// A fake rendering and whether `rendering_finding` must report it.
#[derive(Clone, Copy, Debug)]
struct RenderCase {
    rendered: &'static str,
    is_finding: bool,
}

/// The lookup every rendering fixture uses.
const GREETING: Lookup<'static> = Lookup {
    tag: LocaleTag::new("de"),
    key: "greeting",
};

#[rstest]
#[case::unresolved_placeable(RenderCase { rendered: "Value { $x }", is_finding: true })]
#[case::replacement_character(RenderCase { rendered: "Value \u{fffd}", is_finding: true })]
#[case::empty(RenderCase { rendered: "   ", is_finding: true })]
#[case::clean(RenderCase { rendered: "Value 3", is_finding: false })]
fn rendering_finding_reports_defective_output(#[case] case: RenderCase) {
    let render = |_: Lookup<'_>, _: SampleInput| Ok(case.rendered.to_owned());
    let finding = rendering_finding(GREETING, SampleInput::counting(1), &render);
    assert_eq!(
        finding.is_some(),
        case.is_finding,
        "{case:?} produced {finding:?}"
    );
}

#[test]
fn rendering_finding_passes_through_lookup_errors() {
    let render = |_: Lookup<'_>, _: SampleInput| Err(String::from("de: message missing"));
    assert_eq!(
        rendering_finding(GREETING, SampleInput::counting(1), &render),
        Some(String::from("de: message missing")),
        "a lookup error must surface as the finding"
    );
}

#[test]
fn copied_patterns_reports_text_equal_to_the_source() {
    let translation = Ftl("greeting = { $name }\n    .note = Remark.\n").parse();
    let tag = LocaleTag::new("de");
    let same = |_: Lookup<'_>, _: SampleInput| Ok(String::from("Same text"));
    let localized = |lookup: Lookup<'_>, _: SampleInput| Ok(format!("{}: text", lookup.tag));
    let copies = copied_patterns(tag, &translation, &same).expect("fake renderer cannot fail");
    let originals =
        copied_patterns(tag, &translation, &localized).expect("fake renderer cannot fail");
    Expected {
        needle: "is a copy of the source text",
        count: 2,
    }
    .assert(&copies);
    Expected::none("is a copy").assert(&originals);
}

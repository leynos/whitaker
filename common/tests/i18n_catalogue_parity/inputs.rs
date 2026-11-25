//! Typed inputs for rendering catalogue patterns through the loader.
//!
//! The parity gates render every message of every locale many times. Naming
//! the locale tag, the `$handling` selector, and each render request as small
//! `Copy` types keeps those calls readable and stops a tag, a key, and a
//! selector value from being passed in the wrong order as bare strings.

use std::{borrow::Cow, fmt};

use whitaker_common::i18n::{Arguments, FluentValue, branch_phrase};

/// A locale tag as it names a directory under `common/locales/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocaleTag<'a>(&'a str);

impl<'a> LocaleTag<'a> {
    /// Wrap a tag such as `pt-BR`.
    pub const fn new(tag: &'a str) -> Self { Self(tag) }

    /// Return the tag as written.
    pub const fn as_str(self) -> &'a str { self.0 }

    /// Return the language subtag, such as `pt` for `pt-BR`.
    pub fn language(self) -> &'a str {
        self.0
            .split_once('-')
            .map_or(self.0, |(primary, _)| primary)
    }

    /// Report whether the tag is a variant of the source language.
    pub fn is_english(self) -> bool { self.language() == "en" }
}

impl fmt::Display for LocaleTag<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter.write_str(self.0) }
}

/// The values `no_expect_outside_tests` passes as its `$handling` selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handling {
    /// The receiver is an `Option`.
    Option,
    /// The receiver is a `Result`.
    Result,
    /// The receiver is any other type.
    Other,
}

impl Handling {
    /// Every selector value, so each `$handling` variant gets rendered.
    pub const ALL: [Self; 3] = [Self::Option, Self::Result, Self::Other];

    /// Return the selector value the lint passes to Fluent.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Option => "option",
            Self::Result => "result",
            Self::Other => "other",
        }
    }
}

/// The variable values one render uses: a count and a `$handling` selector.
#[derive(Clone, Copy, Debug)]
pub struct SampleInput {
    /// The value of every counted variable (`$count`, `$lines`, and so on).
    pub count: i64,
    /// The `$handling` selector value.
    pub handling: Handling,
}

impl SampleInput {
    /// Build a sample with `count` and the default `$handling` variant.
    pub const fn counting(count: i64) -> Self {
        Self {
            count,
            handling: Handling::Other,
        }
    }
}

/// One pattern to render: a locale and an `id` or `id.attribute` key.
#[derive(Clone, Copy, Debug)]
pub struct Lookup<'a> {
    /// The locale to render in.
    pub tag: LocaleTag<'a>,
    /// The message key, optionally with an attribute suffix.
    pub key: &'a str,
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
///
/// `branch_phrase` is rendered for the requested locale, as the
/// `conditional_max_n_branches` lint does, so its English fallback for
/// unsupported languages is exercised too.
pub fn sample_arguments(tag: LocaleTag<'_>, input: SampleInput) -> Arguments<'static> {
    let branches = usize::try_from(input.count).unwrap_or(usize::MAX);
    let phrase = branch_phrase(tag.as_str(), branches);
    let strings = SAMPLE_STRINGS
        .into_iter()
        .map(|(name, value)| (name, FluentValue::from(value)));
    let numbers = sample_numbers(input.count)
        .into_iter()
        .map(|(name, value)| (name, FluentValue::from(value)));
    let derived = [
        ("handling", FluentValue::from(input.handling.as_str())),
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

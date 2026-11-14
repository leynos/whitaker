//! Rendered diagnostic snapshots for every supported locale.
//!
//! These snapshots pin the human-visible primary, note, and help text without
//! involving the compiler, so a Fluent edit is reviewed as a text change
//! rather than as a UI stderr diff.
#![cfg(feature = "dylint-driver")]
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

use std::borrow::Cow;

use whitaker_common::{
    Arguments,
    Localizer,
    MessageResolution,
    i18n::{DiagnosticMessageSet, FluentValue, MessageKey},
    noop_reporter,
    safe_resolve_message_set,
};

const LINT_NAME: &str = "macro_aware_cognitive_complexity";

#[test]
fn english_messages() {
    insta::assert_snapshot!("en_gb", render("en-GB"));
}

#[test]
fn welsh_messages() {
    insta::assert_snapshot!("cy", render("cy"));
}

#[test]
fn gaelic_messages() {
    insta::assert_snapshot!("gd", render("gd"));
}

#[test]
fn unknown_locale_messages() {
    insta::assert_snapshot!("zz", render("zz"));
}

#[test]
fn unknown_locale_matches_english() {
    assert_eq!(
        render("zz"),
        render("en-GB"),
        "an unknown locale must render the English text byte for byte"
    );
}

/// Render the primary, note, and help lines for `locale`.
fn render(locale: &str) -> String {
    let messages = resolve(&Localizer::new(Some(locale)));
    format!(
        "primary: {}\nnote: {}\nhelp: {}",
        messages.primary(),
        messages.note(),
        messages.help()
    )
}

/// Resolve the sample diagnostic through the shared Fluent loader.
fn resolve(localizer: &Localizer) -> DiagnosticMessageSet {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("name"),
        FluentValue::String(Cow::Owned(String::from("sample"))),
    );
    args.insert(Cow::Borrowed("score"), FluentValue::from(3_i64));
    args.insert(Cow::Borrowed("limit"), FluentValue::from(2_i64));

    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MessageKey::new(LINT_NAME),
        args: &args,
    };

    safe_resolve_message_set(localizer, resolution, noop_reporter, fallback_messages)
}

/// Build the English text used when a translation is missing.
fn fallback_messages() -> DiagnosticMessageSet {
    DiagnosticMessageSet::new(
        String::from("fallback primary"),
        String::from("fallback note"),
        String::from("fallback help"),
    )
}

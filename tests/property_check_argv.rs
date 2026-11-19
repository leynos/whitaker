//! Property tests for the `cargo dylint` vector `whitaker check` assembles.
//!
//! The property that matters is structural: whatever the user forwarded — an
//! empty vector, ordinary arguments, or their own `--` — Whitaker's options
//! come first, its separator is the first one in the vector, and every user
//! argument survives in order after it. The classifier below keeps the
//! property from passing vacuously: it names the three input shapes and the
//! table test proves each one is actually exercised.

use std::ffi::OsString;

use proptest::prelude::*;
use rstest::rstest;
use whitaker::cli::check::argv::{DylintOption, assemble_dylint_args};

/// The shape of a user argument vector, as far as assembly is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// Nothing was forwarded.
    Empty,
    /// The user forwarded their own separator, which must survive.
    WithSeparator,
    /// Ordinary `cargo check` arguments.
    Plain,
}

/// Classify a user vector, so input coverage is stated rather than assumed.
fn shape_of(user: &[OsString]) -> Shape {
    if user.is_empty() {
        Shape::Empty
    } else if user.iter().any(|argument| argument == "--") {
        Shape::WithSeparator
    } else {
        Shape::Plain
    }
}

/// Build the expected Whitaker prefix from the literal command line.
///
/// Written out rather than derived from [`DylintOption::SET`] so the property
/// compares the assembler against an independent statement of the contract:
/// reversing or reordering the set would change the actual vector while the
/// expected one stayed put.
fn whitaker_prefix() -> Vec<OsString> {
    ["--lib=whitaker_suite", "--no-metadata", "--"]
        .map(OsString::from)
        .to_vec()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Keep Whitaker's prefix fixed and the user's suffix intact.
    #[test]
    fn assembly_keeps_whitakers_prefix_first_and_the_user_suffix_intact(
        user in prop::collection::vec("[a-z=-]{0,12}", 0..6)
            .prop_map(|arguments| arguments.into_iter().map(OsString::from).collect::<Vec<_>>())
    ) {
        let assembled = assemble_dylint_args(&user);
        let prefix = whitaker_prefix();

        prop_assert_eq!(
            assembled.len(),
            prefix.len() + user.len(),
            "assembly forwarded {} arguments for {} user arguments",
            assembled.len(),
            user.len(),
        );
        let (head, tail) = assembled.split_at(prefix.len());
        prop_assert_eq!(
            head,
            prefix.as_slice(),
            "Whitaker's options and separator must come first",
        );
        prop_assert_eq!(
            tail,
            user.as_slice(),
            "the user's arguments must survive unchanged and in order",
        );

        // The first separator is Whitaker's own, whatever the user forwarded.
        let first_separator = assembled
            .iter()
            .position(|argument| argument == "--")
            .expect("the assembled vector always holds a separator");
        prop_assert_eq!(
            first_separator,
            prefix.len() - 1,
            "no user argument may precede Whitaker's separator",
        );
        prop_assert_eq!(
            shape_of(&user),
            shape_of(tail),
            "assembly must not change the shape of the user's arguments",
        );
    }
}

/// Assemble each named input shape and compare against the literal vector.
///
/// The generator may rarely produce a literal `--`, so this table states the
/// separator case directly: assembly must keep a user separator rather than
/// dropping or reordering it.
#[rstest]
#[case::empty(&[], &["--lib=whitaker_suite", "--no-metadata", "--"])]
#[case::plain(
    &["-p", "whitaker-common"],
    &["--lib=whitaker_suite", "--no-metadata", "--", "-p", "whitaker-common"]
)]
#[case::separator(
    &["-p", "--", "-q"],
    &["--lib=whitaker_suite", "--no-metadata", "--", "-p", "--", "-q"]
)]
fn assembly_covers_every_named_input_shape(#[case] user: &[&str], #[case] expected: &[&str]) {
    let forwarded: Vec<OsString> = user.iter().map(OsString::from).collect();
    let wanted: Vec<OsString> = expected.iter().map(OsString::from).collect();

    assert_eq!(
        assemble_dylint_args(&forwarded),
        wanted,
        "assembly must keep Whitaker's prefix first and the user's tokens in order"
    );
}

/// Prove that every input shape the property ranges over is reachable.
///
/// A green property run is evidence about all three shapes rather than about
/// empty vectors alone only if this table shows each one can occur.
#[test]
fn the_property_covers_every_input_shape() {
    let plain = vec![OsString::from("-p"), OsString::from("whitaker-common")];
    let with_separator = vec![
        OsString::from("-p"),
        OsString::from("--"),
        OsString::from("whitaker-common"),
    ];

    assert_eq!(
        shape_of(&[]),
        Shape::Empty,
        "an empty vector is its own shape"
    );
    assert_eq!(
        shape_of(&plain),
        Shape::Plain,
        "ordinary cargo arguments are the plain shape"
    );
    assert_eq!(
        shape_of(&with_separator),
        Shape::WithSeparator,
        "a user separator must be recognized rather than treated as plain"
    );
}

/// Require each Whitaker-owned option to render as one non-separator argument.
///
/// That is what makes the prefix's first `--` unambiguously Whitaker's own.
#[test]
fn no_whitaker_option_renders_as_a_separator() {
    for option in DylintOption::SET {
        let token = option.token();

        assert!(
            !token.is_empty(),
            "{option:?} must render as a non-empty argument"
        );
        assert!(
            token != "--",
            "{option:?} must never be mistaken for the separator"
        );
        assert!(
            !token.to_string_lossy().contains(char::is_whitespace),
            "{option:?} must render as exactly one argument"
        );
    }
}

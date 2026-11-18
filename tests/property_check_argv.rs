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

/// The arguments Whitaker contributes, in order.
fn whitaker_prefix() -> Vec<OsString> {
    let mut prefix: Vec<OsString> = DylintOption::SET
        .iter()
        .map(|option| option.token())
        .collect();
    prefix.push(OsString::from("--"));
    prefix
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Whitaker's prefix is fixed, and the user's suffix is untouched.
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

/// Every input shape the property ranges over is reachable, so a green run
/// above is evidence about all three rather than about empty vectors alone.
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

/// Every Whitaker-owned option renders as a single argument that is not the
/// separator, which is what makes the prefix's first `--` unambiguous.
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

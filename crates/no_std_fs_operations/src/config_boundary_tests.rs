//! Unit tests for the forward-compatible `dylint.toml` boundary.
//!
//! `BoundaryConfig` is the private deserializer that keeps the keys this suite
//! understands and consumes the rest, so an older suite still applies its
//! exclusions when a newer one writes an unknown key. These tests pin that
//! contract: which keys survive, how unknown values are consumed, and what a
//! malformed known value does.

use std::collections::HashSet;

use proptest::prelude::*;
use rstest::rstest;

use super::BoundaryConfig;

/// Keep the exclusions the suite understands while recording every unknown
/// top-level key, including nested tables and TOML temporal values.
#[rstest]
#[case::scalar(r"future_option = true", &["future_option"])]
#[case::quoted_key(r#""future option" = true"#, &["future option"])]
#[case::nested_table("[future_table]\nenabled = true", &["future_table"])]
#[case::local_date("future_date = 2026-01-02", &["future_date"])]
#[case::local_time("future_time = 07:32:00", &["future_time"])]
fn boundary_config_keeps_known_keys_and_records_unknown_values(
    #[case] unknown_toml: &str,
    #[case] expected_unknown: &[&str],
) {
    let toml = format!(
        concat!(
            "excluded_crates = [\"helper_crate\"]\n",
            "excluded_paths = [\"my_app::legacy_io\"]\n",
            "{unknown_toml}\n",
        ),
        unknown_toml = unknown_toml,
    );
    let boundary: BoundaryConfig = toml::from_str(&toml).expect("boundary config should parse");
    let (config, unknown_keys) = boundary.into_parts();

    assert_eq!(
        config.excluded_crates,
        HashSet::from(["helper_crate".to_owned()]),
        "the known crate exclusion should survive `{unknown_toml}`"
    );
    assert_eq!(
        config.excluded_paths,
        HashSet::from(["my_app::legacy_io".to_owned()]),
        "the known path exclusion should survive `{unknown_toml}`"
    );
    assert_eq!(
        unknown_keys, expected_unknown,
        "`{unknown_toml}` should be recorded as the unknown keys it contains"
    );
}

/// A name that is not one of the two keys this suite understands.
fn unknown_name() -> impl Strategy<Value = String> {
    "[a-c][a-z]{0,4}".prop_filter("known keys are generated separately", |name| {
        name != "excluded_crates" && name != "excluded_paths"
    })
}

/// A TOML value of a shape a newer suite might introduce.
fn unknown_value() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("true".to_owned()),
        (0_i32..100).prop_map(|number| number.to_string()),
        Just("\"text\"".to_owned()),
        Just("[1, 2]".to_owned()),
        Just("2026-01-02".to_owned()),
        Just("07:32:00".to_owned()),
    ]
}

// Keep known keys and sort unknown names for any key order and value shape.
//
// The `rstest` cases above fix the document order: the known fields always
// come first. `visit_map` reads keys in document order and accumulates
// unknown names as it goes, so the interleaved orders and value shapes a
// real forward-dated `dylint.toml` can contain are only covered here. A
// `proptest!` block is a macro invocation, so its comment cannot be a doc
// comment.
proptest! {
    #[test]
    fn boundary_config_keeps_known_keys_in_any_order(
        crates in prop::collection::vec("[a-z_]{1,8}", 0..=3),
        paths in prop::collection::vec("[a-z_]{1,4}(::[a-z_]{1,4}){0,2}", 0..=3),
        // `(name, value, position)` where position places the unknown entry
        // relative to the two known fields: before both, between them, or
        // after both.
        unknowns in prop::collection::vec((unknown_name(), unknown_value(), 0_usize..3), 0..=4),
    ) {
        let crates_toml = crates
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let paths_toml = paths
            .iter()
            .map(|path| format!("\"{path}\""))
            .collect::<Vec<_>>()
            .join(", ");

        // Group unknown entries by the position they take relative to the
        // known fields, so any generated set can appear in any order. A name
        // is written at most once: TOML rejects a repeated key, so a duplicate
        // would test the parser rather than this boundary.
        let mut before: Vec<(String, String)> = Vec::new();
        let mut between: Vec<(String, String)> = Vec::new();
        let mut after: Vec<(String, String)> = Vec::new();
        let mut written: HashSet<&str> = HashSet::new();
        for (name, value, position) in &unknowns {
            if !written.insert(name.as_str()) {
                continue;
            }
            let entry = (name.clone(), value.clone());
            match position {
                0 => before.push(entry),
                1 => between.push(entry),
                _ => after.push(entry),
            }
        }
        let render = |entries: &[(String, String)]| {
            entries
                .iter()
                .map(|(name, value)| format!("{name} = {value}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let (before_text, between_text, after_text) =
            (render(&before), render(&between), render(&after));

        let toml = format!(
            "{before_text}\nexcluded_crates = [{crates_toml}]\n{between_text}\nexcluded_paths = [{paths_toml}]\n{after_text}\n"
        );

        let boundary: BoundaryConfig =
            toml::from_str(&toml).expect("generated boundary config should parse");
        let (config, unknown_keys) = boundary.into_parts();

        prop_assert_eq!(config.excluded_crates, crates.iter().cloned().collect::<HashSet<_>>());
        prop_assert_eq!(config.excluded_paths, paths.iter().cloned().collect::<HashSet<_>>());

        // The reported names are exactly the generated ones, sorted and
        // deduplicated, whatever order the document used.
        let mut expected: Vec<String> = unknowns.iter().map(|(name, _, _)| name.clone()).collect();
        expected.sort_unstable();
        expected.dedup();
        prop_assert_eq!(unknown_keys, expected);
    }
}

/// Report unknown key names in a stable order regardless of document order.
#[test]
fn boundary_config_sorts_unknown_key_names() {
    let toml = concat!(
        "zulu = 1\n",
        "alpha = 2\n",
        "[middle]\n",
        "enabled = true\n",
    );
    let boundary: BoundaryConfig = toml::from_str(toml).expect("boundary config should parse");
    let (_, unknown_keys) = boundary.into_parts();
    assert_eq!(
        unknown_keys,
        ["alpha", "middle", "zulu"],
        "document order should not change the reported order"
    );
}

/// Treat an empty table as "no exclusions" rather than an error.
#[test]
fn boundary_config_defaults_missing_fields() {
    let boundary: BoundaryConfig = toml::from_str("").expect("empty boundary config should parse");
    let (config, unknown_keys) = boundary.into_parts();
    assert!(
        config.excluded_crates.is_empty(),
        "an empty table should exclude no crates"
    );
    assert!(
        config.excluded_paths.is_empty(),
        "an empty table should exclude no paths"
    );
    assert!(
        unknown_keys.is_empty(),
        "an empty table should report no unknown keys"
    );
}

/// Reject a wrong-typed known value instead of ignoring it, so a malformed
/// exclusion cannot appear to apply.
#[rstest]
#[case::wrong_crates_type(r#"excluded_crates = "not_an_array""#)]
#[case::wrong_crate_element_type("excluded_crates = [1, 2, 3]")]
#[case::wrong_paths_type(r#"excluded_paths = "not_an_array""#)]
#[case::wrong_path_element_type(r#"excluded_paths = ["my_app::legacy_io", 1]"#)]
fn boundary_config_rejects_wrong_known_types(#[case] toml: &str) {
    assert!(
        toml::from_str::<BoundaryConfig>(toml).is_err(),
        "expected error for: {toml}"
    );
}

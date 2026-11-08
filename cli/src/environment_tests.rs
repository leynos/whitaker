//! Tests for injected environment values and compatibility precedence.

use ortho_config::MapEnv;
use serde_json::json;

/// Preserve canonical and legacy values in distinct precedence layers.
#[test]
fn separates_legacy_and_canonical_locale() {
    let source = MapEnv::new()
        .with_var("DYLINT_LOCALE", "cy")
        .with_var("WHITAKER_DIAGNOSTICS_LOCALE", "en-GB");
    let (legacy, canonical, notice) =
        super::collect(&source).expect("collect injected environment layers");
    assert_eq!(legacy, json!({"diagnostics": {"locale": "cy"}}));
    assert_eq!(canonical, json!({"diagnostics": {"locale": "en-GB"}}));
    assert!(notice);
}

/// Treat one selector as a list and reject invalid boolean values.
#[test]
fn parses_single_selector_and_rejects_invalid_boolean() {
    let source = MapEnv::new().with_var("WHITAKER_LINT_SELECT", "PAN");
    assert_eq!(
        super::collect(&source)
            .expect("collect injected environment layers")
            .1,
        json!({"lint": {"select": ["PAN"]}})
    );
    let invalid = MapEnv::new().with_var("WHITAKER_LINT_EXPERIMENTAL", "perhaps");
    assert!(super::collect(&invalid).is_err());
}

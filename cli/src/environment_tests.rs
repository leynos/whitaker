//! Tests for injected environment values and compatibility precedence.

use ortho_config::MapEnv;
use serde_json::json;

/// Preserve canonical and legacy values in distinct precedence layers.
#[test]
fn separates_legacy_and_canonical_locale() -> ortho_config::OrthoResult<()> {
    let source = MapEnv::new()
        .with_var("DYLINT_LOCALE", "cy")
        .with_var("WHITAKER_DIAGNOSTICS_LOCALE", "en-GB");
    let (legacy, canonical, notice) = super::collect(&source)?;
    assert_eq!(legacy, json!({"diagnostics": {"locale": "cy"}}));
    assert_eq!(canonical, json!({"diagnostics": {"locale": "en-GB"}}));
    assert!(notice);
    Ok(())
}

/// Treat one selector as a list and reject invalid boolean values.
#[test]
fn parses_single_selector_and_rejects_invalid_boolean() -> ortho_config::OrthoResult<()> {
    let source = MapEnv::new().with_var("WHITAKER_LINT_SELECT", "PAN");
    assert_eq!(
        super::collect(&source)?.1,
        json!({"lint": {"select": ["PAN"]}})
    );
    let invalid = MapEnv::new().with_var("WHITAKER_LINT_EXPERIMENTAL", "perhaps");
    assert!(super::collect(&invalid).is_err());
    Ok(())
}

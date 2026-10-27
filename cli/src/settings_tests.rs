//! Tests for source ordering in the OrthoConfig settings model.

use super::Configuration;
use ortho_config::MergeComposer;
use serde_json::json;

/// Preserve the precedence of explicit arguments over environment and files.
#[test]
fn later_sources_override_earlier_sources() -> ortho_config::OrthoResult<()> {
    let mut layers = MergeComposer::new();
    layers.push_defaults(json!({"diagnostics": {"locale": "en-GB"}}));
    layers.push_file(json!({"diagnostics": {"locale": "cy"}}), None);
    layers.push_environment(json!({"diagnostics": {"locale": "fr"}}));
    layers.push_cli(json!({"diagnostics": {"locale": "de"}}));
    let config = Configuration::merge_from_layers(layers.layers())?;
    assert_eq!(config.diagnostics.locale, "de");
    Ok(())
}

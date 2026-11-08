//! Tests for source ordering in the `OrthoConfig` settings model.

use ortho_config::MergeComposer;
use serde_json::json;

use super::Configuration;

/// Preserve the precedence of explicit arguments over environment and files.
#[test]
fn later_sources_override_earlier_sources() {
    let mut layers = MergeComposer::new();
    layers.push_defaults(json!({"diagnostics": {"locale": "en-GB"}}));
    layers.push_file(json!({"diagnostics": {"locale": "cy"}}), None);
    layers.push_environment(json!({"diagnostics": {"locale": "fr"}}));
    layers.push_cli(json!({"diagnostics": {"locale": "de"}}));
    let config = Configuration::merge_from_layers(layers.layers())
        .expect("merge the configured source layers");
    assert_eq!(config.diagnostics.locale, "de");
}

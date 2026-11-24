//! Wire contract for the `properties.whitaker` property bag.
//!
//! The clone detector distinguishes "not measured" from a measured zero, so
//! every optional property must be absent rather than present with a
//! placeholder. These tests pin that boundary from outside the crate.

use serde_json::Value;
use whitaker_sarif::{
    BaselineState,
    RefinementKind,
    WhitakerProperties,
    WhitakerPropertiesBuilder,
};

/// Build the property bag for a properties value, failing on serialization.
fn whitaker_bag(properties: &WhitakerProperties) -> Value {
    let bag = match properties.try_to_value() {
        Ok(bag) => bag,
        Err(error) => panic!("properties should serialize: {error}"),
    };
    bag.get("whitaker").map_or_else(
        || panic!("the whitaker key must be present in {bag}"),
        Clone::clone,
    )
}

/// Check that unset optional properties are absent from the serialized bag.
#[test]
fn unset_optional_properties_are_omitted() {
    let properties = WhitakerPropertiesBuilder::new("T1")
        .build()
        .expect("default properties should build");
    let bag = whitaker_bag(&properties);

    for key in ["cosine", "refinement", "tokenRuleId", "astHashesEqual"] {
        assert!(
            bag.get(key).is_none(),
            "unset property `{key}` must be omitted rather than defaulted: {bag}"
        );
    }
}

/// Check that a measured cosine serializes under the documented key.
#[test]
fn measured_cosine_serializes() {
    let properties = WhitakerPropertiesBuilder::new("T3")
        .with_cosine(0.875)
        .build()
        .expect("finite cosine should build");
    let bag = whitaker_bag(&properties);

    assert_eq!(
        bag.get("cosine").and_then(Value::as_f64),
        Some(0.875),
        "the measured cosine must serialize under `cosine`: {bag}"
    );
}

/// Check that refinement provenance serializes with its documented spelling.
#[test]
fn refinement_provenance_serializes() {
    let properties = WhitakerPropertiesBuilder::new("T3")
        .with_cosine(0.9)
        .with_refinement(RefinementKind::Reclassified)
        .with_token_rule_id("WHK001")
        .with_ast_hashes_equal(false)
        .build()
        .expect("provenance properties should build");
    let bag = whitaker_bag(&properties);

    assert_eq!(
        bag.get("refinement").and_then(Value::as_str),
        Some("reclassified"),
        "refinement must serialize its lowercase spelling: {bag}"
    );
    assert_eq!(
        bag.get("tokenRuleId").and_then(Value::as_str),
        Some("WHK001"),
        "the token rule must serialize under `tokenRuleId`: {bag}"
    );
    assert_eq!(
        bag.get("astHashesEqual").and_then(Value::as_bool),
        Some(false),
        "hash equality must serialize under `astHashesEqual`: {bag}"
    );
}

/// Check that a fully populated bag survives extraction unchanged.
#[test]
fn populated_bag_round_trips_through_extraction() {
    let properties = WhitakerPropertiesBuilder::new("T2")
        .with_k(25)
        .with_window(16)
        .with_jaccard(0.95)
        .with_cosine(1.0)
        .with_group_id(0)
        .with_class_size(2)
        .with_refinement(RefinementKind::Carried)
        .with_token_rule_id("WHK002")
        .with_ast_hashes_equal(true)
        .build()
        .expect("populated properties should build");

    let bag = properties
        .try_to_value()
        .expect("properties should serialize");
    let extracted = WhitakerProperties::try_from(&bag).expect("properties should extract");

    assert_eq!(
        extracted, properties,
        "the serialized bag must round trip through extraction"
    );
}

/// Check that a result without a baseline state omits the wire property.
#[test]
fn result_without_baseline_state_omits_it() {
    let result = whitaker_sarif::ResultBuilder::new("WHK001")
        .with_message("clone")
        .build()
        .expect("a message-only result should build");
    let json = match serde_json::to_value(&result) {
        Ok(json) => json,
        Err(error) => panic!("result should serialize: {error}"),
    };

    assert!(
        json.get("baselineState").is_none(),
        "an unset baseline state must be omitted: {json}"
    );
}

/// Check that a typed baseline state serializes with its SARIF spelling.
#[test]
fn typed_baseline_state_serializes() {
    let result = whitaker_sarif::ResultBuilder::new("WHK003")
        .with_message("near miss")
        .with_baseline_state(BaselineState::New)
        .build()
        .expect("a result with a baseline state should build");
    let json = match serde_json::to_value(&result) {
        Ok(json) => json,
        Err(error) => panic!("result should serialize: {error}"),
    };

    assert_eq!(
        json.get("baselineState").and_then(Value::as_str),
        Some("new"),
        "the baseline state must serialize with its SARIF spelling: {json}"
    );
}

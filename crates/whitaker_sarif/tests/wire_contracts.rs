//! Regression and property coverage for the SARIF 2.1.0 JSON boundary.

use proptest::prelude::*;
use serde_json::{Value, json};
use whitaker_sarif::{Artefact, ArtefactLocation, PhysicalLocation, Region, Run, RunBuilder};

/// Generate varied relative Rust-source URIs for serialization properties.
fn uri_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(b'a'..=b'z', 1..32).prop_map(|bytes| {
        let path: String = bytes.into_iter().map(char::from).collect();
        format!("src/{path}.rs")
    })
}

/// Check the canonical physical-location key and reject Rust-name variants.
fn assert_physical_location_wire_keys(json: &Value) {
    assert!(
        json.get("artifactLocation").is_some(),
        "the canonical `artifactLocation` key should be present"
    );
    assert!(
        json.get("artefactLocation").is_none(),
        "the Rust-spelled `artefactLocation` key should be absent"
    );
    assert!(
        json.get("artefact_location").is_none(),
        "the Rust-spelled `artefact_location` key should be absent"
    );
}

/// Check the canonical run artefact key and reject the Rust field spelling.
fn assert_run_wire_keys(json: &Value) {
    assert!(
        json.get("artifacts").is_some(),
        "the canonical `artifacts` key should be present"
    );
    assert!(
        json.get("artefacts").is_none(),
        "the Rust-spelled `artefacts` key should be absent"
    );
}

/// Check a serialized location matches the canonical SARIF shape.
fn assert_serialized_location(serialized: &Value, expected: &Value) {
    assert_eq!(
        serialized.get("artifactLocation"),
        Some(expected),
        "the serialized location should use the canonical key"
    );
}

/// Check a decoded physical location reproduced every canonical field.
fn assert_decoded_physical_location(decoded: &PhysicalLocation) {
    assert_eq!(
        decoded.artefact_location.uri, "src/fixed.rs",
        "the decoded URI should come from the canonical key"
    );
    assert_eq!(
        decoded.artefact_location.uri_base_id.as_deref(),
        Some("%SRCROOT%"),
        "the decoded base identifier should survive the canonical key"
    );
    assert_eq!(
        decoded.region.as_ref().map(|region| region.start_line),
        Some(7),
        "the decoded region should retain its start line"
    );
}

/// Check the first decoded artefact matches the canonical SARIF shape.
fn assert_decoded_run(run: &Run, expected: &[Artefact]) {
    assert_eq!(
        run.artefacts.as_slice(),
        expected,
        "the decoded artefacts should come from the canonical key"
    );
}

/// Check the builder retained the artefact it was given.
fn assert_builder_retained_artefact(run: &Run, expected: &Artefact) {
    assert_eq!(
        run.artefacts,
        vec![expected.clone()],
        "the builder should retain the artefact it was given"
    );
}

/// Check the serialized artefact nest uses the canonical keys throughout.
fn assert_serialized_artefact_uses_canonical_keys(serialized: &Value) {
    let artefact_list = serialized.get("artifacts").and_then(Value::as_array);
    assert_eq!(
        artefact_list
            .and_then(|artefacts| artefacts.first())
            .and_then(|entry| entry.get("location"))
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str),
        Some("src/main.rs"),
        "the serialized artefact should use the canonical keys"
    );
}

/// Check a re-serialized value reproduces the canonical JSON exactly.
fn assert_reserializes_to(actual: &Value, canonical: &Value) {
    assert_eq!(
        actual, canonical,
        "re-serializing the decoded model should reproduce the canonical JSON"
    );
}

/// Verify populated locations use the SARIF key and decode canonical JSON.
#[test]
fn populated_physical_location_uses_the_canonical_sarif_property() -> Result<(), serde_json::Error>
{
    let physical = PhysicalLocation {
        artefact_location: ArtefactLocation {
            uri: "src/main.rs".into(),
            uri_base_id: Some("%SRCROOT%".into()),
        },
        region: Some(Region {
            start_line: 12,
            start_column: Some(4),
            end_line: Some(14),
            end_column: Some(19),
            byte_offset: Some(128),
            byte_length: Some(64),
        }),
    };

    let serialized = serde_json::to_value(&physical)?;
    assert_physical_location_wire_keys(&serialized);
    assert_serialized_location(
        &serialized,
        &json!({"uri": "src/main.rs", "uriBaseId": "%SRCROOT%"}),
    );

    let canonical = json!({
        "artifactLocation": {"uri": "src/fixed.rs", "uriBaseId": "%SRCROOT%"},
        "region": {"startLine": 7, "endLine": 8}
    });
    let decoded: PhysicalLocation = serde_json::from_value(canonical.clone())?;
    assert_decoded_physical_location(&decoded);

    let round_trip = serde_json::to_value(&decoded)?;
    assert_reserializes_to(&round_trip, &canonical);
    Ok(())
}

/// Verify the public run builder and model retain the canonical `artifacts` key.
#[test]
fn populated_run_builder_preserves_the_canonical_sarif_property() -> Result<(), serde_json::Error> {
    let artefact = Artefact {
        location: ArtefactLocation {
            uri: "src/main.rs".into(),
            uri_base_id: Some("%SRCROOT%".into()),
        },
        mime_type: Some("text/x-rust".into()),
    };
    let run = RunBuilder::new("whitaker", "0.3.0")
        .with_artefact(artefact.clone())
        .build();
    assert_builder_retained_artefact(&run, &artefact);

    let serialized = serde_json::to_value(&run)?;
    assert_run_wire_keys(&serialized);
    assert_serialized_artefact_uses_canonical_keys(&serialized);

    let canonical = json!({
        "tool": {"driver": {"name": "whitaker", "version": "0.3.0"}},
        "artifacts": [{
            "location": {"uri": "src/fixed.rs", "uriBaseId": "%SRCROOT%"},
            "mimeType": "text/x-rust"
        }]
    });
    let decoded: Run = serde_json::from_value(canonical.clone())?;
    let expected_artefacts = [Artefact {
        location: ArtefactLocation {
            uri: "src/fixed.rs".into(),
            uri_base_id: Some("%SRCROOT%".into()),
        },
        mime_type: Some("text/x-rust".into()),
    }];
    assert_decoded_run(&decoded, &expected_artefacts);

    let round_trip = serde_json::to_value(&decoded)?;
    assert_reserializes_to(&round_trip, &canonical);
    Ok(())
}

proptest! {
    /// Check varied URIs, optional base IDs, and optional regions retain the
    /// canonical location key and survive JSON round-trips.
    #[test]
    fn generated_physical_locations_keep_the_canonical_wire_key(
        uri in uri_strategy(),
        uri_base_id in prop::option::of(uri_strategy()),
        region in prop::option::of((1_usize..=1000, prop::option::of(1_usize..=500)).prop_map(|(start_line, start_column)| Region {
            start_line,
            start_column,
            end_line: None,
            end_column: None,
            byte_offset: None,
            byte_length: None,
        })),
    ) {
        let physical = PhysicalLocation {
            artefact_location: ArtefactLocation { uri, uri_base_id },
            region,
        };
        let serialized = serde_json::to_value(&physical);
        prop_assert!(serialized.is_ok());
        let serialized_json = serialized.unwrap_or(Value::Null);
        prop_assert!(serialized_json.get("artifactLocation").is_some());
        prop_assert!(serialized_json.get("artefactLocation").is_none());
        prop_assert!(serialized_json.get("artefact_location").is_none());

        let decoded = serde_json::from_value::<PhysicalLocation>(serialized_json.clone());
        prop_assert!(decoded.is_ok());
        let decoded_location = decoded.unwrap_or_else(|_| physical.clone());
        prop_assert_eq!(decoded_location.clone(), physical);
        let round_trip = serde_json::to_value(decoded_location);
        prop_assert!(round_trip.is_ok());
        let round_trip_json = round_trip.unwrap_or(Value::Null);
        prop_assert_eq!(round_trip_json, serialized_json);
    }

    /// Check non-empty artefact lists with optional base IDs and MIME types
    /// retain the canonical run key and survive JSON round-trips.
    #[test]
    fn generated_runs_keep_the_canonical_wire_key_and_round_trip(
        artefacts in prop::collection::vec(
            (uri_strategy(), prop::option::of(uri_strategy()), prop::option::of(any::<bool>()).prop_map(|value| value.map(|_| "text/x-rust".to_owned()))),
            1..5
        ),
    ) {
        let expected_artefacts: Vec<Artefact> = artefacts
            .into_iter()
            .map(|(uri, uri_base_id, mime_type)| Artefact {
                location: ArtefactLocation { uri, uri_base_id },
                mime_type,
            })
            .collect();
        let run = expected_artefacts
            .iter()
            .cloned()
            .fold(RunBuilder::new("whitaker", "0.3.0"), |builder, artefact| {
                builder.with_artefact(artefact)
            })
            .build();
        prop_assert!(!run.artefacts.is_empty());

        let serialized = serde_json::to_value(&run);
        prop_assert!(serialized.is_ok());
        let serialized_json = serialized.unwrap_or(Value::Null);
        prop_assert!(serialized_json.get("artifacts").is_some());
        prop_assert!(serialized_json.get("artefacts").is_none());
        let serialized_artefacts = serialized_json.get("artifacts");
        prop_assert_eq!(
            serialized_artefacts.and_then(Value::as_array).map(Vec::len),
            Some(run.artefacts.len())
        );

        let decoded = serde_json::from_value::<Run>(serialized_json.clone());
        prop_assert!(decoded.is_ok());
        let decoded_run = decoded.unwrap_or_else(|_| run.clone());
        prop_assert_eq!(decoded_run.artefacts.as_slice(), expected_artefacts.as_slice());
        let round_trip = serde_json::to_value(decoded_run);
        prop_assert!(round_trip.is_ok());
        let round_trip_json = round_trip.unwrap_or(Value::Null);
        prop_assert_eq!(round_trip_json, serialized_json);
    }
}

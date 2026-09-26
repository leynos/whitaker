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
    assert!(json.get("artifactLocation").is_some());
    assert!(json.get("artefactLocation").is_none());
    assert!(json.get("artefact_location").is_none());
}

/// Check the canonical run artefact key and reject the Rust field spelling.
fn assert_run_wire_keys(json: &Value) {
    assert!(json.get("artifacts").is_some());
    assert!(json.get("artefacts").is_none());
}

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
    assert_eq!(
        serialized["artifactLocation"],
        json!({"uri": "src/main.rs", "uriBaseId": "%SRCROOT%"})
    );

    let canonical = json!({
        "artifactLocation": {"uri": "src/fixed.rs", "uriBaseId": "%SRCROOT%"},
        "region": {"startLine": 7, "endLine": 8}
    });
    let decoded: PhysicalLocation = serde_json::from_value(canonical.clone())?;
    assert_eq!(decoded.artefact_location.uri, "src/fixed.rs");
    assert_eq!(
        decoded.artefact_location.uri_base_id.as_deref(),
        Some("%SRCROOT%")
    );
    assert_eq!(
        decoded.region.as_ref().map(|region| region.start_line),
        Some(7)
    );
    assert_eq!(serde_json::to_value(decoded)?, canonical);
    Ok(())
}

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
    assert_eq!(run.artefacts, vec![artefact]);

    let serialized = serde_json::to_value(&run)?;
    assert_run_wire_keys(&serialized);
    assert_eq!(serialized["artifacts"][0]["location"]["uri"], "src/main.rs");

    let canonical = json!({
        "tool": {"driver": {"name": "whitaker", "version": "0.3.0"}},
        "artifacts": [{
            "location": {"uri": "src/fixed.rs", "uriBaseId": "%SRCROOT%"},
            "mimeType": "text/x-rust"
        }]
    });
    let decoded: Run = serde_json::from_value(canonical.clone())?;
    assert_eq!(
        decoded.artefacts,
        vec![Artefact {
            location: ArtefactLocation {
                uri: "src/fixed.rs".into(),
                uri_base_id: Some("%SRCROOT%".into()),
            },
            mime_type: Some("text/x-rust".into()),
        }]
    );
    assert_eq!(serde_json::to_value(decoded)?, canonical);
    Ok(())
}

proptest! {
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
        let serialized = serialized.unwrap_or(Value::Null);
        prop_assert!(serialized.get("artifactLocation").is_some());
        prop_assert!(serialized.get("artefactLocation").is_none());
        prop_assert!(serialized.get("artefact_location").is_none());

        let decoded = serde_json::from_value::<PhysicalLocation>(serialized.clone());
        prop_assert!(decoded.is_ok());
        let decoded = decoded.unwrap_or_else(|_| physical.clone());
        prop_assert_eq!(decoded.clone(), physical);
        let round_trip = serde_json::to_value(decoded);
        prop_assert!(round_trip.is_ok());
        let round_trip = round_trip.unwrap_or(Value::Null);
        prop_assert_eq!(round_trip, serialized);
    }

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
        let serialized = serialized.unwrap_or(Value::Null);
        prop_assert!(serialized.get("artifacts").is_some());
        prop_assert!(serialized.get("artefacts").is_none());
        prop_assert_eq!(
            serialized["artifacts"].as_array().map(|artefacts| artefacts.len()),
            Some(run.artefacts.len())
        );

        let decoded = serde_json::from_value::<Run>(serialized.clone());
        prop_assert!(decoded.is_ok());
        let decoded = decoded.unwrap_or_else(|_| run.clone());
        prop_assert_eq!(decoded.artefacts.as_slice(), expected_artefacts.as_slice());
        let round_trip = serde_json::to_value(decoded);
        prop_assert!(round_trip.is_ok());
        let round_trip = round_trip.unwrap_or(Value::Null);
        prop_assert_eq!(round_trip, serialized);
    }
}

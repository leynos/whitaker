//! Compile contract for the public Oxford-spelled SARIF model and builder API.

use whitaker_sarif::{Artefact, ArtefactLocation, PhysicalLocation, RunBuilder};

fn main() {
    let artefact = Artefact {
        location: ArtefactLocation {
            uri: "src/main.rs".into(),
            uri_base_id: None,
        },
        mime_type: None,
    };
    let run = RunBuilder::new("whitaker", "0.3.0")
        .with_artefact(artefact)
        .build();
    let location = PhysicalLocation {
        artefact_location: ArtefactLocation {
            uri: "src/main.rs".into(),
            uri_base_id: None,
        },
        region: None,
    };
    assert_eq!(run.artefacts.len(), 1);
    assert_eq!(location.artefact_location.uri, "src/main.rs");
}

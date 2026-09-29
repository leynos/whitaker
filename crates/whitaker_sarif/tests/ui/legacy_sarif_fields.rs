//! Compile-fail contract for removed SARIF model and builder field spellings.

use whitaker_sarif::{Artefact, ArtefactLocation, PhysicalLocation, RunBuilder};

fn main() {
    let _location = PhysicalLocation {
        artifact_location: ArtefactLocation {
            uri: "src/main.rs".into(),
            uri_base_id: None,
        },
        region: None,
    };
    let run = RunBuilder::new("whitaker", "0.3.0").build();
    let _ = run.artifacts;
    let _ = RunBuilder::new("whitaker", "0.3.0").with_artifact(Artefact {
        location: ArtefactLocation {
            uri: "src/main.rs".into(),
            uri_base_id: None,
        },
        mime_type: None,
    });
}

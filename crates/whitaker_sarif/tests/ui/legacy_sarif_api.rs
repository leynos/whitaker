//! Compile-fail contract for removed American-spelled SARIF API names.

use whitaker_sarif::{Artifact, ArtifactLocation, RunBuilder};

fn main() {
    let _ = RunBuilder::new("whitaker", "0.3.0").with_artifact(Artifact {
        location: ArtifactLocation {
            uri: "src/main.rs".into(),
            uri_base_id: None,
        },
        mime_type: None,
    });
}

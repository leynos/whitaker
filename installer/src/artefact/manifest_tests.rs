//! Tests for manifest schema types.

use super::*;
use rstest::{fixture, rstest};
use serde_json::Value;

#[fixture]
fn sample_provenance() -> ManifestProvenance {
    ManifestProvenance {
        git_sha: GitSha::try_from("abc1234").expect("valid sha"),
        schema_version: SchemaVersion::current(),
        toolchain: ToolchainChannel::try_from("nightly-2025-09-18").expect("valid channel"),
        target: TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target"),
    }
}

#[fixture]
fn sample_content() -> ManifestContent {
    ManifestContent {
        generated_at: GeneratedAt::new("2026-02-03T00:00:00Z"),
        files: vec!["libwhitaker_lints@nightly-2025-09-18-x86_64-unknown-linux-gnu.so".to_owned()],
        sha256: Sha256Digest::try_from("a".repeat(64).as_str()).expect("valid digest"),
    }
}

#[fixture]
fn sample_manifest(
    sample_provenance: ManifestProvenance,
    sample_content: ManifestContent,
) -> Manifest {
    Manifest::new(sample_provenance, sample_content)
}

#[rstest]
fn accessors_return_all_fields(sample_manifest: Manifest) {
    assert_eq!(sample_manifest.git_sha().as_str(), "abc1234");
    assert_eq!(sample_manifest.schema_version().as_u32(), 1);
    assert_eq!(sample_manifest.toolchain().as_str(), "nightly-2025-09-18");
    assert_eq!(
        sample_manifest.target().as_str(),
        "x86_64-unknown-linux-gnu"
    );
    assert_eq!(
        sample_manifest.generated_at().as_str(),
        "2026-02-03T00:00:00Z"
    );
    assert_eq!(sample_manifest.files().len(), 1);
    assert_eq!(sample_manifest.sha256().as_str().len(), 64);
}

#[rstest]
fn generated_at_display() {
    let ts = GeneratedAt::new("2026-02-03T00:00:00Z");
    assert_eq!(format!("{ts}"), "2026-02-03T00:00:00Z");
}

#[rstest]
fn serialized_json_contains_all_adr_001_keys(sample_manifest: Manifest) {
    let json = serde_json::to_string(&sample_manifest).expect("serialization succeeds");
    let parsed: Value = serde_json::from_str(&json).expect("valid JSON");
    let obj = parsed.as_object().expect("top-level object");

    // ADR-001 specifies these seven top-level keys.
    let required_keys = [
        "git_sha",
        "schema_version",
        "toolchain",
        "target",
        "generated_at",
        "files",
        "sha256",
    ];
    for key in &required_keys {
        assert!(obj.contains_key(*key), "missing key: {key}");
    }

    assert_eq!(obj.get("git_sha").and_then(Value::as_str), Some("abc1234"));
    assert_eq!(obj.get("schema_version").and_then(Value::as_u64), Some(1));
    assert_eq!(
        obj.get("toolchain").and_then(Value::as_str),
        Some("nightly-2025-09-18")
    );
    assert_eq!(
        obj.get("target").and_then(Value::as_str),
        Some("x86_64-unknown-linux-gnu")
    );
    assert!(
        obj.get("files")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
    );
    assert!(obj.get("sha256").and_then(Value::as_str).is_some());
}

#[rstest]
fn manifest_with_multiple_files() {
    let provenance = ManifestProvenance {
        git_sha: GitSha::try_from("deadbeef").expect("valid"),
        schema_version: SchemaVersion::current(),
        toolchain: ToolchainChannel::try_from("nightly-2025-09-18").expect("valid"),
        target: TargetTriple::try_from("aarch64-apple-darwin").expect("valid"),
    };
    let content = ManifestContent {
        generated_at: GeneratedAt::new("2026-02-03T12:00:00Z"),
        files: vec!["file_a.dylib".to_owned(), "file_b.dylib".to_owned()],
        sha256: Sha256Digest::try_from("b".repeat(64).as_str()).expect("valid"),
    };
    let m = Manifest::new(provenance, content);
    assert_eq!(m.files().len(), 2);
}

#[rstest]
fn serde_round_trip(sample_manifest: Manifest) {
    let json = serde_json::to_string_pretty(&sample_manifest).expect("serialize");
    let back: Manifest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(sample_manifest, back);
}

#[rstest]
#[case::invalid_target(
    r#"{
        "git_sha": "abc1234",
        "schema_version": 1,
        "toolchain": "nightly-2025-09-18",
        "target": "wasm32-unknown-unknown",
        "generated_at": "2026-02-03T00:00:00Z",
        "files": ["lib.so"],
        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }"#
)]
#[case::missing_fields(r#"{"git_sha": "abc1234"}"#)]
fn deserialize_rejects_invalid_payloads(#[case] json: &str) {
    let result: std::result::Result<Manifest, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

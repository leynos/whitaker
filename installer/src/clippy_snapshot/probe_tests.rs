//! Snapshot queries report paths they cannot inspect instead of reading them
//! as absent.
//!
//! A NUL byte makes every platform refuse a path with `InvalidInput`, which is
//! an inspection failure rather than absence on Linux, macOS, and Windows
//! alike.

use super::super::{
    BundleArtefact, ClippySnapshotError, ClippySnapshotManifest, GenerationMetadata,
    check_installed, parse_manifest, verify_repository,
};
use camino::Utf8Path;
use rstest::rstest;

const COMMIT: &str = "e4608a25c2e20116fbc1e97a51a73f094a1db7a1";

/// An uninspectable path.
const UNINSPECTABLE: &str = "uninspectable\0path";

/// A manifest pinning [`COMMIT`]; the tests never reach its other fields.
fn manifest() -> ClippySnapshotManifest {
    parse_manifest(&format!(
        concat!(
            "schema_version = 1\n",
            "upstream = \"https://example.invalid/rust-clippy\"\n",
            "branch = \"main\"\n",
            "commit = \"{COMMIT}\"\n",
            "toolchain = \"nightly-2026-05-28\"\n",
            "selected_revision = \"{COMMIT}\"\n",
            "dylint_version = \"6.3.0\"\n",
        ),
        COMMIT = COMMIT,
    ))
    .expect("parse the test manifest")
}

/// Regression: `is_dir` and `is_file` once turned these failures into
/// `NotInstalled` or `BundleMissing`, hiding the environmental fault.
#[rstest]
#[case::generation_metadata(|path: &Utf8Path| GenerationMetadata::read(path).map(drop))]
#[case::installed_check(|path: &Utf8Path| check_installed(path, COMMIT))]
#[case::repository_verification(|path: &Utf8Path| verify_repository(path, &manifest()))]
#[case::bundle_sidecar(|path: &Utf8Path| BundleArtefact::from_sidecar(path, &manifest()).map(drop))]
fn an_uninspectable_path_is_an_io_error(
    #[case] query: fn(&Utf8Path) -> Result<(), ClippySnapshotError>,
) {
    let error = query(Utf8Path::new(UNINSPECTABLE)).expect_err("cannot inspect the path");

    assert!(matches!(error, ClippySnapshotError::Io { .. }), "{error:?}");
}

#[rstest]
#[case::generation_metadata(|path: &Utf8Path| GenerationMetadata::read(path).map(drop))]
#[case::repository_verification(|path: &Utf8Path| verify_repository(path, &manifest()))]
fn an_absent_generation_is_still_not_installed(
    #[case] query: fn(&Utf8Path) -> Result<(), ClippySnapshotError>,
) {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let absent = Utf8Path::from_path(dir.path())
        .expect("UTF-8 temp path")
        .join(COMMIT);

    let error = query(&absent).expect_err("nothing is installed");

    assert!(
        matches!(error, ClippySnapshotError::NotInstalled { .. }),
        "{error:?}"
    );
}

#[test]
fn an_absent_bundle_is_still_missing() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let absent = Utf8Path::from_path(dir.path())
        .expect("UTF-8 temp path")
        .join("whitaker-clippy-snapshot.bundle");

    let error = BundleArtefact::from_sidecar(&absent, &manifest()).expect_err("no bundle");

    assert!(
        matches!(error, ClippySnapshotError::BundleMissing { .. }),
        "{error:?}"
    );
}

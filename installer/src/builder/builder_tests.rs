//! Unit tests for the lint-crate build orchestration.
//!
//! These exercise feature resolution, library naming, and the revision a build
//! command publishes. The clean/error paths of the Git probe itself live in
//! `builder_revision_tests.rs`.

use rstest::{fixture, rstest};

use super::*;

#[fixture]
fn builder() -> Builder {
    Builder::new(BuildConfig {
        toolchain: Toolchain::with_override(&Utf8PathBuf::from("/tmp/test"), "nightly-2026-05-28"),
        target_dir: Utf8PathBuf::from("/tmp/target"),
        jobs: None,
        verbosity: 0,
        experimental: false,
    })
}

#[test]
fn library_extension_is_correct() {
    let ext = library_extension();
    #[cfg(target_os = "linux")]
    assert_eq!(ext, ".so");
    #[cfg(target_os = "macos")]
    assert_eq!(ext, ".dylib");
    #[cfg(target_os = "windows")]
    assert_eq!(ext, ".dll");
    // Fallback for other Unix-like platforms (e.g., FreeBSD, OpenBSD)
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    assert_eq!(ext, ".so");
}

#[rstest]
#[case::non_suite_crate("module_max_lines", false, "dylint-driver")]
#[case::non_suite_with_experimental("module_max_lines", true, "dylint-driver")]
#[case::suite_without_experimental("whitaker_suite", false, "dylint-driver")]
fn features_for_crate_returns_expected_features(
    mut builder: Builder,
    #[case] crate_name: &str,
    #[case] experimental: bool,
    #[case] expected: &str,
) {
    builder.config.experimental = experimental;
    let result = builder
        .features_for_crate(&CrateName::from(crate_name))
        .expect("features should resolve without reading a manifest");
    assert_eq!(result, expected);
}

/// Builds a builder whose workspace holds a suite declaring `manifest`.
fn builder_for_suite(manifest: &str) -> (tempfile::TempDir, Builder) {
    let dir = tempfile::tempdir().expect("temporary workspace");
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("UTF-8 path");
    std::fs::create_dir_all(root.join("suite")).expect("create suite directory");
    std::fs::write(root.join("suite/Cargo.toml"), manifest).expect("write suite manifest");
    let builder = Builder::new(BuildConfig {
        toolchain: Toolchain::with_override(&root, "nightly-2026-05-28"),
        target_dir: root.join("target"),
        jobs: None,
        verbosity: 0,
        experimental: true,
    });
    (dir, builder)
}

/// Select the feature set a suite manifest declares in experimental mode.
///
/// A current suite advertises the aggregate `experimental-enable-all`
/// feature, and a pinned older suite lists each lint instead. Both must reach
/// the build, so they are one table rather than two near-identical bodies.
#[rstest]
#[case::aggregate(
    "[features]\ndylint-driver = []\nexperimental-enable-all = [\"dylint-driver\"]\n",
    "dylint-driver,experimental-enable-all"
)]
#[case::per_lint(
    concat!(
        "[features]\n",
        "dylint-driver = []\n",
        "experimental-rstest-helper-should-be-fixture = [\"dylint-driver\"]\n",
    ),
    "dylint-driver,experimental-rstest-helper-should-be-fixture",
)]
fn a_suite_gets_the_features_its_manifest_declares(#[case] manifest: &str, #[case] expected: &str) {
    let (_dir, builder) = builder_for_suite(manifest);
    let result = builder
        .features_for_crate(&CrateName::from("whitaker_suite"))
        .expect("features should resolve");
    assert_eq!(result, expected);
}

/// A missing suite manifest fails before Cargo is run.
#[test]
fn an_unreadable_suite_manifest_fails_the_build_before_cargo_runs() {
    let (dir, builder) = builder_for_suite("[features]\n");
    std::fs::remove_file(dir.path().join("suite/Cargo.toml")).expect("remove manifest");
    let error = builder
        .features_for_crate(&CrateName::from("whitaker_suite"))
        .expect_err("a missing manifest must not become an empty feature list");
    assert!(matches!(error, InstallerError::Io(_)), "got {error:?}");
}

/// Read the revision a prepared build command would publish.
///
/// `build_command` is the same helper `build_crate` executes, so these
/// assertions cover the production wiring rather than a parallel copy.
fn published_revision(probe: MockSuiteRevisionProbe) -> Option<String> {
    let builder = Builder::with_probe(test_config(), Box::new(probe));
    let command = builder
        .build_command(&CrateName::from("module_max_lines"))
        .expect("a non-suite crate resolves without a manifest");
    command
        .get_envs()
        .find(|(name, _)| *name == "WHITAKER_SUITE_REVISION")
        .and_then(|(_, value)| value)
        .map(|value| value.to_string_lossy().into_owned())
}

/// Build configuration shared by the command seam tests.
fn test_config() -> BuildConfig {
    BuildConfig {
        toolchain: Toolchain::with_override(&Utf8PathBuf::from("/tmp/test"), "nightly-2026-05-28"),
        target_dir: Utf8PathBuf::from("/tmp/target"),
        jobs: None,
        verbosity: 0,
        experimental: false,
    }
}

/// Publish the revision a successful probe reports.
#[test]
fn a_successful_probe_publishes_its_revision() {
    let mut probe = MockSuiteRevisionProbe::new();
    probe
        .expect_probe()
        .returning(|_| Ok(Some("abc1234+dirty".to_owned())));

    assert_eq!(
        published_revision(probe).as_deref(),
        Some("abc1234+dirty"),
        "the build command should carry the probed revision"
    );
}

/// Clear the variable when the probe cannot resolve a revision.
#[test]
fn a_failed_probe_publishes_no_revision() {
    let mut probe = MockSuiteRevisionProbe::new();
    probe.expect_probe().returning(|_| {
        Err(crate::builder::revision::ProbeError::NotUtf8 {
            args: "rev-parse".to_owned(),
        })
    });

    assert_eq!(
        published_revision(probe),
        None,
        "a failed probe must clear the variable rather than inherit one"
    );
}

/// Treat a directory outside Git as absent provenance, not a failure.
#[test]
fn a_non_repository_probe_publishes_no_revision() {
    let mut probe = MockSuiteRevisionProbe::new();
    probe.expect_probe().returning(|_| Ok(None));

    assert_eq!(published_revision(probe), None);
}

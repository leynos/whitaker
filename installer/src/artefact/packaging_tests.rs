//! Unit tests for the artefact packaging module.

use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::*;
use crate::artefact::error::ArtefactError;

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn temp_dir() -> std::io::Result<TempDir> { TempDir::new() }

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn sample_git_sha() -> Result<GitSha, ArtefactError> { GitSha::try_from("abc1234") }

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn sample_toolchain() -> Result<ToolchainChannel, ArtefactError> {
    ToolchainChannel::try_from("nightly-2026-05-28")
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn sample_target() -> Result<TargetTriple, ArtefactError> {
    TargetTriple::try_from("x86_64-unknown-linux-gnu")
}

#[rstest]
fn compute_sha256_of_known_content(#[from(temp_dir)] temp_dir_res: std::io::Result<TempDir>) {
    let temp_dir = temp_dir_res.expect("temporary directory should be created");

    let path = temp_dir.path().join("test.bin");
    // SHA-256 of empty file is the well-known constant.
    fs::write(&path, b"").expect("write");
    let digest = compute_sha256(&path).expect("sha256 succeeds");
    assert_eq!(
        digest.as_str(),
        concat!(
            "e3b0c44298fc1c149afbf4c8996fb924",
            "27ae41e4649b934ca495991b7852b855"
        )
    );
}

#[rstest]
fn create_archive_contains_files(#[from(temp_dir)] temp_dir_res: std::io::Result<TempDir>) {
    let temp_dir = temp_dir_res.expect("temporary directory should be created");

    let file_a = temp_dir.path().join("a.txt");
    let file_b = temp_dir.path().join("b.txt");
    fs::write(&file_a, b"alpha").expect("write a");
    fs::write(&file_b, b"bravo").expect("write b");

    let archive_path = temp_dir.path().join("test.tar.zst");
    create_archive(
        &archive_path,
        &[(file_a, "a.txt".to_owned()), (file_b, "b.txt".to_owned())],
    )
    .expect("archive creation succeeds");

    let entry_names = list_archive_entries(&archive_path).expect("archive should list entries");
    assert!(entry_names.contains(&"a.txt".to_owned()));
    assert!(entry_names.contains(&"b.txt".to_owned()));
}

#[rstest]
fn generate_manifest_json_matches_schema(
    #[from(sample_git_sha)] sample_git_sha_res: Result<GitSha, ArtefactError>,
    #[from(sample_toolchain)] sample_toolchain_res: Result<ToolchainChannel, ArtefactError>,
    #[from(sample_target)] sample_target_res: Result<TargetTriple, ArtefactError>,
) {
    let sample_git_sha = sample_git_sha_res.expect("sample git SHA should validate");
    let sample_toolchain = sample_toolchain_res.expect("sample toolchain should validate");
    let sample_target = sample_target_res.expect("sample target should validate");

    let provenance = ManifestProvenance {
        git_sha: sample_git_sha,
        schema_version: SchemaVersion::current(),
        toolchain: sample_toolchain,
        target: sample_target,
    };
    let content = ManifestContent {
        generated_at: GeneratedAt::new("2026-02-11T00:00:00Z"),
        files: vec!["libtest.so".to_owned()],
        sha256: Sha256Digest::try_from("a".repeat(64)).expect("valid digest"),
    };
    let manifest = Manifest::new(provenance, content);
    let json = generate_manifest_json(&manifest).expect("serialization");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let obj = parsed.as_object().expect("object");

    for key in &[
        "git_sha",
        "schema_version",
        "toolchain",
        "target",
        "generated_at",
        "files",
        "sha256",
    ] {
        assert!(obj.contains_key(*key), "missing key: {key}");
    }
}

#[rstest]
fn package_artefact_produces_valid_archive(
    #[from(temp_dir)] temp_dir_res: std::io::Result<TempDir>,
    #[from(sample_git_sha)] sample_git_sha_res: Result<GitSha, ArtefactError>,
    #[from(sample_toolchain)] sample_toolchain_res: Result<ToolchainChannel, ArtefactError>,
    #[from(sample_target)] sample_target_res: Result<TargetTriple, ArtefactError>,
) {
    let temp_dir = temp_dir_res.expect("temporary directory should be created");
    let sample_git_sha = sample_git_sha_res.expect("sample git SHA should validate");
    let sample_toolchain = sample_toolchain_res.expect("sample toolchain should validate");
    let sample_target = sample_target_res.expect("sample target should validate");

    let lib_path = temp_dir.path().join("libwhitaker_suite.so");
    fs::write(&lib_path, b"fake library").expect("write lib");

    let output_dir = temp_dir.path().join("dist");
    fs::create_dir_all(&output_dir).expect("mkdir");

    let params = PackageParams {
        git_sha: sample_git_sha.clone(),
        toolchain: sample_toolchain.clone(),
        target: sample_target.clone(),
        library_files: vec![lib_path],
        output_dir,
        generated_at: GeneratedAt::new("2026-02-11T10:00:00Z"),
    };

    let output = package_artefact(&params).expect("packaging succeeds");
    assert!(output.archive_path.exists());

    let expected_name = ArtefactName::new(sample_git_sha, sample_toolchain, sample_target);
    assert_eq!(
        output
            .archive_path
            .file_name()
            .expect("filename")
            .to_string_lossy(),
        expected_name.filename()
    );

    let entry_names =
        list_archive_entries(&output.archive_path).expect("archive should list entries");
    assert!(entry_names.contains(&"libwhitaker_suite.so".to_owned()));
    assert!(
        !entry_names.contains(&"manifest.json".to_owned()),
        "manifest must not be embedded in the archive"
    );
}

#[rstest]
fn package_artefact_rejects_empty_files(#[from(temp_dir)] temp_dir_res: std::io::Result<TempDir>) {
    let temp_dir = temp_dir_res.expect("temporary directory should be created");

    let output_dir = temp_dir.path().join("dist");
    fs::create_dir_all(&output_dir).expect("mkdir");

    let params = PackageParams {
        git_sha: GitSha::try_from("abc1234").expect("valid"),
        toolchain: ToolchainChannel::try_from("nightly-2026-05-28").expect("valid"),
        target: TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid"),
        library_files: vec![],
        output_dir,
        generated_at: GeneratedAt::new("2026-02-11T10:00:00Z"),
    };

    let result = package_artefact(&params);
    assert!(matches!(
        result.expect_err("expected error"),
        PackagingError::EmptyFileList
    ));
}

#[rstest]
fn package_artefact_fails_when_library_file_missing(
    #[from(temp_dir)] temp_dir_res: std::io::Result<TempDir>,
) {
    let temp_dir = temp_dir_res.expect("temporary directory should be created");

    let missing = temp_dir.path().join("nonexistent_lib.so");
    let output_dir = temp_dir.path().join("dist");
    fs::create_dir_all(&output_dir).expect("mkdir");

    let params = PackageParams {
        git_sha: GitSha::try_from("abc1234").expect("valid"),
        toolchain: ToolchainChannel::try_from("nightly-2026-05-28").expect("valid"),
        target: TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid"),
        library_files: vec![missing],
        output_dir,
        generated_at: GeneratedAt::new("2026-02-11T10:00:00Z"),
    };

    let result = package_artefact(&params);
    assert!(result.is_err(), "expected error for missing library file");
    assert!(
        matches!(result.expect_err("checked above"), PackagingError::Io(_)),
        "expected Io error variant"
    );
}

#[rstest]
fn archive_name_follows_adr_convention(
    #[from(sample_git_sha)] sample_git_sha_res: Result<GitSha, ArtefactError>,
    #[from(sample_toolchain)] sample_toolchain_res: Result<ToolchainChannel, ArtefactError>,
    #[from(sample_target)] sample_target_res: Result<TargetTriple, ArtefactError>,
    #[from(temp_dir)] temp_dir_res: std::io::Result<TempDir>,
) {
    let sample_git_sha = sample_git_sha_res.expect("sample git SHA should validate");
    let sample_toolchain = sample_toolchain_res.expect("sample toolchain should validate");
    let sample_target = sample_target_res.expect("sample target should validate");
    let temp_dir = temp_dir_res.expect("temporary directory should be created");

    let lib_path = temp_dir.path().join("libtest.so");
    fs::write(&lib_path, b"content").expect("write");

    let output_dir = temp_dir.path().join("out");
    fs::create_dir_all(&output_dir).expect("mkdir");

    let params = PackageParams {
        git_sha: sample_git_sha.clone(),
        toolchain: sample_toolchain.clone(),
        target: sample_target.clone(),
        library_files: vec![lib_path],
        output_dir,
        generated_at: GeneratedAt::new("2026-02-11T00:00:00Z"),
    };

    let output = package_artefact(&params).expect("packaging");
    let expected = ArtefactName::new(sample_git_sha, sample_toolchain, sample_target);
    assert_eq!(
        output
            .archive_path
            .file_name()
            .expect("filename")
            .to_string_lossy(),
        expected.filename()
    );
}

/// Create a [`PackageOutput`] with a single dummy library for tests that
/// only need a valid package without caring about specific file content.
fn create_test_package(temp_dir: &TempDir) -> Result<PackageOutput, PackagingError> {
    let lib_path = temp_dir.path().join("libtest.so");
    fs::write(&lib_path, b"test content for hash")?;

    let output_dir = temp_dir.path().join("dist");
    fs::create_dir_all(&output_dir)?;

    let params = PackageParams {
        git_sha: GitSha::try_from("deadbeef")?,
        toolchain: ToolchainChannel::try_from("nightly-2026-05-28")?,
        target: TargetTriple::try_from("x86_64-unknown-linux-gnu")?,
        library_files: vec![lib_path],
        output_dir,
        generated_at: GeneratedAt::new("2026-02-11T12:00:00Z"),
    };

    package_artefact(&params)
}

#[rstest]
fn manifest_sha256_is_valid_hex(#[from(temp_dir)] temp_dir_res: std::io::Result<TempDir>) {
    let temp_dir = temp_dir_res.expect("temporary directory should be created");

    let output = create_test_package(&temp_dir).expect("test package should build");
    assert_eq!(output.manifest.sha256().as_str().len(), 64);
    assert!(
        output
            .manifest
            .sha256()
            .as_str()
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    );
}

#[rstest]
fn manifest_sha256_matches_archive_digest(
    #[from(temp_dir)] temp_dir_res: std::io::Result<TempDir>,
) {
    let temp_dir = temp_dir_res.expect("temporary directory should be created");

    let output = create_test_package(&temp_dir).expect("test package should build");
    let archive_digest = compute_sha256(&output.archive_path).expect("sha256 of archive");
    assert_eq!(
        archive_digest.as_str(),
        output.manifest.sha256().as_str(),
        "manifest sha256 must match the archive's actual digest"
    );
}

#[rstest]
fn packaging_produces_deterministic_digest(
    #[from(temp_dir)] temp_dir_res: std::io::Result<TempDir>,
) {
    let temp_dir = temp_dir_res.expect("temporary directory should be created");

    let lib_path = temp_dir.path().join("libtest.so");
    fs::write(&lib_path, b"content for hash check").expect("write");

    // Run packaging twice with identical inputs; digests must match.
    let mut digests = Vec::new();
    for i in 0..2 {
        let output_dir = temp_dir.path().join(format!("dist{i}"));
        fs::create_dir_all(&output_dir).expect("mkdir");
        let params = PackageParams {
            git_sha: GitSha::try_from("abc1234").expect("valid"),
            toolchain: ToolchainChannel::try_from("nightly-2026-05-28").expect("valid"),
            target: TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid"),
            library_files: vec![lib_path.clone()],
            output_dir,
            generated_at: GeneratedAt::new("2026-02-12T10:00:00Z"),
        };
        let output = package_artefact(&params).expect("packaging");
        digests.push(output.manifest.sha256().as_str().to_owned());
    }
    let [first, second] = digests.as_slice() else {
        panic!("expected exactly two digests, got {}", digests.len());
    };
    assert_eq!(
        first, second,
        "identical inputs must produce identical manifest digests"
    );
}

#[rstest]
fn collect_file_names_rejects_path_without_filename() {
    let paths = vec![PathBuf::from("/some/valid.so"), PathBuf::from("/")];
    let result = collect_file_names(&paths);
    assert!(
        matches!(
            result.expect_err("expected error"),
            PackagingError::InvalidLibraryPath(_)
        ),
        "expected InvalidLibraryPath for root path"
    );
}

/// Extract entry names from a `.tar.zst` archive for test assertions.
fn list_archive_entries(archive_path: &Path) -> std::io::Result<Vec<String>> {
    let file = fs::File::open(archive_path)?;
    let decoder = zstd::Decoder::new(file)?;
    let mut archive = tar::Archive::new(decoder);
    archive
        .entries()?
        .map(|entry_result| {
            let entry = entry_result?;
            let path = entry.path()?;
            Ok(path.to_string_lossy().into_owned())
        })
        .collect()
}

//! End-to-end pruning coverage for issue 565.
//!
//! A prebuilt archive can hold standalone libraries for crates the running
//! installer was never compiled to know. These tests drive a real `.tar.zst`
//! through the real extractor and the prune, so the guarantee is asserted on
//! the artefacts the installer actually consumes rather than on staged files
//! a test wrote by hand.

use std::{
    fs,
    path::{Path, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};

use super::{
    super::{prune_prebuilt_libraries, staged_library_filename},
    TestBaseDirs,
    create_staged_library,
};
use crate::{
    artefact::{
        download::{ArtefactDownloader, DownloadError},
        extraction::ZstdExtractor,
    },
    crate_name::CrateName,
    error::InstallerError,
    prebuilt::{PrebuiltConfig, PrebuiltResult, attempt_prebuilt_with},
    prebuilt_path::prebuilt_library_dir,
    resolution::SUITE_CRATE,
    test_utils::{library_basename, prebuilt_manifest_json_with_files, write_library_archive},
};

const TARGET: &str = "x86_64-unknown-linux-gnu";
const TOOLCHAIN: &str = "nightly-2026-05-28";
const OTHER_TOOLCHAIN: &str = "nightly-2020-01-01";
/// Stands in for a lint crate added to the suite after this installer shipped.
const UNKNOWN_CRATE: &str = "a_crate_added_after_this_release";
/// A staged library left by an earlier install that this archive never delivered.
const STALE_CRATE: &str = "a_stale_crate_from_an_earlier_install";

/// A temporary Whitaker data directory with its prebuilt library destination.
struct PrebuiltDir {
    temp: tempfile::TempDir,
    library_dir: Utf8PathBuf,
}

/// Serve one fixture manifest and copy one fixture archive to the request.
struct FixtureDownloader {
    manifest: String,
    archive: PathBuf,
}

impl ArtefactDownloader for FixtureDownloader {
    /// Return the fixture manifest without contacting the network.
    fn download_manifest(&self, _target: &str) -> Result<String, DownloadError> {
        Ok(self.manifest.clone())
    }

    /// Copy the fixture archive to the destination the pipeline requests.
    fn download_archive(&self, _filename: &str, dest: &Path) -> Result<(), DownloadError> {
        fs::copy(&self.archive, dest)
            .map(|_bytes_copied| ())
            .map_err(DownloadError::Io)
    }
}

/// Create a temporary prebuilt library directory beneath a fresh data root.
#[fixture]
fn prebuilt_dir() -> PrebuiltDir {
    let temp = tempfile::tempdir().expect("temp dir should be available");
    let dirs = TestBaseDirs {
        data_dir: Some(temp.path().to_path_buf()),
    };
    let library_dir = prebuilt_library_dir(&dirs, TOOLCHAIN, TARGET)
        .expect("prebuilt library directory should resolve");
    fs::create_dir_all(library_dir.as_std_path())
        .expect("prebuilt library directory should be creatable");
    PrebuiltDir { temp, library_dir }
}

/// Seed the files a prune must never touch, returning their names.
///
/// The README is not a library. The other-toolchain library is a staged
/// library, but not for this install's toolchain. The stale library is a
/// staged library for this toolchain that this archive did not deliver, so
/// the prune must remove it.
fn seed_preserved_files(library_dir: &Utf8Path) -> Vec<String> {
    create_staged_library(library_dir, "module_max_lines", OTHER_TOOLCHAIN);
    create_staged_library(library_dir, STALE_CRATE, TOOLCHAIN);
    fs::write(
        library_dir.join("README.txt").as_std_path(),
        b"not a library",
    )
    .expect("README seed should be written");
    vec![
        "README.txt".to_owned(),
        staged_library_filename("module_max_lines", OTHER_TOOLCHAIN),
    ]
}

/// Stage a real archive through the real extractor, returning what it delivered.
fn stage_archive(fixture: &PrebuiltDir, crate_names: &[&str]) -> Vec<String> {
    let archive_path = write_library_archive(fixture.temp.path(), crate_names)
        .expect("fixture archive should be written");
    let archive_bytes = fs::read(&archive_path).expect("fixture archive should be readable");
    let files: Vec<String> = crate_names
        .iter()
        .map(|name| library_basename(name))
        .collect();
    let manifest = prebuilt_manifest_json_with_files(TOOLCHAIN, TARGET, &archive_bytes, &files);
    let downloader = FixtureDownloader {
        manifest,
        archive: archive_path,
    };
    let config = PrebuiltConfig {
        target: TARGET,
        toolchain: TOOLCHAIN,
        destination_dir: &fixture.library_dir,
        quiet: true,
        allow_source_fallback: false,
    };
    let mut stderr = Vec::new();

    match attempt_prebuilt_with(&config, &downloader, &ZstdExtractor, &mut stderr) {
        PrebuiltResult::Success {
            staging_path,
            delivered,
        } => {
            assert_eq!(
                staging_path, fixture.library_dir,
                "the archive should stage into the requested prebuilt library directory",
            );
            delivered
        }
        PrebuiltResult::Fallback { reason } => panic!(
            "staging the fixture archive should succeed: {reason}; stderr: {}",
            String::from_utf8_lossy(&stderr)
        ),
    }
}

/// Assert the directory holds exactly `expected`, in any order.
fn assert_directory_holds(directory: &Utf8Path, expected: &[String]) {
    let mut actual: Vec<String> = fs::read_dir(directory.as_std_path())
        .expect("prebuilt library directory should be readable")
        .map(|entry| {
            entry
                .expect("directory entry should be readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    actual.sort_unstable();
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort_unstable();

    assert_eq!(
        actual, expected_sorted,
        "the prune should leave exactly the expected files",
    );
}

/// Regression test for whitaker issue 565.
#[rstest]
fn a_suite_install_prunes_libraries_the_installer_does_not_know(prebuilt_dir: PrebuiltDir) {
    let mut expected = seed_preserved_files(&prebuilt_dir.library_dir);
    expected.push(staged_library_filename(SUITE_CRATE, TOOLCHAIN));

    let delivered = stage_archive(
        &prebuilt_dir,
        &[SUITE_CRATE, "module_max_lines", UNKNOWN_CRATE],
    );
    let mut expected_delivered = vec![
        staged_library_filename(SUITE_CRATE, TOOLCHAIN),
        staged_library_filename("module_max_lines", TOOLCHAIN),
        staged_library_filename(UNKNOWN_CRATE, TOOLCHAIN),
    ];
    expected_delivered.sort_unstable();
    let mut delivered_sorted = delivered.clone();
    delivered_sorted.sort_unstable();
    assert_eq!(
        delivered_sorted, expected_delivered,
        "the archive should deliver every staged library it holds",
    );

    prune_prebuilt_libraries(&prebuilt_dir.library_dir, TOOLCHAIN, &[], &delivered)
        .expect("a suite install should prune every unrequested library");

    assert_directory_holds(&prebuilt_dir.library_dir, &expected);
}

/// An individual install keeps only the requested library, not the suite or
/// an unknown crate the archive also delivered.
#[rstest]
fn an_individual_install_prunes_the_suite_and_unknown_library(prebuilt_dir: PrebuiltDir) {
    let mut expected = seed_preserved_files(&prebuilt_dir.library_dir);
    expected.push(staged_library_filename("module_max_lines", TOOLCHAIN));

    let delivered = stage_archive(
        &prebuilt_dir,
        &[SUITE_CRATE, "module_max_lines", UNKNOWN_CRATE],
    );
    prune_prebuilt_libraries(
        &prebuilt_dir.library_dir,
        TOOLCHAIN,
        &[CrateName::from("module_max_lines")],
        &delivered,
    )
    .expect("an individual install should prune the suite and unknown library");

    assert_directory_holds(&prebuilt_dir.library_dir, &expected);
}

/// An archive that does not hold the requested crate is refused, naming it.
#[rstest]
fn an_archive_without_the_requested_crate_is_refused(prebuilt_dir: PrebuiltDir) {
    let delivered = stage_archive(&prebuilt_dir, &[SUITE_CRATE, UNKNOWN_CRATE]);

    let error = prune_prebuilt_libraries(
        &prebuilt_dir.library_dir,
        TOOLCHAIN,
        &[CrateName::from("module_max_lines")],
        &delivered,
    )
    .expect_err("an archive that lacks a requested crate must not pass as an install");

    assert!(
        matches!(
            &error,
            InstallerError::StagingFailed { reason } if reason.contains("module_max_lines")
        ),
        "the error should name the missing crate, got: {error:?}",
    );
}

//! Every published tar archive stores its files as regular entries.
//!
//! `tar::Builder` writes a file with holes as a GNU sparse entry (typeflag
//! `S`) unless told not to, and cargo-binstall's extractor skips every entry
//! that is not `Regular` or `Directory` without an error, so it extracted
//! nothing from the installer archive and fell back to a source build (#461).
//! A linked release binary often has holes on the build runner's filesystem,
//! and a sparse entry reads back correctly through `tar`, so only the entry
//! type shows the defect. Each packager is driven here over a file with a
//! hole, and the fixture is proved to produce a sparse entry under a default
//! builder, so these tests cannot pass on a fixture that never builds the case.

use crate::artefact::packaging::create_archive;
use crate::dependency_binaries::find_dependency_binary;
use crate::dependency_packaging::{DependencyPackageParams, package_dependency_binary};
use crate::installer_packaging::{
    InstallerPackageParams, TargetTriple, Version, package_installer,
};
use rstest::{fixture, rstest};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tar::EntryType;

/// The hole's length: large enough that every Linux filesystem allocates no
/// blocks for it, so `SEEK_HOLE` reports it.
const HOLE_BYTES: u64 = 4 * 1024 * 1024;

/// A temporary directory holding one file with a hole in its middle.
struct HoleyBinary {
    temp: tempfile::TempDir,
    path: PathBuf,
}

/// Write a file of `HOLE_BYTES` with data only at each end.
#[fixture]
fn holey_binary() -> HoleyBinary {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("binary");
    let mut file = fs::File::create(&path).expect("create binary");
    file.write_all(b"head").expect("write head");
    file.seek(SeekFrom::Start(HOLE_BYTES))
        .expect("seek past hole");
    file.write_all(b"tail").expect("write tail");
    HoleyBinary { temp, path }
}

/// Return each entry's path and type from an uncompressed tar stream.
fn entry_types(reader: impl Read) -> Vec<(String, EntryType)> {
    tar::Archive::new(reader)
        .entries()
        .expect("entries")
        .map(|entry| {
            let entry = entry.expect("valid tar entry");
            let path = entry.path().expect("entry path").display().to_string();
            (path, entry.header().entry_type())
        })
        .collect()
}

/// Return the entries of a `.tgz` archive.
fn tgz_entry_types(path: &Path) -> Vec<(String, EntryType)> {
    let file = fs::File::open(path).expect("open archive");
    entry_types(flate2::read::GzDecoder::new(file))
}

/// Assert that an archive has entries and that each is a file or directory.
fn assert_regular_entries(entries: &[(String, EntryType)]) {
    assert!(!entries.is_empty(), "the archive has no entries");
    let irregular: Vec<_> = entries
        .iter()
        .filter(|(_, kind)| !matches!(kind, EntryType::Regular | EntryType::Directory))
        .collect();
    assert!(
        irregular.is_empty(),
        "cargo-binstall skips these entries: {irregular:?}"
    );
}

#[cfg(target_os = "linux")]
#[rstest]
fn the_fixture_is_stored_sparse_by_a_default_builder(holey_binary: HoleyBinary) {
    let mut builder = tar::Builder::new(Vec::new());
    builder
        .append_path_with_name(&holey_binary.path, "binary")
        .expect("append");
    let bytes = builder.into_inner().expect("finish archive");
    let entries = entry_types(bytes.as_slice());
    assert_eq!(
        entries,
        vec![("binary".to_owned(), EntryType::GNUSparse)],
        "without a sparse entry here the tests below prove nothing"
    );
}

#[rstest]
fn the_installer_archive_holds_regular_entries(holey_binary: HoleyBinary) {
    let output = package_installer(InstallerPackageParams {
        version: Version::new("0.2.9"),
        target: TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target"),
        binary_path: holey_binary.path.clone(),
        output_dir: holey_binary.temp.path().join("dist"),
    })
    .expect("package installer");
    assert_regular_entries(&tgz_entry_types(&output.archive_path));
}

#[rstest]
fn a_dependency_archive_holds_regular_entries(holey_binary: HoleyBinary) {
    let dependency = find_dependency_binary("dylint-link")
        .expect("dependency manifest should load")
        .expect("dependency should exist");
    let output = package_dependency_binary(DependencyPackageParams {
        dependency: dependency.clone(),
        target: TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target"),
        binary_path: holey_binary.path.clone(),
        output_dir: holey_binary.temp.path().join("dist"),
    })
    .expect("package dependency");
    assert_regular_entries(&tgz_entry_types(&output.archive_path));
}

#[rstest]
fn a_lint_library_archive_holds_regular_entries(holey_binary: HoleyBinary) {
    let archive_path = holey_binary.temp.path().join("lints.tar.zst");
    let files = [(holey_binary.path.clone(), "libwhitaker.so".to_owned())];
    create_archive(&archive_path, &files).expect("create archive");
    let file = fs::File::open(&archive_path).expect("open archive");
    let decoder = zstd::Decoder::new(file).expect("zstd decoder");
    assert_regular_entries(&entry_types(decoder));
}

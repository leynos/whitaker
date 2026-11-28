//! Fixtures for prebuilt-archive and manifest tests.
//!
//! These helpers build the artefacts the installer consumes: a flat
//! `.tar.zst` archive of plain library names, the sidecar manifest that
//! describes it, and the platform library names both derive. They live in
//! their own module so the broader `test_utils` surface stays readable.

use std::path::PathBuf;

/// Compute the SHA-256 hex digest of a byte slice for test fixtures.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    crate::hex::to_lower_hex(&Sha256::digest(data))
}

/// Build prebuilt manifest JSON for tests with configurable fields.
#[must_use]
pub fn prebuilt_manifest_json(
    toolchain: impl AsRef<str>,
    target: impl AsRef<str>,
    sha256: impl AsRef<str>,
) -> String {
    let toolchain_value = toolchain.as_ref();
    let target_value = target.as_ref();
    let sha256_value = sha256.as_ref();
    format!(
        concat!(
            r#"{{"git_sha":"abc1234","schema_version":1,"#,
            r#""toolchain":"{toolchain}","#,
            r#""target":"{target}","#,
            r#""generated_at":"2026-02-03T00:00:00Z","#,
            r#""files":["libwhitaker_suite.so"],"#,
            r#""sha256":"{sha256}"}}"#,
        ),
        toolchain = toolchain_value,
        target = target_value,
        sha256 = sha256_value,
    )
}

/// Return the plain (unstaged) library file name for a lint crate.
///
/// The name follows the same platform conventions the packaging binary uses:
/// the platform prefix, the crate name with hyphens normalised to
/// underscores, and the platform extension.
///
/// # Examples
///
/// ```
/// use whitaker_installer::{
///     builder::{library_extension, library_prefix},
///     test_utils::library_basename,
/// };
///
/// assert_eq!(
///     library_basename("module-max-lines"),
///     format!(
///         "{}module_max_lines{}",
///         library_prefix(),
///         library_extension()
///     ),
/// );
/// ```
#[must_use]
pub fn library_basename(crate_name: &str) -> String {
    format!(
        "{}{}{}",
        crate::builder::library_prefix(),
        crate_name.replace('-', "_"),
        crate::builder::library_extension(),
    )
}

/// Write a flat `.tar.zst` archive holding one plain library per crate name.
///
/// Entries mirror what `whitaker-package-lints` produces: plain
/// `lib<crate><extension>` names, which the installer renames into its staged
/// form during extraction. The archive is written into `directory` and its
/// path is returned.
///
/// # Errors
///
/// Returns an I/O error when the archive cannot be created, an entry cannot
/// be appended, or the zstd frame cannot be finished.
///
/// # Examples
///
/// ```
/// use whitaker_installer::test_utils::write_library_archive;
///
/// let directory = tempfile::tempdir().expect("temp dir should be available");
/// let archive = write_library_archive(directory.path(), &["whitaker_suite"])
///     .expect("fixture archive should be written");
/// assert!(archive.is_file());
/// ```
pub fn write_library_archive(
    directory: &std::path::Path,
    crate_names: &[&str],
) -> std::io::Result<PathBuf> {
    // Contents are irrelevant to pruning; only the entry names matter.
    const LIBRARY_CONTENTS: &[u8] = b"fake prebuilt library";
    let archive_path = directory.join("whitaker-lints-fixture.tar.zst");
    let output = std::fs::File::create(&archive_path)?;
    let encoder = zstd::Encoder::new(output, 0)?;
    let mut builder = tar::Builder::new(encoder);
    for crate_name in crate_names {
        let basename = library_basename(crate_name);
        let mut header = tar::Header::new_gnu();
        header.set_size(LIBRARY_CONTENTS.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, &basename, LIBRARY_CONTENTS)?;
    }
    let finished_encoder = builder.into_inner()?;
    finished_encoder.finish()?;
    Ok(archive_path)
}

/// Build prebuilt manifest JSON for an archive holding `files`.
///
/// The digest is computed from `archive`, so a fixture downloader that serves
/// those bytes passes the installer's checksum verification. File names are
/// inserted verbatim and must be JSON-safe; library basenames derived from
/// crate names are.
///
/// # Examples
///
/// ```
/// use whitaker_installer::test_utils::prebuilt_manifest_json_with_files;
///
/// let manifest = prebuilt_manifest_json_with_files(
///     "nightly-2026-05-28",
///     "x86_64-unknown-linux-gnu",
///     b"archive bytes",
///     &["libwhitaker_suite.so".to_owned()],
/// );
/// assert!(manifest.contains(r#""files":["libwhitaker_suite.so"]"#));
/// ```
#[must_use]
pub fn prebuilt_manifest_json_with_files(
    toolchain: impl AsRef<str>,
    target: impl AsRef<str>,
    archive: &[u8],
    files: &[String],
) -> String {
    let files_json = files
        .iter()
        .map(|name| format!(r#""{name}""#))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        concat!(
            r#"{{"git_sha":"abc1234","schema_version":1,"#,
            r#""toolchain":"{}","#,
            r#""target":"{}","#,
            r#""generated_at":"2026-02-03T00:00:00Z","#,
            r#""files":[{}],"#,
            r#""sha256":"{}"}}"#,
        ),
        toolchain.as_ref(),
        target.as_ref(),
        files_json,
        sha256_hex(archive),
    )
}

#[cfg(test)]
mod tests {
    //! Tests for the prebuilt-archive fixture helpers.

    use super::{
        library_basename,
        prebuilt_manifest_json_with_files,
        sha256_hex,
        write_library_archive,
    };
    use crate::{
        artefact::extraction::{ArtefactExtractor, ZstdExtractor},
        builder::{library_extension, library_prefix},
    };

    #[test]
    fn sha256_hex_matches_known_vector() {
        // SHA-256 of "abc" is a well-known NIST test vector.
        assert_eq!(
            sha256_hex(b"abc"),
            concat!(
                "ba7816bf8f01cfea414140de5dae2223",
                "b00361a396177a9cb410ff61f20015ad",
            ),
        );
    }

    #[test]
    fn sha256_hex_of_empty_input_is_the_known_constant() {
        assert_eq!(
            sha256_hex(b""),
            concat!(
                "e3b0c44298fc1c149afbf4c8996fb924",
                "27ae41e4649b934ca495991b7852b855",
            ),
        );
    }

    #[test]
    fn library_basename_uses_platform_naming_and_normalises_hyphens() {
        let basename = library_basename("module-max-lines");

        assert_eq!(
            basename,
            format!(
                "{}module_max_lines{}",
                library_prefix(),
                library_extension()
            ),
            "a hyphenated crate name should become a platform library basename",
        );
    }

    #[test]
    fn write_library_archive_round_trips_through_the_real_extractor() {
        let directory = tempfile::tempdir().expect("temp dir should be available");
        let archive = write_library_archive(
            directory.path(),
            &["whitaker_suite", "a_crate_added_after_this_release"],
        )
        .expect("fixture archive should be written");
        let destination = directory.path().join("out");
        std::fs::create_dir_all(&destination).expect("extraction destination should be created");

        let extracted = ZstdExtractor
            .extract(&archive, &destination)
            .expect("fixture archive should extract");

        let mut names = extracted;
        names.sort_unstable();
        assert_eq!(
            names,
            vec![
                library_basename("a_crate_added_after_this_release"),
                library_basename("whitaker_suite"),
            ],
            "the archive should hold one plain library per requested crate",
        );
    }

    #[test]
    fn prebuilt_manifest_json_with_files_lists_every_file_and_hashes_the_archive() {
        let archive = b"archive bytes";
        let files = vec![
            library_basename("whitaker_suite"),
            library_basename("module_max_lines"),
        ];

        let manifest = prebuilt_manifest_json_with_files(
            "nightly-2026-05-28",
            "x86_64-unknown-linux-gnu",
            archive,
            &files,
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&manifest).expect("manifest should be valid JSON");
        let expected_digest = sha256_hex(archive);

        assert_eq!(
            parsed.get("files"),
            Some(&serde_json::json!(files)),
            "the manifest should list every archive file",
        );
        assert_eq!(
            parsed.get("sha256").and_then(serde_json::Value::as_str),
            Some(expected_digest.as_str()),
            "the manifest digest should match the archive bytes",
        );
    }
}

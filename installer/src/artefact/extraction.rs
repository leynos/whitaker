//! Archive extraction for prebuilt lint library artefacts.
//!
//! Extracts `.tar.zst` archives to a target directory with path
//! traversal protection to prevent zip-slip attacks.

use std::path::{Component, Path};

/// Trait for extracting artefact archives, enabling test mocking.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::extraction::ZstdExtractor;
///
/// let extractor = ZstdExtractor;
/// // Use extractor.extract(archive_path, dest_dir) in production
/// ```
#[cfg_attr(test, mockall::automock)]
pub trait ArtefactExtractor {
    /// Extract the archive at `archive_path` into `dest_dir`.
    ///
    /// Returns the list of filenames that were extracted.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionError::PathTraversal`] if any entry
    /// attempts to escape the destination directory.
    /// Returns [`ExtractionError::EmptyArchive`] if no files are found.
    /// Returns [`ExtractionError::Io`] on I/O failures.
    fn extract(&self, archive_path: &Path, dest_dir: &Path)
    -> Result<Vec<String>, ExtractionError>;
}

/// Errors arising from archive extraction.
#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    /// I/O error during extraction.
    #[error("extraction I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A path in the archive attempts to traverse outside the destination.
    #[error("path traversal detected: {path}")]
    PathTraversal {
        /// The offending path from the archive entry.
        path: String,
    },

    /// The archive contains no files.
    #[error("archive contains no library files")]
    EmptyArchive,
}

/// Default extractor using `tar` and `zstd` crates.
///
/// Validates each entry path before extraction to guard against
/// path traversal attacks (zip-slip).
pub struct ZstdExtractor;

impl ArtefactExtractor for ZstdExtractor {
    fn extract(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
    ) -> Result<Vec<String>, ExtractionError> {
        let file = std::fs::File::open(archive_path)?;
        let decoder = zstd::Decoder::new(file)?;
        let mut archive = tar::Archive::new(decoder);
        let mut extracted = Vec::new();

        for entry_result in archive.entries()? {
            let mut entry = entry_result?;
            let entry_path = entry.path()?.into_owned();

            validate_entry_path(&entry_path)?;

            let dest_path = dest_dir.join(&entry_path);
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            entry.unpack(&dest_path)?;

            if let Some(name) = entry_path.file_name() {
                extracted.push(name.to_string_lossy().into_owned());
            }
        }

        if extracted.is_empty() {
            return Err(ExtractionError::EmptyArchive);
        }

        Ok(extracted)
    }
}

/// Validate that a tar entry path does not escape the destination
/// directory via `..` components or absolute paths.
fn validate_entry_path(path: &Path) -> Result<(), ExtractionError> {
    if path.is_absolute() {
        return Err(ExtractionError::PathTraversal {
            path: path.display().to_string(),
        });
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(ExtractionError::PathTraversal {
                path: path.display().to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::path::PathBuf;

    #[test]
    fn extract_real_archive() {
        // Create a temp archive with a single file, then extract it.
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let archive_path = temp_dir.path().join("test.tar.zst");
        let dest_dir = temp_dir.path().join("out");
        std::fs::create_dir_all(&dest_dir).expect("create dest");

        // Create a file to archive.
        let source_file = temp_dir.path().join("hello.txt");
        std::fs::write(&source_file, b"hello world").expect("write source");

        // Build a .tar.zst archive. Explicitly finish both the tar
        // builder and the zstd encoder to ensure the frame is complete.
        let output_file = std::fs::File::create(&archive_path).expect("create archive");
        let encoder = zstd::Encoder::new(output_file, 0).expect("zstd encoder");
        let mut builder = tar::Builder::new(encoder);
        builder
            .append_path_with_name(&source_file, "hello.txt")
            .expect("append");
        let encoder = builder.into_inner().expect("tar finish");
        encoder.finish().expect("zstd finish");

        let extractor = ZstdExtractor;
        let files = extractor
            .extract(&archive_path, &dest_dir)
            .expect("extract");
        assert_eq!(files, vec!["hello.txt"]);
        assert!(dest_dir.join("hello.txt").exists());
    }

    #[rstest]
    #[case::parent_dir("../escape.txt")]
    #[case::nested_parent("foo/../../escape.txt")]
    fn rejects_path_traversal(#[case] bad_path: &str) {
        let path = PathBuf::from(bad_path);
        let result = validate_entry_path(&path);
        assert!(
            matches!(result, Err(ExtractionError::PathTraversal { .. })),
            "expected PathTraversal for {bad_path}"
        );
    }

    #[test]
    fn accepts_normal_paths() {
        let path = PathBuf::from("lib/libfoo.so");
        assert!(validate_entry_path(&path).is_ok());
    }

    #[test]
    fn rejects_absolute_path() {
        let path = PathBuf::from("/etc/passwd");
        let result = validate_entry_path(&path);
        assert!(matches!(result, Err(ExtractionError::PathTraversal { .. })));
    }

    #[test]
    fn extract_empty_archive() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let archive_path = temp_dir.path().join("empty.tar.zst");
        let dest_dir = temp_dir.path().join("out");
        std::fs::create_dir_all(&dest_dir).expect("create dest");

        // Build an empty archive. Explicitly finish both layers.
        let output_file = std::fs::File::create(&archive_path).expect("create");
        let encoder = zstd::Encoder::new(output_file, 0).expect("zstd");
        let builder = tar::Builder::new(encoder);
        let encoder = builder.into_inner().expect("tar finish");
        encoder.finish().expect("zstd finish");

        let extractor = ZstdExtractor;
        let result = extractor.extract(&archive_path, &dest_dir);
        assert!(matches!(result, Err(ExtractionError::EmptyArchive)));
    }
}

//! Pruning of staged prebuilt libraries.
//!
//! A prebuilt archive holds the suite and every constituent crate. Only what
//! was requested should stay beside the suite, or rustc rejects the duplicate
//! lint registration (issue #565).

use std::{collections::HashSet, fs, io};

use camino::Utf8Path;
use whitaker_installer::{
    builder::{library_extension, library_prefix},
    crate_name::CrateName,
    error::{InstallerError, Result},
    resolution::SUITE_CRATE,
};

pub(super) fn requested_crate_names(requested_crates: &[CrateName]) -> HashSet<&str> {
    if requested_crates.is_empty() {
        return HashSet::from([SUITE_CRATE]);
    }
    requested_crates.iter().map(CrateName::as_str).collect()
}

pub(super) fn staged_library_filename(crate_name: &str, toolchain_channel: &str) -> String {
    let normalized = crate_name.replace('-', "_");
    format!(
        "{}{}@{}{}",
        library_prefix(),
        normalized,
        toolchain_channel,
        library_extension()
    )
}

/// A directory of staged libraries for one toolchain.
struct Staging<'a> {
    path: &'a Utf8Path,
    toolchain: &'a str,
}

impl Staging<'_> {
    /// Return the crate behind a staged library file name for this toolchain.
    ///
    /// A staged name is `<prefix><crate>@<toolchain><extension>`, as built by
    /// [`staged_library_filename`]. Anything else, including a library staged
    /// for another toolchain, yields `None`.
    fn crate_of<'f>(&self, file_name: &'f str) -> Option<&'f str> {
        let suffix = format!("@{}{}", self.toolchain, library_extension());
        file_name
            .strip_prefix(library_prefix())?
            .strip_suffix(suffix.as_str())
            .filter(|name| !name.is_empty())
    }

    /// Remove every library staged for this toolchain whose crate was not requested.
    fn remove_unrequested(&self, requested_crates: &[CrateName]) -> Result<()> {
        let requested: HashSet<String> = requested_crate_names(requested_crates)
            .into_iter()
            .map(|name| name.replace('-', "_"))
            .collect();
        let read_error = |error: io::Error| {
            staging_error(format!(
                "failed to read staged libraries in {}: {error}",
                self.path
            ))
        };

        for dir_entry in fs::read_dir(self.path.as_std_path()).map_err(read_error)? {
            let entry = dir_entry.map_err(read_error)?;
            let file_name = entry.file_name();
            let unrequested = file_name
                .to_str()
                .and_then(|name| self.crate_of(name))
                .is_some_and(|crate_name| !requested.contains(crate_name));
            if unrequested {
                remove_stale_file(&entry.path())?;
            }
        }
        Ok(())
    }

    /// Fail unless this archive delivered, and the directory holds, every requested library.
    fn require_delivered(
        &self,
        requested_crates: &[CrateName],
        delivered: &[String],
    ) -> Result<()> {
        let mut missing: Vec<&str> = requested_crate_names(requested_crates)
            .into_iter()
            .filter(|name| {
                let staged = staged_library_filename(name, self.toolchain);
                !delivered.contains(&staged) || !self.path.join(&staged).as_std_path().is_file()
            })
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        missing.sort_unstable();
        Err(staging_error(format!(
            "the prebuilt archive does not hold the requested librar{}: {}",
            if missing.len() == 1 { "y" } else { "ies" },
            missing.join(", ")
        )))
    }
}

/// Remove staged prebuilt libraries that were not requested for this install.
///
/// Prebuilt archives can contain both suite and constituent crates, while local
/// builds stage only requested crates. Pruning keeps prebuilt installs aligned
/// with local-build behaviour and avoids duplicate lint registration when the
/// wrapper runs `cargo dylint --all`.
///
/// The prune is driven by what is staged, not by a list of crates this
/// installer was compiled with. An archive from a newer Whitaker can hold a
/// crate this build has never heard of; if that crate is registered by the
/// suite, leaving its standalone library beside the suite makes rustc reject
/// the duplicate lint registration (issue #565). Every library staged for
/// `toolchain_channel` whose crate was not requested is therefore removed.
/// Files for another toolchain, and files that are not staged libraries, are
/// left alone.
///
/// After pruning, every requested crate must have been delivered by this
/// archive (`delivered`) and be staged. The directory is reused between
/// installs, so a library left by an earlier install must not satisfy the
/// check. A selection the archive cannot satisfy (a crate that is not in the rolling archive, such
/// as an experimental lint) is reported as an error, which the caller turns into
/// a source-build fallback or, where that is forbidden, a refusal, rather than
/// an install that succeeds without the lint.
pub(super) fn prune_prebuilt_libraries(
    staging_path: &Utf8Path,
    toolchain_channel: &str,
    requested_crates: &[CrateName],
    delivered: &[String],
) -> Result<()> {
    let staging = Staging {
        path: staging_path,
        toolchain: toolchain_channel,
    };
    staging.remove_unrequested(requested_crates)?;
    staging.require_delivered(requested_crates, delivered)
}

const fn staging_error(reason: String) -> InstallerError {
    InstallerError::StagingFailed { reason }
}

fn remove_stale_file(path: &std::path::Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(staging_error(format!(
            "failed to remove stale prebuilt library {}: {error}",
            path.display()
        ))),
        _ => Ok(()),
    }
}

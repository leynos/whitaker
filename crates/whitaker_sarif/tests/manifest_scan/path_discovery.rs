//! Locating a crate's manifest on disk.
//!
//! The guard's subject crates are not all laid out the same way, so discovery
//! has two sources and they are tried in a fixed order. The root manifest's
//! `[workspace.dependencies]` declaration is authoritative — it records the
//! `path` Cargo itself resolves — and the `<root>/crates/<crate>` directory
//! convention is the fallback for a crate the root does not declare.
//!
//! Both orders matter. Trying the convention first would fail to find
//! `whitaker-common`, which lives at `<root>/common`; a failed lookup returns
//! `None`, and a guard handed `None` reports the same clean result as a guard
//! that read the file and found nothing. The failure is silent, which is why
//! `architecture_boundary.rs` pins each branch against real crates.
//!
//! **The two sources can disagree.** When the root declares a path that holds
//! no manifest while the crate sits at its conventional sibling location,
//! neither answer may be acted on. Trusting the declaration reports a present
//! crate as absent, which is the dormancy the guard exists to avoid; falling
//! back to the convention accepts a root manifest that names the wrong path.
//! [`DiscoveryError::DeclaredPathMissing`] reports the conflict instead, so a
//! stale declaration fails the guard rather than quietly disabling it.
//!
//! **Discovery has three outcomes, not two.** A manifest can be present,
//! confirmed absent, or undecidable because the filesystem refused to answer.
//! The third used to be folded into the second: `is_file()` reports `false` for
//! a permission denial exactly as it does for a missing file, so a guard that
//! could not look reported the same clean bill of health as one that looked and
//! found nothing. `manifest_if_present` therefore returns a `Result`, and
//! `Ok(None)` means only that every candidate was *confirmed* absent. Discovery
//! that reads a manifest it has already located keeps the panicking
//! [`read_manifest`] contract, and that boundary is stated on
//! [`declared_path`].

use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::{parse_manifest, read_manifest, workspace_table_of};

/// Why discovery could not decide whether a manifest is present.
///
/// This type exists so that "I could not look" never masquerades as "I looked
/// and there was nothing there". Only the second fact may certify absence; the
/// first must fail the guard, which is what carrying it as an error makes the
/// caller do.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DiscoveryError {
    /// The filesystem would not say whether a manifest is present.
    #[error("cannot tell whether {path} is a manifest: {source}")]
    Unreadable {
        /// The path the filesystem refused to describe.
        path: Utf8PathBuf,
        /// The failure the filesystem reported.
        source: io::Error,
    },
    /// A path exists but holds no regular file, so no manifest can be read.
    #[error("{path} exists but is not a regular file")]
    NotAFile {
        /// The path that is not a regular file.
        path: Utf8PathBuf,
    },
    /// The root declares a path that is absent while the crate sits at its
    /// conventional sibling location.
    ///
    /// This is a disagreement between the two discovery sources, and neither
    /// answer may be acted on. Trusting the declaration reports the crate as
    /// absent, which lets the mapping-crate guard skip a manifest that is
    /// really there; falling back to the convention would silently accept a
    /// root manifest that names the wrong path. Reporting the conflict makes
    /// the stale declaration visible instead of dormant.
    #[error(
        "the root manifest declares {declared} for {crate_name}, but no manifest is there; the \
         crate is present at {sibling} instead, so the declaration is stale"
    )]
    DeclaredPathMissing {
        /// The crate whose declaration disagrees with the filesystem.
        crate_name: String,
        /// The path the root manifest declares, which holds no manifest.
        declared: Utf8PathBuf,
        /// The conventional sibling path, which does hold one.
        sibling: Utf8PathBuf,
    },
}

/// Locates a crate's manifest relative to this test crate.
///
/// The workspace lays most crates out as `<root>/crates/<crate>/Cargo.toml`, and
/// this test lives in `crates/whitaker_sarif`, so the current crate is found at
/// `CARGO_MANIFEST_DIR` and a sibling beside it.
///
/// That convention does not hold for `whitaker-common`, which lives at
/// `<root>/common` — a directory named after neither its package nor its
/// dependency key. So a declaration is consulted first: the root manifest's
/// `[workspace.dependencies] <crate_name> = { path = … }` records where a crate
/// actually is, and the convention is only the fallback for a crate the root
/// does not declare. Without the declaration `whitaker-common` resolves to
/// nothing, and a guard that cannot find a manifest reports a clean bill of
/// health for a file it never opened.
///
/// Returns `Ok(Some(path))` only for a regular file that is present,
/// `Ok(None)` only when every candidate was confirmed absent, and a
/// [`DiscoveryError`] when a candidate could not be interrogated or the two
/// sources disagreed. Returning a `Result` rather than panicking is what lets
/// the mapping-crate half of the guard stay dormant until ADR 0013's mapping
/// crate is created.
///
/// # Errors
///
/// Returns [`DiscoveryError::Unreadable`] when the filesystem refuses to
/// describe a candidate path, [`DiscoveryError::NotAFile`] when a candidate
/// exists but is not a regular file, and
/// [`DiscoveryError::DeclaredPathMissing`] when the root declares a path that
/// holds no manifest while the crate is present at its conventional sibling
/// location.
pub(crate) fn manifest_if_present(crate_name: &str) -> Result<Option<Utf8PathBuf>, DiscoveryError> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.file_name() == Some(crate_name) {
        // Not probed: this is the manifest the running test binary was built
        // from, so the build would have failed before the question arose.
        return Ok(Some(manifest_dir.join("Cargo.toml")));
    }

    // Computed once and reused by both branches: the convention is the whole
    // answer for an undeclared crate, and the cross-check for a declared one.
    let sibling = manifest_dir
        .parent()
        .map(|parent| parent.join(crate_name).join("Cargo.toml"));

    declared_path(crate_name)?.map_or_else(
        || probe_sibling(sibling.as_deref()),
        |declared| resolve_declared(crate_name, declared, sibling.as_deref()),
    )
}

/// Resolve a crate that the root manifest declares a path for.
///
/// A declaration naming a present manifest is authoritative and is returned
/// unchanged. When it does not, the conventional sibling is still consulted:
/// a crate present at its sibling location while the root points elsewhere is
/// a stale declaration, and neither source alone reports that honestly.
/// Trusting the declaration would report the crate absent and let the
/// mapping-crate guard skip a manifest that really is there; trusting the
/// sibling would hide the wrong path in the root manifest.
///
/// The sibling is consulted only after the declaration was *confirmed* absent,
/// so a refused lookup still surfaces as [`DiscoveryError::Unreadable`] rather
/// than being read as the decisive "no".
///
/// # Errors
///
/// Returns [`DiscoveryError::DeclaredPathMissing`] for the disagreement above,
/// and propagates any probe failure from either path.
pub(crate) fn resolve_declared(
    crate_name: &str,
    declared: Utf8PathBuf,
    conventional: Option<&Utf8Path>,
) -> Result<Option<Utf8PathBuf>, DiscoveryError> {
    if probe(&declared)? {
        return Ok(Some(declared));
    }
    let Some(sibling) = conventional else {
        return Ok(None);
    };
    if !probe(sibling)? {
        return Ok(None);
    }
    Err(DiscoveryError::DeclaredPathMissing {
        crate_name: crate_name.to_owned(),
        declared,
        sibling: sibling.to_owned(),
    })
}

/// Resolve a crate the root manifest declares no path for.
///
/// # Errors
///
/// Propagates a probe failure, which must not be read as a confirmed absence.
fn probe_sibling(sibling: Option<&Utf8Path>) -> Result<Option<Utf8PathBuf>, DiscoveryError> {
    let Some(candidate) = sibling else {
        return Ok(None);
    };
    Ok(probe(candidate)?.then(|| candidate.to_owned()))
}

/// Returns the manifest path the root manifest declares for `crate_name`.
///
/// The declared path is written relative to the root manifest — `path =
/// "common"` — so the root's own directory is prepended. Returns `Ok(None)`
/// when the root is unreachable or declares no path for the crate, which sends
/// the caller to the directory convention.
///
/// The read that follows a confirmed presence panics rather than returning an
/// error. That is deliberate: a root manifest that exists and cannot be read is
/// no longer an undecidable discovery, it is a layout this guard does not
/// understand, and [`read_manifest`] reports it with the path it tried.
///
/// # Errors
///
/// Returns [`DiscoveryError::Unreadable`] or [`DiscoveryError::NotAFile`] when
/// the root manifest itself cannot be interrogated. Those cases must not be
/// read as "the root declares nothing", which would silently discard the
/// authoritative source.
///
/// # Panics
///
/// Panics when the root manifest is present but cannot be read or parsed.
fn declared_path(crate_name: &str) -> Result<Option<Utf8PathBuf>, DiscoveryError> {
    let Some(root_dir) = root_manifest_dir()? else {
        return Ok(None);
    };
    let document = parse_manifest(&read_manifest(&root_dir.join("Cargo.toml")));
    let declared = document
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(|dependencies| dependencies.get(crate_name))
        .and_then(|entry| entry.get("path"))
        .and_then(toml::Value::as_str);

    // The closure parameter is named `relative` rather than `declared` because
    // the workspace denies `shadow_reuse`, and reusing the outer binding here
    // would read as if the two were the same value.
    Ok(declared.map(|relative| root_dir.join(relative).join("Cargo.toml")))
}

/// Returns the directory holding the root manifest, if one is reachable.
///
/// # Errors
///
/// Returns a [`DiscoveryError`] when the root manifest cannot be interrogated.
/// Reporting absence here would send every lookup down the directory
/// convention, which is the branch that cannot find `whitaker-common` — so an
/// unreadable root would silently disable the guard's strongest source.
fn root_manifest_dir() -> Result<Option<Utf8PathBuf>, DiscoveryError> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(root_dir) = manifest_dir.parent().and_then(Utf8Path::parent) else {
        return Ok(None);
    };
    let candidate = root_dir.join("Cargo.toml");
    Ok(probe(&candidate)?.then(|| root_dir.to_owned()))
}

/// Reports whether a regular file is present at `path`.
///
/// Returns `Ok(false)` only for a lookup the filesystem has *confirmed* found
/// nothing. Every other failure — a permission denial, an I/O error, a path
/// that exists but is a directory — comes back as an error, because "I could
/// not look" and "I looked and it was not there" are different facts and only
/// the second one may certify absence.
///
/// The two confirmations are `NotFound` and `NotADirectory`. The second is not
/// a failure dressed up as an absence: when `a/Cargo.toml` is a regular file
/// then `a/Cargo.toml/Cargo.toml` provably cannot exist, and the kernel saying
/// so is the same fact as a missing file.
///
/// # Errors
///
/// Returns [`DiscoveryError::Unreadable`] when the parent directory cannot be
/// opened or the entry cannot be described, and [`DiscoveryError::NotAFile`]
/// when the path exists but is not a regular file.
pub(crate) fn probe(path: &Utf8Path) -> Result<bool, DiscoveryError> {
    let Some(directory) = path.parent() else {
        return Err(unreadable(
            path,
            io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory"),
        ));
    };
    let Some(filename) = path.file_name() else {
        return Err(unreadable(
            path,
            io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"),
        ));
    };
    let handle = match Dir::open_ambient_dir(directory, ambient_authority()) {
        Ok(handle) => handle,
        // A parent that does not exist, or that is itself a regular file,
        // proves the child cannot exist: `a/b` is absent whenever `a` is. Both
        // are confirmations rather than refusals, and folding them into the
        // error arm would fail the convention fallback for every crate that is
        // simply not there.
        Err(source) if is_absence(&source) => return Ok(false),
        Err(source) => return Err(unreadable(directory, source)),
    };

    match handle.metadata(filename) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(DiscoveryError::NotAFile {
            path: path.to_owned(),
        }),
        Err(source) if is_absence(&source) => Ok(false),
        Err(source) => Err(unreadable(path, source)),
    }
}

/// Wraps a filesystem failure as an undecidable lookup at `path`.
fn unreadable(path: &Utf8Path, source: io::Error) -> DiscoveryError {
    DiscoveryError::Unreadable {
        path: path.to_owned(),
        source,
    }
}

/// Returns whether a failed lookup proves that nothing is at the path.
///
/// `NotFound` is the obvious case. `NotADirectory` is the same fact stated
/// about a different component: when `crate/Cargo.toml` is a regular file then
/// `crate/Cargo.toml/Cargo.toml` cannot exist, so the kernel's refusal is a
/// confirmation rather than a failure to look. Every other kind is a genuine
/// inability to answer.
fn is_absence(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

/// Returns the root workspace's `[workspace.dependencies]` table.
///
/// A member manifest may inherit a dependency — and its rename — through
/// `{ workspace = true }`, in which case the package name lives only in the
/// root manifest. Resolving that is what stops an inherited rename from being
/// an invisible edge.
///
/// Returns `Ok(None)` when no root is reachable, which is the correct answer
/// for a fixture and a fail-closed one for a real manifest: an inherited entry
/// whose declaration cannot be read is not treated as clean.
///
/// # Errors
///
/// Returns a [`DiscoveryError`] when the root manifest cannot be interrogated.
/// An unreadable root must not be reported as a root that declares nothing,
/// because every inherited entry would then look unresolvable for a reason that
/// is not the manifest's own shape.
///
/// # Panics
///
/// Panics when a root manifest is found but cannot be parsed, which means the
/// workspace this guard navigates has changed shape.
pub(crate) fn workspace_dependencies() -> Result<Option<toml::Table>, DiscoveryError> {
    let Some(root_dir) = root_manifest_dir()? else {
        return Ok(None);
    };
    let document = parse_manifest(&read_manifest(&root_dir.join("Cargo.toml")));
    Ok(workspace_table_of(&document))
}

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

use camino::{Utf8Path, Utf8PathBuf};

use super::{parse_manifest, read_manifest, workspace_table_of};

/// Locates a crate's manifest relative to this test crate, or reports absence.
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
/// Returning an `Option` rather than panicking is what lets the mapping-crate
/// half of the guard stay dormant until ADR 005's mapping crate is created.
pub(crate) fn manifest_if_present(crate_name: &str) -> Option<Utf8PathBuf> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.file_name() == Some(crate_name) {
        return Some(manifest_dir.join("Cargo.toml"));
    }

    if let Some(declared) = declared_path(crate_name) {
        return declared.is_file().then_some(declared);
    }

    let candidate = manifest_dir
        .parent()
        .map(|parent| parent.join(crate_name).join("Cargo.toml"))?;
    candidate.is_file().then_some(candidate)
}

/// Returns the manifest path the root manifest declares for `crate_name`.
///
/// The declared path is written relative to the root manifest — `path =
/// "common"` — so the root's own directory is prepended. Returns `None` when the
/// root is unreachable or declares no path for the crate, which sends the caller
/// to the directory convention.
#[must_use]
fn declared_path(crate_name: &str) -> Option<Utf8PathBuf> {
    let root_dir = root_manifest_dir()?;
    let document = parse_manifest(&read_manifest(&root_dir.join("Cargo.toml")));
    let declared = document
        .get("workspace")?
        .get("dependencies")?
        .get(crate_name)?
        .get("path")?
        .as_str()?;
    Some(root_dir.join(declared).join("Cargo.toml"))
}

/// Returns the directory holding the root manifest, if one is reachable.
#[must_use]
fn root_manifest_dir() -> Option<Utf8PathBuf> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_dir = manifest_dir.parent()?.parent()?;
    root_dir
        .join("Cargo.toml")
        .is_file()
        .then(|| root_dir.to_owned())
}

/// Returns the root workspace's `[workspace.dependencies]` table.
///
/// A member manifest may inherit a dependency — and its rename — through
/// `{ workspace = true }`, in which case the package name lives only in the
/// root manifest. Resolving that is what stops an inherited rename from being
/// an invisible edge.
///
/// Returns `None` when no root is reachable, which is the correct answer for a
/// fixture and a fail-closed one for a real manifest: an inherited entry whose
/// declaration cannot be read is not treated as clean.
///
/// # Panics
///
/// Panics when a root manifest is found but cannot be parsed, which means the
/// workspace this guard navigates has changed shape.
pub(crate) fn workspace_dependencies() -> Option<toml::Table> {
    let root = root_manifest_dir()?.join("Cargo.toml");
    let document = parse_manifest(&read_manifest(&root));
    workspace_table_of(&document)
}

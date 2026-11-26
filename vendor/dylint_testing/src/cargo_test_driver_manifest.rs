//! Parse and classify `--manifest-path` selectors against the prepared workspace.

use std::{
    ffi::{OsStr, OsString},
    fs,
    io,
    path::{Path, PathBuf},
};

/// One `--manifest-path` selector in a Cargo argument list.
#[derive(Debug)]
pub(super) struct ManifestSelector {
    /// Index of the argument that carries the flag.
    pub(super) position: usize,
    /// Number of arguments the selector occupies; two for the separate form.
    pub(super) length: usize,
    /// Manifest the selector names, absent when the flag has no value.
    pub(super) value: Option<OsString>,
}

/// Parse every `--manifest-path` selector in a Cargo argument list.
///
/// Cargo accepts the separate form (`--manifest-path <value>`) and the joined
/// form (`--manifest-path=<value>`). The value of a separate flag is consumed
/// with the flag, so a token that merely resembles a selector is never parsed
/// as one. Scanning stops at a bare `--`, because everything after it belongs
/// to the program Cargo runs rather than to Cargo. Metadata selection and
/// Nextest routing both read this parser, so their view of the grammar cannot
/// drift.
pub(super) fn manifest_selectors(arguments: &[OsString]) -> Vec<ManifestSelector> {
    let mut selectors = Vec::new();
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        if argument == OsStr::new("--") {
            break;
        }
        if argument == OsStr::new("--manifest-path") {
            let value = arguments.get(index + 1).cloned();
            let length = 1 + usize::from(value.is_some());
            selectors.push(ManifestSelector {
                position: index,
                length,
                value,
            });
            index += length;
            continue;
        }
        if let Some(manifest) = argument.to_string_lossy().strip_prefix("--manifest-path=") {
            selectors.push(ManifestSelector {
                position: index,
                length: 1,
                value: Some(OsString::from(manifest)),
            });
        }
        index += 1;
    }

    selectors
}

/// Confirm every manifest selector points at the current workspace.
///
/// A repeated flag is legal, and the prepared metadata describes only one
/// workspace, so a single selector naming another workspace must keep the
/// query away from the cache. A flag with no value is malformed and is treated
/// the same way.
pub(super) fn manifest_is_workspace_root(
    arguments: &[OsString],
    command_index: usize,
    workspace_root: &Path,
) -> bool {
    manifest_selectors(&arguments[command_index + 1..])
        .iter()
        .all(|selector| {
            selector
                .value
                .as_deref()
                .is_some_and(|manifest| manifest_path_is_workspace_root(manifest, workspace_root))
        })
}

/// How one manifest selector relates to the prepared workspace.
#[derive(Debug)]
pub(super) enum ManifestMatch {
    /// The selector names the prepared root manifest, and it can be read.
    Prepared,
    /// The selector names a file that is not the prepared root manifest.
    Foreign,
}

/// Check whether a manifest argument names the workspace's root manifest.
///
/// Both sides are canonicalized, so a checkout reached through a symlink, or a
/// manifest spelled with `..`, still matches. A manifest that cannot be
/// resolved — missing, or unreadable — is not compared lexically: the prepared
/// cache describes a file that does not exist, so the real Cargo must report
/// the problem instead of the driver answering from stale metadata. That is
/// also the answer Nextest routing needs, where keeping the selector is the
/// safe choice.
///
/// An unresolvable selector therefore answers `false` here, but the cause is
/// still reported by [`classify_manifest`] and can be inspected by a caller
/// that wants to distinguish a broken path from a foreign one.
pub(super) fn manifest_path_is_workspace_root(manifest: &OsStr, workspace_root: &Path) -> bool {
    matches!(
        classify_manifest(manifest, workspace_root, is_readable_file),
        Ok(ManifestMatch::Prepared)
    )
}

/// Classify a manifest selector against the prepared workspace root.
///
/// Resolution and readability are injected so each outcome can be tested
/// without permission bits, which a privileged test runner does not enforce.
/// Canonicalization proves only that a path resolves, so a manifest that
/// resolves but cannot be opened is reported as an error rather than as
/// prepared. The injected operation returns its own result, so the error keeps
/// its category instead of every failure becoming a permission problem.
///
/// # Errors
///
/// Return the I/O error that stopped the selector being resolved or read.
pub(super) fn classify_manifest(
    manifest: &OsStr,
    workspace_root: &Path,
    is_readable: impl Fn(&Path) -> io::Result<()>,
) -> io::Result<ManifestMatch> {
    let manifest_path = fs::canonicalize(resolve_manifest(manifest, workspace_root))?;
    let workspace_root = fs::canonicalize(workspace_root)?;

    if manifest_path.file_name() != Some(OsStr::new("Cargo.toml"))
        || manifest_path.parent() != Some(workspace_root.as_path())
    {
        return Ok(ManifestMatch::Foreign);
    }

    is_readable(&manifest_path)?;
    Ok(ManifestMatch::Prepared)
}

/// Open a file to prove it can be read, reporting why it cannot.
pub(super) fn is_readable_file(path: &Path) -> io::Result<()> { fs::File::open(path).map(drop) }

/// Resolve a manifest selector against the prepared workspace root.
fn resolve_manifest(manifest: &OsStr, workspace_root: &Path) -> PathBuf {
    let manifest_path = Path::new(manifest);
    if manifest_path.is_absolute() {
        manifest_path.to_path_buf()
    } else {
        workspace_root.join(manifest_path)
    }
}

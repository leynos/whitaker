//! Serve prepared workspace metadata and route Cargo's Nextest call directly.
//!
//! `cargo-llvm-cov` invokes Cargo with `--cargo-metadata` and a
//! `--manifest-path` that Nextest rejects. The driver answers a `--no-deps`
//! metadata query from the file `make fetch` prepared, and forwards every
//! other call unchanged. Routing drops a `--manifest-path` that names the
//! prepared workspace, because the prepared metadata already describes it.

use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io,
    path::{Path, PathBuf},
    process::Command,
};

/// The environment the driver reads, resolved by the caller.
///
/// Resolving the variables up front keeps the routing logic testable without
/// mutating the process environment.
#[derive(Debug, Default)]
pub struct DriverEnvironment {
    /// Workspace root the prepared metadata describes.
    pub metadata_root: Option<PathBuf>,
    /// File holding the prepared metadata.
    pub metadata_cache: Option<PathBuf>,
    /// Cargo executable that ordinary calls are forwarded to.
    pub cargo_executable: Option<OsString>,
    /// Standalone Nextest executable that `nextest run` is routed to.
    pub nextest_executable: Option<OsString>,
}

impl DriverEnvironment {
    /// Read the driver's variables from the process environment.
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            metadata_root: env::var_os("CARGO_METADATA_ROOT").map(PathBuf::from),
            metadata_cache: env::var_os("CARGO_METADATA_CACHE").map(PathBuf::from),
            cargo_executable: env::var_os("CARGO_REAL"),
            nextest_executable: env::var_os("CARGO_NEXTEST_REAL"),
        }
    }
}

/// Run the driver for the current process, returning the exit code to use.
///
/// # Errors
///
/// Return an I/O error if the working directory cannot be read, the metadata
/// cache cannot be read, or a child process cannot be started.
pub fn run_from_process() -> io::Result<i32> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let current_directory = env::current_dir()?;
    let environment = DriverEnvironment::from_process();
    let mut stdout = io::stdout().lock();
    run(&arguments, &current_directory, &environment, &mut stdout)
}

/// Serve cached metadata or run the routed child, returning its exit code.
///
/// # Errors
///
/// Return an I/O error naming the operation that failed when the metadata
/// cache cannot be read or a child process cannot be started. The named
/// operation is the only diagnostic for a failure that happens before a child
/// exists to inherit stderr from.
pub fn run(
    arguments: &[OsString],
    current_directory: &Path,
    environment: &DriverEnvironment,
    stdout: &mut impl io::Write,
) -> io::Result<i32> {
    if let (Some(root), Some(cache)) = (
        environment.metadata_root.as_deref(),
        environment.metadata_cache.as_deref(),
    ) && is_workspace_metadata_query(arguments, current_directory, root)?
    {
        let metadata = fs::read(cache)
            .map_err(|error| operation_error(&error, "reading the prepared metadata cache"))?;
        stdout.write_all(&metadata)?;
        return Ok(0);
    }

    let cargo_executable = environment.cargo_executable.as_deref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "CARGO_REAL must name the Cargo executable",
        )
    })?;
    let nextest_executable = environment
        .nextest_executable
        .as_deref()
        .unwrap_or_else(|| OsStr::new("cargo-nextest"));
    let prepared_root = prepared_workspace(current_directory, environment.metadata_root.as_deref());
    let status = routed_command(
        cargo_executable,
        nextest_executable,
        arguments,
        prepared_root,
    )
    .status()
    .map_err(|error| operation_error(&error, "starting the routed child"))?;
    Ok(status.code().unwrap_or(1))
}

/// Check whether the request is Cargo metadata for the prepared workspace.
///
/// # Errors
///
/// Return an I/O error if either directory cannot be canonicalized.
fn is_workspace_metadata_query(
    arguments: &[OsString],
    current_directory: &Path,
    workspace_root: &Path,
) -> io::Result<bool> {
    let command_index = cargo_command_index(arguments);
    if !is_no_deps_metadata_query(arguments, command_index) {
        return Ok(false);
    }

    let current_directory = fs::canonicalize(current_directory)?;
    let workspace_root = fs::canonicalize(workspace_root)?;
    if current_directory != workspace_root || !prepared_manifest_is_readable(&workspace_root) {
        return Ok(false);
    }

    Ok(manifest_is_workspace_root(
        arguments,
        command_index,
        &workspace_root,
    ))
}

/// Check that the prepared workspace still holds a readable root manifest.
///
/// Canonicalization proves a path resolves; it does not prove the file can be
/// opened. The prepared cache describes the workspace's root manifest, so
/// serving it while that manifest is missing or unreadable would answer a
/// question the real Cargo has to fail. A request with no `--manifest-path`
/// has no selector to classify, so this check is what keeps it away from the
/// cache.
fn prepared_manifest_is_readable(workspace_root: &Path) -> bool {
    // The answer here is deliberately a yes or no: an unreadable manifest sends
    // the request to the real Cargo, which reports the failure itself, so there
    // is no caller left to receive the error.
    is_readable_file(&workspace_root.join("Cargo.toml")).is_ok()
}

/// Check whether the request is a `cargo metadata --no-deps` query.
///
/// The prepared cache holds `--no-deps` output, which carries no dependency
/// packages and no resolve graph. Serving it to a full metadata query would
/// hand that caller a silently incomplete document, so only `--no-deps`
/// requests may read it; every other query reaches the real Cargo.
fn is_no_deps_metadata_query(arguments: &[OsString], command_index: usize) -> bool {
    arguments
        .get(command_index)
        .is_some_and(|argument| argument == OsStr::new("metadata"))
        && arguments
            .iter()
            .any(|argument| argument == OsStr::new("--no-deps"))
}

/// Locate the Cargo subcommand after an optional rustup toolchain selector.
fn cargo_command_index(arguments: &[OsString]) -> usize {
    usize::from(
        arguments
            .first()
            .is_some_and(|argument| argument.to_string_lossy().starts_with('+')),
    )
}

/// One `--manifest-path` selector in a Cargo argument list.
#[derive(Debug)]
struct ManifestSelector {
    /// Index of the argument that carries the flag.
    position: usize,
    /// Number of arguments the selector occupies; two for the separate form.
    length: usize,
    /// Manifest the selector names, absent when the flag has no value.
    value: Option<OsString>,
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
fn manifest_selectors(arguments: &[OsString]) -> Vec<ManifestSelector> {
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
fn manifest_is_workspace_root(
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
enum ManifestMatch {
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
fn manifest_path_is_workspace_root(manifest: &OsStr, workspace_root: &Path) -> bool {
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
fn classify_manifest(
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
fn is_readable_file(path: &Path) -> io::Result<()> { fs::File::open(path).map(drop) }

/// Name the operation that failed while preserving the error's category.
///
/// The driver's example prints this error to stderr. A failure before a child
/// exists has no inherited diagnostics to explain it, so the message names the
/// step; the kind survives so a missing file stays distinguishable from a
/// permission problem.
fn operation_error(error: &io::Error, operation: &str) -> io::Error {
    io::Error::new(error.kind(), format!("{operation}: {error}"))
}

/// Resolve a manifest selector against the prepared workspace root.
fn resolve_manifest(manifest: &OsStr, workspace_root: &Path) -> PathBuf {
    let manifest_path = Path::new(manifest);
    if manifest_path.is_absolute() {
        manifest_path.to_path_buf()
    } else {
        workspace_root.join(manifest_path)
    }
}

/// Return the prepared workspace root when the caller is standing in it.
///
/// A `--manifest-path` naming the prepared workspace may only be dropped when
/// the routed call would otherwise resolve that same workspace. Both sides are
/// canonicalized, so a checkout reached through a symlink, or a current
/// directory spelled with `..`, still matches. A path that cannot be resolved
/// answers `None`, which keeps the selector: a kept selector makes Nextest
/// describe the workspace the caller named, while a dropped one would silently
/// move the run to whatever manifest the current directory holds.
fn prepared_workspace<'a>(
    current_directory: &Path,
    workspace_root: Option<&'a Path>,
) -> Option<&'a Path> {
    let workspace_root = workspace_root?;
    let current_directory = fs::canonicalize(current_directory).ok()?;
    let canonical_root = fs::canonicalize(workspace_root).ok()?;
    (current_directory == canonical_root).then_some(workspace_root)
}

/// Route Cargo's Nextest subcommand to the standalone Nextest executable.
fn routed_command(
    cargo_executable: &OsStr,
    nextest_executable: &OsStr,
    arguments: &[OsString],
    workspace_root: Option<&Path>,
) -> Command {
    let command_index = cargo_command_index(arguments);
    if arguments
        .get(command_index)
        .is_some_and(|argument| argument == OsStr::new("nextest"))
        && arguments
            .get(command_index + 1)
            .is_some_and(|argument| argument == OsStr::new("run"))
    {
        let forwarded = without_prepared_manifest(&arguments[command_index + 2..], workspace_root);
        return nextest_command(nextest_executable, &forwarded);
    }

    cargo_command(cargo_executable, arguments)
}

/// Drop a `--manifest-path` naming the workspace the metadata describes.
///
/// Nextest refuses `--manifest-path` together with `--cargo-metadata`, and
/// `cargo llvm-cov` adds the flag when it invokes Cargo, so the routed call
/// must not forward that pair. A manifest naming any other workspace is kept,
/// because the prepared metadata would describe the wrong workspace. The
/// caller passes a root only when the current directory is that workspace, so
/// a run started elsewhere keeps its selector.
fn without_prepared_manifest(
    arguments: &[OsString],
    workspace_root: Option<&Path>,
) -> Vec<OsString> {
    let Some(workspace_root) = workspace_root else {
        return arguments.to_vec();
    };

    let mut forwarded = Vec::with_capacity(arguments.len());
    let mut start = 0;
    for selector in manifest_selectors(arguments) {
        forwarded.extend(arguments[start..selector.position].iter().cloned());
        start = selector.position + selector.length;
        let names_prepared = selector
            .value
            .as_deref()
            .is_some_and(|manifest| manifest_path_is_workspace_root(manifest, workspace_root));
        if !names_prepared {
            forwarded.extend(arguments[selector.position..start].iter().cloned());
        }
    }
    forwarded.extend(arguments[start..].iter().cloned());

    forwarded
}

/// Run the standalone Nextest executable without changing its Cargo flags.
fn nextest_command(nextest_executable: &OsStr, arguments: &[OsString]) -> Command {
    let mut command = Command::new(nextest_executable);
    command
        .arg("nextest")
        .arg("run")
        .args(arguments)
        .env_remove("CARGO_METADATA_CACHE")
        .env_remove("CARGO_METADATA_ROOT");
    command
}

/// Forward an ordinary Cargo command without adding policy flags.
fn cargo_command(cargo_executable: &OsStr, arguments: &[OsString]) -> Command {
    let mut command = Command::new(cargo_executable);
    command.args(arguments);
    command
}

#[cfg(test)]
#[path = "cargo_test_driver_tests.rs"]
mod tests;

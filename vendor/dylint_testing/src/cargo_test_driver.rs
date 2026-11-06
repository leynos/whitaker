//! Serve prepared workspace metadata and route Cargo's Nextest call directly.
//!
//! `cargo-llvm-cov` invokes Cargo with `--cargo-metadata` and a
//! `--manifest-path` that Nextest rejects. The driver answers the metadata
//! query from the file `make fetch` prepared, and forwards every other call
//! unchanged. Routing drops a `--manifest-path` that names the prepared
//! workspace, because the prepared metadata already describes it.

use std::{
    env,
    ffi::{OsStr, OsString},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};


//! Serve prepared workspace metadata and route Cargo's Nextest call directly.
//!
//! `cargo-llvm-cov` invokes Cargo with `--cargo-metadata` and a
//! `--manifest-path` that Nextest rejects. The driver answers a `--no-deps`
//! metadata query from the file `make fetch` prepared, and forwards every
//! other call unchanged. Routing drops a `--manifest-path` that names the
//! prepared workspace, because the prepared metadata already describes it.
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
/// Return an I/O error if the metadata cache cannot be read or a child process
/// cannot be started.
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
        stdout.write_all(&fs::read(cache)?)?;
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
    let status = routed_command(
        cargo_executable,
        nextest_executable,
        arguments,
        environment.metadata_root.as_deref(),
    )
    .status()?;
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
    fs::File::open(workspace_root.join("Cargo.toml")).is_ok()
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
/// as one. Metadata selection and Nextest routing both read this parser, so
/// their view of the grammar cannot drift.
fn manifest_selectors(arguments: &[OsString]) -> Vec<ManifestSelector> {
    let mut selectors = Vec::new();
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
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

/// Check whether a manifest argument names the workspace's root manifest.
///
/// Both sides are canonicalized, so a checkout reached through a symlink, or a
/// manifest spelled with `..`, still matches. A manifest that cannot be
/// resolved — missing, or unreadable — is classified as foreign rather than
/// compared lexically: the prepared cache describes a file that does not
/// exist, so the real Cargo must report the problem instead of the driver
/// answering from stale metadata. Both failure modes point the same way that
/// Nextest routing needs, where keeping the selector is also the safe choice.
fn manifest_path_is_workspace_root(manifest: &OsStr, workspace_root: &Path) -> bool {
    let Ok(manifest_path) = fs::canonicalize(resolve_manifest(manifest, workspace_root)) else {
        return false;
    };
    let Ok(workspace_root) = fs::canonicalize(workspace_root) else {
        return false;
    };

    manifest_path.file_name() == Some(OsStr::new("Cargo.toml"))
        && manifest_path.parent() == Some(workspace_root.as_path())
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
/// because the prepared metadata would describe the wrong workspace.
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

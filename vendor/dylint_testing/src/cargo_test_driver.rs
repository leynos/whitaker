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
    let is_metadata = arguments
        .get(command_index)
        .is_some_and(|argument| argument == OsStr::new("metadata"));
    if !is_metadata {
        return Ok(false);
    }

    let current_directory = fs::canonicalize(current_directory)?;
    let workspace_root = fs::canonicalize(workspace_root)?;
    if current_directory != workspace_root {
        return Ok(false);
    }

    Ok(manifest_is_workspace_root(
        arguments,
        command_index,
        &workspace_root,
    ))
}

/// Locate the Cargo subcommand after an optional rustup toolchain selector.
fn cargo_command_index(arguments: &[OsString]) -> usize {
    usize::from(
        arguments
            .first()
            .is_some_and(|argument| argument.to_string_lossy().starts_with('+')),
    )
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
    let mut arguments = arguments[command_index + 1..].iter();
    while let Some(argument) = arguments.next() {
        if argument == OsStr::new("--manifest-path") {
            let Some(manifest) = arguments.next() else {
                return false;
            };
            if !manifest_path_is_workspace_root(manifest, workspace_root) {
                return false;
            }
        } else if let Some(manifest) = argument.to_string_lossy().strip_prefix("--manifest-path=")
            && !manifest_path_is_workspace_root(OsStr::new(manifest), workspace_root)
        {
            return false;
        }
    }

    true
}

/// Check whether a manifest argument names the workspace's root manifest.
fn manifest_path_is_workspace_root(manifest: &OsStr, workspace_root: &Path) -> bool {
    let manifest_path = Path::new(manifest);
    let manifest_path = if manifest_path.is_absolute() {
        manifest_path.to_path_buf()
    } else {
        workspace_root.join(manifest_path)
    };

    manifest_path.file_name() == Some(OsStr::new("Cargo.toml"))
        && manifest_path.parent() == Some(workspace_root)
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
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        if argument == OsStr::new("--manifest-path") {
            match arguments.next() {
                Some(manifest) if manifest_path_is_workspace_root(manifest, workspace_root) => {}
                Some(manifest) => {
                    forwarded.push(argument.clone());
                    forwarded.push(manifest.clone());
                }
                None => forwarded.push(argument.clone()),
            }
            continue;
        }
        if let Some(manifest) = argument.to_string_lossy().strip_prefix("--manifest-path=")
            && manifest_path_is_workspace_root(OsStr::new(manifest), workspace_root)
        {
            continue;
        }
        forwarded.push(argument.clone());
    }

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
mod tests {
    //! Check metadata selection and command routing without spawning tools.

    use super::{is_workspace_metadata_query, routed_command};
    use std::{
        ffi::{OsStr, OsString},
        path::Path,
    };

    /// Require workspace metadata requests to match the prepared root.
    #[test]
    fn uses_cached_metadata_only_for_workspace_queries() {
        let arguments = ["metadata", "--no-deps"].map(OsString::from);

        let is_workspace_query = is_workspace_metadata_query(
            &arguments,
            Path::new(env!("CARGO_MANIFEST_DIR")),
            Path::new(env!("CARGO_MANIFEST_DIR")),
        )
        .expect("workspace paths should resolve");
        let is_fixture_query = is_workspace_metadata_query(
            &arguments,
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples"),
            Path::new(env!("CARGO_MANIFEST_DIR")),
        )
        .expect("workspace path should resolve");

        assert!(is_workspace_query);
        assert!(!is_fixture_query);
    }

    /// Recognise the metadata query behind a rustup toolchain selector.
    #[test]
    fn uses_cached_metadata_behind_a_toolchain_selector() {
        let arguments = ["+nightly", "metadata"].map(OsString::from);

        let is_workspace_query = is_workspace_metadata_query(
            &arguments,
            Path::new(env!("CARGO_MANIFEST_DIR")),
            Path::new(env!("CARGO_MANIFEST_DIR")),
        )
        .expect("workspace paths should resolve");

        assert!(is_workspace_query);
    }

    /// Keep the driver's own variables out of the routed Nextest child.
    #[test]
    fn removes_the_driver_variables_from_the_nextest_child() {
        let arguments = ["nextest", "run"].map(OsString::from);
        let command = routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            None,
        );
        let removed: Vec<_> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_owned())
            .collect();

        assert!(
            removed.contains(&OsString::from("CARGO_METADATA_CACHE")),
            "the child must not inherit the metadata cache: {removed:?}"
        );
        assert!(
            removed.contains(&OsString::from("CARGO_METADATA_ROOT")),
            "the child must not inherit the metadata root: {removed:?}"
        );
    }

    /// Preserve Cargo flags when routing its Nextest subcommand.
    #[test]
    fn routes_nextest_without_adding_frozen_mode() {
        let arguments = ["nextest", "run", "--locked", "--workspace"].map(OsString::from);
        let command = routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            None,
        );
        let actual: Vec<_> = command.get_args().collect();

        assert_eq!(command.get_program(), OsStr::new("cargo-nextest"));
        assert_eq!(actual, ["nextest", "run", "--locked", "--workspace"]);
    }

    /// Drop the manifest flag Cargo's coverage driver adds for the workspace.
    #[test]
    fn drops_the_prepared_workspace_manifest_from_the_nextest_call() {
        let arguments = [
            "nextest",
            "run",
            "--manifest-path",
            "/workspace/Cargo.toml",
            "--target-dir",
            "/workspace/target/llvm-cov-target",
            "--cargo-metadata",
            "/workspace/target/metadata.json",
        ]
        .map(OsString::from);
        let command = routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            Some(Path::new("/workspace")),
        );
        let actual: Vec<_> = command.get_args().collect();

        assert_eq!(
            actual,
            [
                "nextest",
                "run",
                "--target-dir",
                "/workspace/target/llvm-cov-target",
                "--cargo-metadata",
                "/workspace/target/metadata.json",
            ]
        );
    }

    /// Drop the joined form of the prepared workspace manifest flag.
    #[test]
    fn drops_the_joined_prepared_workspace_manifest() {
        let arguments =
            ["nextest", "run", "--manifest-path=/workspace/Cargo.toml"].map(OsString::from);
        let command = routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            Some(Path::new("/workspace")),
        );
        let actual: Vec<_> = command.get_args().collect();

        assert_eq!(actual, ["nextest", "run"]);
    }

    /// Keep a manifest that describes some other workspace.
    #[test]
    fn keeps_a_manifest_for_another_workspace() {
        let arguments =
            ["nextest", "run", "--manifest-path", "/other/Cargo.toml"].map(OsString::from);
        let command = routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            Some(Path::new("/workspace")),
        );
        let actual: Vec<_> = command.get_args().collect();

        assert_eq!(
            actual,
            ["nextest", "run", "--manifest-path", "/other/Cargo.toml"]
        );
    }

    /// Drop every repeated selector that names the prepared workspace.
    #[test]
    fn drops_every_repeated_prepared_workspace_manifest() {
        let arguments = [
            "nextest",
            "run",
            "--manifest-path",
            "/workspace/Cargo.toml",
            "--manifest-path=/workspace/Cargo.toml",
        ]
        .map(OsString::from);
        let command = routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            Some(Path::new("/workspace")),
        );
        let actual: Vec<_> = command.get_args().collect();

        assert_eq!(actual, ["nextest", "run"]);
    }

    /// Keep a repeated selector when any occurrence names another workspace.
    #[test]
    fn keeps_a_repeated_manifest_for_another_workspace() {
        let arguments = [
            "nextest",
            "run",
            "--manifest-path",
            "/workspace/Cargo.toml",
            "--manifest-path",
            "/other/Cargo.toml",
        ]
        .map(OsString::from);
        let command = routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            Some(Path::new("/workspace")),
        );
        let actual: Vec<_> = command.get_args().collect();

        assert_eq!(
            actual,
            ["nextest", "run", "--manifest-path", "/other/Cargo.toml"]
        );
    }

    /// Keep a malformed selector rather than silently repairing it.
    #[test]
    fn keeps_a_dangling_manifest_flag() {
        let arguments = ["nextest", "run", "--manifest-path"].map(OsString::from);
        let command = routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            Some(Path::new("/workspace")),
        );
        let actual: Vec<_> = command.get_args().collect();

        assert_eq!(actual, ["nextest", "run", "--manifest-path"]);
    }

    /// Refuse the cache when a repeated selector names another workspace.
    #[test]
    fn refuses_cached_metadata_for_a_repeated_foreign_manifest() {
        let arguments = [
            "metadata",
            "--manifest-path",
            "Cargo.toml",
            "--manifest-path",
            "/other/Cargo.toml",
        ]
        .map(OsString::from);

        let is_workspace_query = is_workspace_metadata_query(
            &arguments,
            Path::new(env!("CARGO_MANIFEST_DIR")),
            Path::new(env!("CARGO_MANIFEST_DIR")),
        )
        .expect("workspace paths should resolve");

        assert!(!is_workspace_query);
    }

    /// Refuse the cache when a selector has no value.
    #[test]
    fn refuses_cached_metadata_for_a_dangling_manifest_flag() {
        let arguments = ["metadata", "--manifest-path"].map(OsString::from);

        let is_workspace_query = is_workspace_metadata_query(
            &arguments,
            Path::new(env!("CARGO_MANIFEST_DIR")),
            Path::new(env!("CARGO_MANIFEST_DIR")),
        )
        .expect("workspace paths should resolve");

        assert!(!is_workspace_query);
    }

    /// Pass ordinary Cargo calls through without forcing a lockfile policy.
    #[test]
    fn preserves_offline_trybuild_arguments() {
        let arguments = ["build", "--offline"].map(OsString::from);
        let command = routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            None,
        );
        let actual: Vec<_> = command.get_args().collect();

        assert_eq!(command.get_program(), OsStr::new("cargo"));
        assert_eq!(actual, ["build", "--offline"]);
    }
}

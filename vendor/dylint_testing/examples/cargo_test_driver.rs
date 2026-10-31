//! Serve prepared workspace metadata and route Cargo's Nextest call directly.
//!
//! Routing drops a `--manifest-path` that names the prepared workspace, because
//! Nextest rejects that flag alongside `--cargo-metadata`, and the prepared
//! metadata already describes the same workspace.

use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    process::{self, Command},
};

/// Route Cargo requests made by the coverage test driver.
fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "{error}");
        process::exit(1);
    }
}

/// Serve cached metadata or invoke Cargo or Nextest as requested.
///
/// # Errors
///
/// Return an I/O error if the metadata cache cannot be read or a child process
/// cannot be started.
fn run() -> io::Result<()> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let current_directory = env::current_dir()?;
    let metadata_root = env::var_os("CARGO_METADATA_ROOT").map(PathBuf::from);
    let metadata_cache = env::var_os("CARGO_METADATA_CACHE").map(PathBuf::from);

    if let (Some(root), Some(cache)) = (metadata_root.as_deref(), metadata_cache.as_deref())
        && is_workspace_metadata_query(&arguments, &current_directory, root)?
    {
        io::stdout().lock().write_all(&fs::read(cache)?)?;
        return Ok(());
    }

    let cargo_executable = env::var_os("CARGO_REAL").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "CARGO_REAL must name the Cargo executable",
        )
    })?;
    let nextest_executable =
        env::var_os("CARGO_NEXTEST_REAL").unwrap_or_else(|| OsString::from("cargo-nextest"));
    let status = routed_command(
        &cargo_executable,
        &nextest_executable,
        &arguments,
        metadata_root.as_deref(),
    )
    .status()?;
    process::exit(status.code().unwrap_or(1));
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

/// Confirm an optional manifest selector points at the current workspace.
fn manifest_is_workspace_root(
    arguments: &[OsString],
    command_index: usize,
    workspace_root: &Path,
) -> bool {
    let mut arguments = arguments[command_index + 1..].iter();
    while let Some(argument) = arguments.next() {
        if argument == OsStr::new("--manifest-path") {
            return arguments
                .next()
                .is_some_and(|manifest| manifest_path_is_workspace_root(manifest, workspace_root));
        }
        if let Some(manifest) = argument.to_string_lossy().strip_prefix("--manifest-path=") {
            return manifest_path_is_workspace_root(OsStr::new(manifest), workspace_root);
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

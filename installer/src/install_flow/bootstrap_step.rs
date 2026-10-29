//! The install-flow steps that provision and record the Dylint bootstrap.
//!
//! Installation installs the verified Clippy snapshot right after the Dylint
//! tools, before any workspace or library work, so a release that does not
//! publish its snapshot fails before minutes of other work. Once the lint
//! libraries are staged, `bootstrap.json` records what `whitaker-installer
//! run` should use. Neither source-policy mode permits a fallback here: the
//! snapshot comes from the release or from `--clippy-snapshot-bundle`, or
//! installation stops.

use camino::{Utf8Path, Utf8PathBuf};
use std::ffi::OsString;
use std::io::Write;
use whitaker_installer::bootstrap::{
    BOOTSTRAP_SCHEMA, BootstrapLayout, BootstrapMetadata, StoredPath,
};
use whitaker_installer::cli::InstallArgs;
use whitaker_installer::clippy_snapshot::{
    BundleSource, ClippySnapshotManifest, embedded_manifest, generation_path, provision,
};
use whitaker_installer::dirs::{BaseDirs, SystemBaseDirs};
use whitaker_installer::error::{InstallerError, Result};
use whitaker_installer::output::{ShellSnippet, write_stderr_line};
#[cfg(debug_assertions)]
use whitaker_installer::test_support::TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV;
use whitaker_installer::wrapper::{generate_wrapper_scripts, path_instructions};

/// The snapshot generation this installation provisioned.
#[derive(Debug, Clone)]
pub(crate) struct InstalledSnapshot {
    layout: BootstrapLayout,
    manifest: &'static ClippySnapshotManifest,
    generation: Utf8PathBuf,
}

impl InstalledSnapshot {
    /// The generation exported as `DYLINT_CLIPPY_REPO`.
    pub(crate) fn generation(&self) -> &Utf8Path {
        &self.generation
    }

    /// The driver slot exported as `DYLINT_DRIVER_PATH`.
    pub(crate) fn driver_dir(&self) -> Utf8PathBuf {
        self.layout
            .driver_dir(&self.manifest.dylint_version, self.manifest.commit.as_str())
    }
}

/// Return the manifest this installation pins.
///
/// Debug builds honour [`TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV`] so integration
/// tests can install a small fixture snapshot; release builds always use the
/// manifest compiled into the binary.
fn snapshot_manifest() -> Result<&'static ClippySnapshotManifest> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os(TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV) {
        let contents = std::fs::read_to_string(&path)?;
        let manifest = whitaker_installer::clippy_snapshot::parse_manifest(&contents)?;
        // One manifest per process, read once, like the embedded one.
        return Ok(Box::leak(Box::new(manifest)));
    }
    Ok(embedded_manifest()?)
}

/// Resolve the bootstrap layout for this platform.
fn layout(dirs: &dyn BaseDirs) -> Result<BootstrapLayout> {
    BootstrapLayout::from_dirs(dirs).map_err(InstallerError::from)
}

/// Install the pinned Clippy snapshot, reusing a generation that verifies.
///
/// # Errors
///
/// Returns [`InstallerError::ClippySnapshot`] when the snapshot cannot be
/// downloaded, verified, or installed.
pub(crate) fn install_clippy_snapshot(
    args: &InstallArgs,
    dirs: &dyn BaseDirs,
    stderr: &mut dyn Write,
) -> Result<InstalledSnapshot> {
    let manifest = snapshot_manifest()?;
    let layout = layout(dirs)?;
    let source = args
        .clippy_snapshot_bundle
        .clone()
        .map_or(BundleSource::Release, BundleSource::Local);
    if !args.quiet {
        write_stderr_line(
            stderr,
            format!("Installing Clippy snapshot {}...", manifest.commit),
        );
    }
    let generation = provision(&source, manifest, &layout.clippy_root())?;
    Ok(InstalledSnapshot {
        layout,
        manifest,
        generation,
    })
}

/// Record the installation for `whitaker-installer run`.
///
/// Written after the libraries are staged and before any wrapper exists, so
/// a wrapper never points at a half-recorded installation.
///
/// # Errors
///
/// Returns [`InstallerError::Bootstrap`] when the record cannot be written.
pub(crate) fn record_bootstrap(
    snapshot: &InstalledSnapshot,
    toolchain: &str,
    library_dir: &Utf8Path,
) -> Result<()> {
    let metadata = BootstrapMetadata {
        schema_version: BOOTSTRAP_SCHEMA,
        dylint_version: snapshot.manifest.dylint_version.clone(),
        snapshot_commit: snapshot.manifest.commit.to_string(),
        toolchain: toolchain.to_owned(),
        library_dir: StoredPath::new(library_dir, snapshot.layout.data_root()),
    };
    metadata.write(&snapshot.layout.metadata_path())?;
    Ok(())
}

/// Report where the bootstrap lives, for `--dry-run`, without changing it.
///
/// The labels are a stable interface: hosted checks read these lines to find
/// the generation and driver directory on any platform.
///
/// # Errors
///
/// Returns an error when the embedded manifest or the layout is unusable.
pub(crate) fn report_bootstrap_plan(dirs: &dyn BaseDirs, stderr: &mut dyn Write) -> Result<()> {
    let manifest = snapshot_manifest()?;
    let layout = layout(dirs)?;
    let generation = generation_path(&layout.clippy_root(), manifest);
    let state = if generation.is_dir() {
        "installed"
    } else {
        "to be installed"
    };
    write_stderr_line(stderr, "");
    write_stderr_line(stderr, format!("Clippy snapshot: {generation}"));
    write_stderr_line(stderr, format!("Clippy snapshot state: {state}"));
    write_stderr_line(
        stderr,
        format!(
            "Dylint driver directory: {}",
            layout.driver_dir(&manifest.dylint_version, manifest.commit.as_str())
        ),
    );
    Ok(())
}

/// Run `cargo dylint` through the installed bootstrap; return the exit code.
pub(crate) fn run_bootstrap(args: &[OsString], stderr: &mut dyn Write) -> i32 {
    let Some(dirs) = SystemBaseDirs::new() else {
        write_stderr_line(stderr, "whitaker: could not determine platform directories");
        return 1;
    };
    match whitaker_installer::bootstrap::run(&dirs, args) {
        Ok(code) => code,
        Err(error) => {
            write_stderr_line(stderr, format!("whitaker: {error}"));
            1
        }
    }
}

/// Print the shell configuration for `--skip-wrapper` installs.
///
/// Without the wrappers nothing serializes cold driver builds, so the snippet
/// says so rather than leaving concurrent first runs to race.
pub(crate) fn write_shell_snippet(
    snapshot: &InstalledSnapshot,
    library_dir: &Utf8Path,
    stderr: &mut dyn Write,
) {
    let snippet =
        ShellSnippet::for_bootstrap(library_dir, snapshot.generation(), &snapshot.driver_dir());
    write_stderr_line(stderr, "");
    write_stderr_line(stderr, snippet.display_text());
    write_stderr_line(stderr, "");
    write_stderr_line(
        stderr,
        concat!(
            "Only the whitaker wrapper serializes the first driver build; with ",
            "these variables, let one cargo dylint run finish before starting others.",
        ),
    );
}

/// Generate the wrapper scripts and report how to run them.
///
/// # Errors
///
/// Returns [`InstallerError::WrapperGeneration`] when a script cannot be
/// written, and [`InstallerError::StagingFailed`] when its path has no parent.
pub(crate) fn write_wrappers(dirs: &dyn BaseDirs, stderr: &mut dyn Write) -> Result<()> {
    let result = generate_wrapper_scripts(dirs)?;
    write_stderr_line(stderr, "");
    write_stderr_line(stderr, "Wrapper scripts created:");
    write_stderr_line(stderr, format!("  - {}", result.whitaker_path.display()));
    write_stderr_line(stderr, format!("  - {}", result.whitaker_ls_path.display()));
    write_stderr_line(stderr, "");

    if result.in_path {
        write_stderr_line(stderr, "You can now run:");
    } else {
        let bin_dir =
            result
                .whitaker_path
                .parent()
                .ok_or_else(|| InstallerError::StagingFailed {
                    reason: "wrapper script path has no parent directory".to_owned(),
                })?;
        write_stderr_line(stderr, path_instructions(bin_dir));
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, "Then run:");
    }
    write_stderr_line(stderr, "  whitaker --all");
    write_stderr_line(stderr, "  whitaker-ls");
    Ok(())
}

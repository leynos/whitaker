//! Build, install, and verify the Clippy snapshot Whitaker publishes.
//!
//! `build` is the producer's only route to public Clippy. `install` is how the
//! producer's own tests and CI provision the same verified generation the
//! installer gives consumers, and `verify` re-runs the full check on a
//! repository. All three share `whitaker_installer::clippy_snapshot`.

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use std::io::Write;
use whitaker_installer::clippy_snapshot::{
    BuildRequest, BundleSource, ClippySnapshotError, build_bundle, embedded_manifest,
    generation_path, provision, verify_repository,
};

/// Manage the published Clippy snapshot.
#[derive(Parser, Debug)]
#[command(name = "whitaker-clippy-snapshot", version)]
#[command(about = "Build, install, and verify Whitaker's Clippy snapshot")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Supported snapshot subcommands.
#[derive(Subcommand, Debug)]
enum Command {
    /// Clone upstream Clippy and write the bundle and its sidecar.
    Build {
        /// Directory the bundle and sidecar are written to.
        #[arg(long)]
        output_dir: Utf8PathBuf,

        /// Scratch directory for the upstream clone; defaults to a temporary one.
        #[arg(long)]
        work_dir: Option<Utf8PathBuf>,

        /// Repository to clone instead of the manifest's upstream.
        #[arg(long)]
        source: Option<String>,
    },

    /// Install the pinned generation and print its absolute path.
    Install {
        /// Directory holding snapshot generations.
        #[arg(long)]
        root: Utf8PathBuf,

        /// Install this local bundle (with its `.sha256` sidecar) instead of
        /// downloading the published one.
        #[arg(long)]
        bundle: Option<Utf8PathBuf>,
    },

    /// Fully verify a repository against the pinned snapshot.
    Verify {
        /// The bare repository to verify.
        #[arg(long)]
        repo: Utf8PathBuf,
    },

    /// Print where the pinned generation lives under a root, without installing.
    Path {
        /// Directory holding snapshot generations.
        #[arg(long)]
        root: Utf8PathBuf,
    },
}

/// Parse arguments, run the subcommand, and report failures on stderr.
fn main() {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(Some(line)) => {
            let mut stdout = std::io::stdout();
            if writeln!(stdout, "{line}").is_err() {
                std::process::exit(1);
            }
        }
        Ok(None) => {}
        Err(error) => {
            eprintln!("whitaker-clippy-snapshot: {error}");
            std::process::exit(1);
        }
    }
}

/// Run one subcommand, returning the line it prints on success.
fn run(command: Command) -> Result<Option<String>, ClippySnapshotError> {
    let manifest = embedded_manifest()?;
    match command {
        Command::Build {
            output_dir,
            work_dir,
            source,
        } => {
            let scratch =
                tempfile::tempdir().map_err(|error| ClippySnapshotError::io("tempdir", &error))?;
            let work_dir = work_dir.unwrap_or_else(|| utf8_temp(&scratch));
            // The token is offered only after an anonymous clone is refused.
            let token = std::env::var("GH_TOKEN")
                .or_else(|_| std::env::var("GITHUB_TOKEN"))
                .ok();
            let built = build_bundle(&BuildRequest {
                manifest,
                source: source.as_deref().unwrap_or(&manifest.upstream),
                work_dir: &work_dir,
                output_dir: &output_dir,
                token: token.as_deref(),
            })?;
            Ok(Some(built.artefact.bundle.to_string()))
        }
        Command::Install { root, bundle } => {
            let source = bundle.map_or(BundleSource::Release, BundleSource::Local);
            provision(&source, manifest, &root).map(|path| Some(path.to_string()))
        }
        Command::Verify { repo } => verify_repository(&repo, manifest).map(|()| None),
        Command::Path { root } => Ok(Some(generation_path(&root, manifest).to_string())),
    }
}

/// View a temporary directory as UTF-8, falling back to a relative scratch path.
fn utf8_temp(dir: &tempfile::TempDir) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .unwrap_or_else(|_| Utf8PathBuf::from("whitaker-clippy-snapshot-work"))
}

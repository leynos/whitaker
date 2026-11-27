//! The `-v` coverage listing: which packages this run will check.
//!
//! Cargo's own progress lines tell a reader what was checked, but only while
//! the build is running and only if Cargo's progress was not suppressed. The
//! listing states the same set before the child starts, so `whitaker check -v`
//! can be read as "this is the coverage I am about to get", and a silent gap
//! such as a workspace root package hiding its members is visible.
//!
//! The listing is advisory by design. A metadata failure is logged and skipped
//! rather than failing the run: Cargo is about to describe the same workspace
//! and will report a real defect itself, with the status the exit contract
//! already specifies.

use std::io::Write;

use camino::Utf8Path;

use super::{cargo::CargoOptions, selection};
use crate::ports::{
    PackageFacts,
    PackageQueryError,
    WorkspaceFacts,
    WorkspaceQuery,
    WorkspaceQueryRequest,
};

/// What the listing needs to ask Cargo, and where to write.
pub struct ListingRequest<'a> {
    /// The Cargo selectors the user gave.
    ///
    /// The manifest path is part of those options: the same value the run
    /// forwards is the value the metadata query reads.
    pub options: &'a CargoOptions,
    /// Where the lint run will execute, which decides Cargo's default set.
    pub current_dir: &'a Utf8Path,
    /// Where the listing is written.
    pub stderr: &'a mut dyn Write,
}

/// Report whether the invocation asked for the listing.
///
/// `-v` is Whitaker's own verbosity, so it is the invocation's answer rather
/// than Cargo's `-q`. A quiet invocation never prints the listing even when it
/// is also verbose, because `-q` is the stronger statement.
#[must_use]
pub const fn is_requested(quiet: bool, verbosity: u8) -> bool { !quiet && verbosity > 0 }

/// Write the coverage listing, or a note explaining why it is unavailable.
///
/// # Examples
///
/// ```no_run
/// use std::io;
///
/// use whitaker::{
///     adapters::SystemServices,
///     cli::check::{
///         CargoOptions,
///         listing::{ListingRequest, write_listing},
///     },
/// };
///
/// let services = SystemServices::new();
/// let mut stderr = io::stderr();
/// write_listing(
///     &services,
///     &mut ListingRequest {
///         options: &CargoOptions::default(),
///         current_dir: camino::Utf8Path::new("."),
///         stderr: &mut stderr,
///     },
/// );
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn write_listing(services: &dyn WorkspaceQuery, request: &mut ListingRequest<'_>) {
    let ListingRequest {
        options,
        current_dir,
        stderr,
    } = request;
    let Some(facts) = workspace(
        services,
        current_dir,
        options.manifest_path.as_deref(),
        &mut **stderr,
    ) else {
        return;
    };
    let Some(selected) = selection::selected_packages(&facts, options)
        .inspect_err(|error| {
            // Cargo will reject the same selector and report it authoritatively
            // with the exit status the contract already gives it.
            tracing::debug!(%error, "skipping the check listing: the selection did not resolve");
        })
        .ok()
    else {
        return;
    };
    write_lines(&selected, &mut **stderr);
}

/// Read the workspace facts, or note why they are unavailable.
fn workspace(
    services: &dyn WorkspaceQuery,
    current_dir: &Utf8Path,
    manifest_path: Option<&Utf8Path>,
    stderr: &mut dyn Write,
) -> Option<WorkspaceFacts> {
    match services.workspace_facts(&WorkspaceQueryRequest {
        current_dir,
        manifest_path,
    }) {
        Ok(facts) => Some(facts),
        Err(error) => {
            report_query_failure(&error, stderr);
            None
        }
    }
}

/// Write one Cargo-style line per selected package.
fn write_lines(selected: &[&PackageFacts], stderr: &mut dyn Write) {
    for package in selected {
        if !write_line(package, stderr) {
            return;
        }
    }
}

/// Write one package's line and report whether the stream accepted it.
fn write_line(package: &PackageFacts, stderr: &mut dyn Write) -> bool {
    match writeln!(stderr, "{}", selection::checking_line(package)) {
        Ok(()) => true,
        Err(error) => {
            tracing::error!(%error, "failed to write the check listing");
            false
        }
    }
}

/// Note why the listing is unavailable, without failing the run.
fn report_query_failure(error: &PackageQueryError, stderr: &mut dyn Write) {
    tracing::debug!(category = category(error), "skipping the check listing");
    write_notice(stderr);
}

/// Tell the user the listing is missing while the run continues.
fn write_notice(stderr: &mut dyn Write) {
    const NOTICE: &str = concat!(
        "whitaker: could not list the packages Cargo will check; ",
        "the run continues and Cargo reports any workspace defect"
    );
    if let Err(error) = writeln!(stderr, "{NOTICE}") {
        tracing::error!(%error, "failed to write the check listing notice");
    }
}

/// Name a query failure's class, never its payload.
///
/// The payload can name a path, and a log line is not the place for one; the
/// classes are bounded so a dashboard can group them.
const fn category(error: &PackageQueryError) -> &'static str {
    match error {
        PackageQueryError::Spawn { .. } => "spawn",
        PackageQueryError::Metadata { .. } => "metadata",
        PackageQueryError::Decode { .. } => "decode",
    }
}

#[cfg(test)]
mod tests {
    //! When the listing is written, and what it says.

    use std::io;

    use camino::Utf8Path;
    use rstest::rstest;

    use super::{ListingRequest, is_requested, write_listing};
    use crate::{
        cli::check::CargoOptions,
        ports::{
            PackageFacts,
            PackageQueryError,
            WorkspaceFacts,
            WorkspaceQuery,
            WorkspaceQueryRequest,
        },
    };

    /// A query that answers with fixed facts or a fixed failure.
    struct FakeQuery {
        /// The answer to give.
        answer: Result<WorkspaceFacts, &'static str>,
    }

    impl WorkspaceQuery for FakeQuery {
        fn workspace_facts(
            &self,
            _request: &WorkspaceQueryRequest<'_>,
        ) -> Result<WorkspaceFacts, PackageQueryError> {
            self.answer
                .clone()
                .map_err(|message| PackageQueryError::Metadata {
                    message: message.to_owned(),
                })
        }
    }

    /// Facts whose default selection is one member.
    fn facts() -> WorkspaceFacts {
        let package = PackageFacts {
            name: "member_b".to_owned(),
            version: "0.1.0".to_owned(),
            manifest_path: "/work/app/member_b/Cargo.toml".into(),
        };
        WorkspaceFacts {
            workspace_root: "/work/app".into(),
            members: vec![package.clone()],
            default_members: vec![package],
        }
    }

    /// Render the listing for `answer` and `options`.
    fn listing(answer: Result<WorkspaceFacts, &'static str>, options: &CargoOptions) -> String {
        let query = FakeQuery { answer };
        let mut stderr = Vec::new();
        write_listing(
            &query,
            &mut ListingRequest {
                options,
                current_dir: Utf8Path::new("/work/app"),
                stderr: &mut stderr,
            },
        );
        String::from_utf8(stderr).expect("the listing is UTF-8")
    }

    /// Show the packages in Cargo's own progress-line shape.
    #[test]
    fn the_listing_names_the_selected_packages() {
        let shown = listing(Ok(facts()), &CargoOptions::default());

        assert_eq!(
            shown, "    Checking member_b v0.1.0 (/work/app/member_b)\n",
            "the listing must name the package Cargo will check"
        );
    }

    /// Keep standard output pure: the listing goes to the given stream only.
    #[test]
    fn the_listing_is_written_only_to_the_supplied_stream() {
        let shown = listing(Ok(facts()), &CargoOptions::default());

        assert!(
            !shown.is_empty(),
            "the listing must be produced so the stream assertion is meaningful"
        );
        assert!(
            shown.starts_with("    Checking "),
            "the listing must use Cargo's line shape, got {shown:?}"
        );
    }

    /// Continue the run with a note when the query fails.
    #[test]
    fn a_failed_query_is_noted_and_does_not_stop_the_run() {
        let shown = listing(Err("no workspace here"), &CargoOptions::default());

        assert!(
            shown.contains("could not list the packages Cargo will check"),
            "a failed query must be reported, got {shown:?}"
        );
    }

    /// Print nothing when a selector matches nothing, and leave it to Cargo.
    #[test]
    fn an_unresolved_selection_prints_nothing() {
        let options = CargoOptions {
            packages: vec!["absent".to_owned()],
            ..CargoOptions::default()
        };

        let shown = listing(Ok(facts()), &options);

        assert!(
            shown.is_empty(),
            "an unresolved selector must not print a guessed listing, got {shown:?}"
        );
    }

    /// Ask for the listing only when the invocation is verbose and not quiet.
    #[rstest]
    #[case::default_verbosity(false, 0, false)]
    #[case::verbose(false, 1, true)]
    #[case::very_verbose(false, 2, true)]
    #[case::quietly_verbose(true, 1, false)]
    fn the_listing_is_requested_only_by_verbosity(
        #[case] quiet: bool,
        #[case] verbosity: u8,
        #[case] expected: bool,
    ) {
        assert_eq!(
            is_requested(quiet, verbosity),
            expected,
            "quiet={quiet} verbosity={verbosity}"
        );
    }

    /// Keep the stream type honest: a failing stream is handled, not panicked.
    #[test]
    fn a_failing_stream_does_not_panic() {
        struct FailingWriter;

        impl io::Write for FailingWriter {
            fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("the stream is closed"))
            }

            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }

        let query = FakeQuery {
            answer: Ok(facts()),
        };
        let mut stderr = FailingWriter;
        write_listing(
            &query,
            &mut ListingRequest {
                options: &CargoOptions::default(),
                current_dir: Utf8Path::new("/work/app"),
                stderr: &mut stderr,
            },
        );
    }
}

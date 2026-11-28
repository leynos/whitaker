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

use std::{
    ffi::{OsStr, OsString},
    io::Write,
};

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
    /// The arguments forwarded after `--`.
    ///
    /// They belong to Cargo. A package selector among them changes what Cargo
    /// checks without changing [`options`](ListingRequest::options), so the
    /// listing is reported unavailable rather than shown for the wrong set.
    pub suffix: &'a [OsString],
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
///         suffix: &[],
///         current_dir: camino::Utf8Path::new("."),
///         stderr: &mut stderr,
///     },
/// );
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn write_listing(services: &dyn WorkspaceQuery, request: &mut ListingRequest<'_>) {
    let ListingRequest {
        options,
        suffix,
        current_dir,
        stderr,
    } = request;
    if suffix_selects(suffix) {
        tracing::debug!("skipping the check listing: the forwarded suffix selects packages");
        write_notice(stderr, SUFFIX_NOTICE);
        return;
    }
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
    write_notice(stderr, UNAVAILABLE_NOTICE);
}

/// Say that the listing is unavailable, without failing the run.
const UNAVAILABLE_NOTICE: &str = concat!(
    "whitaker: could not list the packages Cargo will check; ",
    "the run continues and Cargo reports any workspace defect"
);

/// Say why a forwarded selector hides the listing.
const SUFFIX_NOTICE: &str = concat!(
    "whitaker: cannot list the packages Cargo will check: ",
    "a package or target selector after `--` is not modelled; ",
    "put it before `--` to see the listing"
);

/// Tell the user the listing is missing while the run continues.
fn write_notice(stderr: &mut dyn Write, notice: &str) {
    if let Err(error) = writeln!(stderr, "{notice}") {
        tracing::error!(%error, "failed to write the check listing notice");
    }
}

/// Report whether the forwarded suffix carries a package or target selector.
///
/// Both change the set the listing names: a package selector changes which
/// packages Cargo visits, and a target selector changes which of them have
/// work. Feature selectors do not change the package or target set, so they
/// leave the listing available. The spellings checked are Cargo's own,
/// including attached and `=` forms.
fn suffix_selects(suffix: &[OsString]) -> bool {
    suffix.iter().any(|argument| is_selector(argument))
}

/// Report whether one forwarded argument selects packages or targets.
fn is_selector(argument: &OsStr) -> bool {
    let Some(text) = argument.to_str() else {
        return false;
    };
    EXACT_SELECTORS.contains(&text)
        || ATTACHED_SELECTORS
            .iter()
            .any(|prefix| text.starts_with(prefix))
        || short_cluster_selects(text)
}

/// Selector flags that never take a value.
const EXACT_SELECTORS: &[&str] = &[
    "-p",
    "--package",
    "--workspace",
    "--all",
    "--exclude",
    "--manifest-path",
    "--lib",
    "--bins",
    "--bin",
    "--tests",
    "--test",
    "--examples",
    "--example",
    "--benches",
    "--bench",
    "--all-targets",
];

/// Selector flags whose value is attached with `=`.
const ATTACHED_SELECTORS: &[&str] = &[
    "--package=",
    "--exclude=",
    "--manifest-path=",
    "--bin=",
    "--test=",
    "--example=",
    "--bench=",
];

/// Report whether a short-option group carries Cargo's `-p`.
fn short_cluster_selects(text: &str) -> bool {
    text.strip_prefix('-')
        .is_some_and(|cluster| !cluster.starts_with('-') && cluster.contains('p'))
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
#[path = "listing_tests.rs"]
mod tests;

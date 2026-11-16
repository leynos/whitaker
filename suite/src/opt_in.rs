//! The suite's `[whitaker]` table in `dylint.toml`.
//!
//! The table is the driver-level opt-in contract for experimental lints
//! (ADR 0014). This module only deserializes it and combines it with the
//! build's baseline; name validation belongs to
//! [`whitaker_common::experimental`], which the installer and the forthcoming
//! `whitaker` command share. It carries no compiler types, so it is testable
//! without a `rustc` session.

use serde::Deserialize;
use whitaker_common::experimental::{ExperimentalOptInError, ExperimentalSelection, resolve};

/// Name of the `dylint.toml` table this module reads.
pub const OPT_IN_TABLE: &str = "whitaker";

/// Deserialized `[whitaker]` table.
///
/// Unknown keys are rejected: a misspelt `experimentl = [...]` would otherwise
/// leave a repository believing its lints ran (compare issue 332).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct OptInConfig {
    /// Experimental lints the repository opts in to, by canonical name.
    pub experimental: Vec<String>,
}

/// What loading the opt-in produced for the driver to act on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedOptIn {
    /// Experimental lints whose passes should run.
    pub selection: ExperimentalSelection,
    /// Promoted lints that the table still lists, for an informational note.
    pub redundant: Vec<&'static str>,
}

/// Why the opt-in could not be loaded; the message is shown to the user as is.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OptInError {
    /// The `[whitaker]` table could not be read or deserialized.
    #[error("cannot read the `[{OPT_IN_TABLE}]` table of dylint.toml: {0}")]
    Config(String),
    /// A listed name is neither an experimental nor a promoted lint.
    #[error(transparent)]
    Unknown(#[from] ExperimentalOptInError),
}

/// Combines the configured table with the build's baseline selection.
///
/// `table` is the result of reading `[whitaker]`: `Ok(None)` when the table is
/// absent (the common case), and `Err` for I/O or deserialization failures,
/// which are reported rather than treated as "no opt-in".
///
/// # Errors
///
/// Returns [`OptInError::Config`] when the table cannot be read, and
/// [`OptInError::Unknown`] when it lists a name that is not a lint.
pub fn load<E: std::fmt::Display>(
    table: Result<Option<OptInConfig>, E>,
    baseline: &ExperimentalSelection,
) -> Result<LoadedOptIn, OptInError> {
    load_with(table, baseline, resolve)
}

/// [`load`] with the name resolver supplied, so tests can drive the promotion
/// path before any real lint has been promoted.
///
/// # Errors
///
/// As [`load`].
pub fn load_with<E: std::fmt::Display>(
    table: Result<Option<OptInConfig>, E>,
    baseline: &ExperimentalSelection,
    resolver: impl FnOnce(&[String]) -> Result<ExperimentalSelection, ExperimentalOptInError>,
) -> Result<LoadedOptIn, OptInError> {
    let config = table
        .map_err(|error| OptInError::Config(error.to_string()))?
        .unwrap_or_default();
    let listed = resolver(&config.experimental)?;
    let redundant = listed.redundant().to_vec();
    Ok(LoadedOptIn {
        selection: baseline.clone().union(&listed),
        redundant,
    })
}

/// Loads the opt-in for a build that is, or is not, an `experimental-enable-all`
/// build.
///
/// The driver passes `cfg!(feature = "experimental-enable-all")`, so the one
/// decision the build feature makes is a parameter a test can set both ways.
///
/// # Errors
///
/// As [`load`].
pub fn load_for_build<E: std::fmt::Display>(
    table: Result<Option<OptInConfig>, E>,
    is_enable_all: bool,
) -> Result<LoadedOptIn, OptInError> {
    load(table, &baseline_for(is_enable_all))
}

/// The experimental lints every run starts with, before `dylint.toml` adds any.
///
/// Empty for the prebuilt suite. A local `--experimental` build enables the
/// suite's `experimental-enable-all` feature, which passes `true` here.
///
/// # Examples
///
/// ```
/// use whitaker_suite::opt_in::baseline_for;
///
/// assert!(!baseline_for(false).contains("string_continuation_style"));
/// assert!(baseline_for(true).contains("string_continuation_style"));
/// ```
#[must_use]
pub fn baseline_for(is_enable_all: bool) -> ExperimentalSelection {
    if is_enable_all {
        ExperimentalSelection::all()
    } else {
        ExperimentalSelection::none()
    }
}

#[cfg(test)]
#[path = "opt_in_tests.rs"]
mod tests;

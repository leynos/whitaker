//! The rule that turns a missing published artefact into an error.
//!
//! Kept beside the argument definitions rather than inside them: the
//! policy is read by the install flow and the dependency installer as
//! well as by the parser, and `cli.rs` is at the repository's file-size
//! limit.

use super::InstallArgs;

#[cfg(test)]
#[path = "source_policy_tests.rs"]
mod tests;

/// Name of the environment variable that forbids a source build.
pub const NO_SOURCE_FALLBACK_ENV: &str = "WHITAKER_NO_SOURCE_FALLBACK";

/// Whether the environment forbids falling back to a source build.
///
/// Any value other than the empty string, `0` or `false` enables the rule.
/// A caller who exported the variable at all meant something by it, and
/// reading an unrecognized value as "off" would silently disable a
/// protection.
fn environment_forbids_source_fallback() -> bool {
    match std::env::var(NO_SOURCE_FALLBACK_ENV) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false"
        ),
        // Present but not valid Unicode. The variable was set, so the rule
        // is on: reading unreadable bytes as "off" would disable a protection
        // precisely when the value cannot be inspected.
        Err(std::env::VarError::NotUnicode(_)) => true,
        Err(std::env::VarError::NotPresent) => false,
    }
}

impl InstallArgs {
    /// Whether a missing published artefact must fail rather than build.
    ///
    /// The environment variable exists for callers that cannot easily add a
    /// flag, such as a composite action invoking the installer through a
    /// wrapper. Either source enables the rule; neither disables the other,
    /// because a caller who set one meant it.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::cli::InstallArgs;
    ///
    /// assert!(!InstallArgs::default().forbids_source_fallback());
    /// ```
    #[must_use]
    pub fn forbids_source_fallback(&self) -> bool {
        self.no_source_fallback || environment_forbids_source_fallback()
    }

    /// Reject a run that both forbids and requires a source build.
    ///
    /// clap rejects the contradictory *flags*, but the rule can also arrive
    /// through the environment, where clap cannot see it. Without this a lane
    /// exporting `WHITAKER_NO_SOURCE_FALLBACK` and passing `--build-only`
    /// would be told to build from source and forbidden from doing so, and
    /// would meet the contradiction as a mid-install error rather than at its
    /// first argument check.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InstallerError::ConflictingSourceOptions`] when
    /// the effective policy forbids a source build another option requires.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::cli::InstallArgs;
    ///
    /// assert!(InstallArgs::default().validate_source_options().is_ok());
    /// ```
    pub fn validate_source_options(&self) -> crate::error::Result<()> {
        if !self.forbids_source_fallback() {
            return Ok(());
        }
        let requires_source = if self.is_build_only {
            Some("--build-only")
        } else if self.experimental {
            Some("--experimental")
        } else if self.suite_version.is_some() {
            Some("--suite-version")
        } else {
            None
        };
        requires_source.map_or(Ok(()), |option| {
            Err(crate::error::InstallerError::ConflictingSourceOptions {
                option: option.to_owned(),
            })
        })
    }

    /// How this run should react to a missing published artefact.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::cli::InstallArgs;
    ///
    /// assert!(!InstallArgs::default().source_policy().no_source_fallback);
    /// ```
    #[must_use]
    pub fn source_policy(&self) -> crate::deps::SourcePolicy {
        crate::deps::SourcePolicy {
            quiet: self.quiet,
            no_source_fallback: self.forbids_source_fallback(),
        }
    }
}

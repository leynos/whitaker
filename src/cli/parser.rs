//! Parses localized Whitaker commands and extracts shared root options.

use std::ffi::{OsStr, OsString};

use clap::{CommandFactory, Parser, Subcommand as ClapSubcommand};
use ortho_config::{LocalizedParse as _, Localizer};
use whitaker_installer::cli::{InstallArgs, ListArgs};

/// Parsed root command and the common options consumed before dispatch.
#[derive(Debug)]
pub struct Cli {
    /// Quiet-mode option shared by Whitaker commands.
    pub quiet: bool,
    /// Verbosity count shared by Whitaker commands.
    pub verbosity: u8,
    /// Required root subcommand.
    pub command: Command,
}

/// Commands owned by the root Whitaker executable.
#[derive(Debug, ClapSubcommand)]
pub enum Command {
    /// Install or repair Whitaker dependencies and lint libraries.
    Install(InstallArgs),
    /// Show installed lints and bundle metadata.
    Ls(ListArgs),
}

/// Clap parser for the subcommand surface.
#[derive(Debug, Parser)]
#[command(
    name = "whitaker",
    version,
    about = "Install and inspect Whitaker lint libraries",
    after_help = concat!(
        "Global options accepted before or after a subcommand:\n",
        "  -q, --quiet\n",
        "  -v, --verbose...",
    )
)]
struct ParsedCli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    /// Parses arguments with localized command metadata and clap errors.
    ///
    /// Shared quiet and verbose options are extracted before clap parses the
    /// selected command. The installer argument type remains the single source
    /// of truth for its option values and conflicts.
    ///
    /// # Examples
    ///
    /// ```
    /// use ortho_config::NoOpLocalizer;
    /// use whitaker::cli::{Cli, Command};
    ///
    /// let cli =
    ///     Cli::try_parse_localized_from(["whitaker", "install", "--dry-run"], &NoOpLocalizer::new())?;
    /// assert!(matches!(cli.command, Command::Install(_)));
    /// # Ok::<(), clap::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a clap error for invalid options or invalid combinations.
    pub fn try_parse_localized_from<I, T>(
        arguments: I,
        localizer: &dyn Localizer,
    ) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let raw: Vec<OsString> = arguments.into_iter().map(Into::into).collect();
        let (parsed_arguments, quiet, verbosity) = extract_global_options(raw.clone());
        let parsed = ParsedCli::try_parse_localized_from(parsed_arguments, localizer)?;
        if matches!(&parsed.command, Command::Install(_))
            && let (Some(program), Some(install_index)) = (
                raw.first(),
                raw.iter()
                    .enumerate()
                    .skip(1)
                    .find_map(|(index, argument)| {
                        (argument == OsStr::new("install")).then_some(index)
                    }),
            )
        {
            // Keep the installer parser authoritative for value boundaries
            // and repeated flags instead of duplicating its option rules.
            let mut install_arguments = Vec::with_capacity(raw.len() - install_index);
            install_arguments.push(program.clone());
            install_arguments.extend(raw.iter().skip(install_index + 1).cloned());
            InstallArgs::try_parse_localized_from(install_arguments, localizer)?;
        }
        if quiet && verbosity > 0 {
            return Err(ParsedCli::command().error(
                clap::error::ErrorKind::ArgumentConflict,
                "the argument '--quiet' cannot be used with '--verbose'",
            ));
        }
        Ok(Self {
            quiet,
            verbosity,
            command: parsed.command,
        })
    }

    /// Returns install arguments with the shared root options applied.
    ///
    /// # Examples
    ///
    /// ```
    /// use ortho_config::NoOpLocalizer;
    /// use whitaker::cli::Cli;
    /// use whitaker_installer::cli::InstallArgs;
    ///
    /// let cli = Cli::try_parse_localized_from(
    ///     ["whitaker", "-v", "install", "--dry-run"],
    ///     &NoOpLocalizer::new(),
    /// )?;
    /// let arguments = cli.install_arguments(&InstallArgs::default());
    /// assert_eq!(arguments.verbosity, 1);
    /// # Ok::<(), clap::Error>(())
    /// ```
    #[must_use]
    pub fn install_arguments(&self, parsed_arguments: &InstallArgs) -> InstallArgs {
        let mut arguments = parsed_arguments.clone();
        arguments.quiet = self.quiet;
        arguments.verbosity = self.verbosity;
        arguments
    }
}

#[derive(Debug, Clone, Copy)]
enum CommonOption {
    Quiet,
    Verbose(u8),
    QuietAndVerbose(u8),
}

/// Keeps argv[0], then extracts shared flags until the `--` delimiter.
///
/// Quiet and verbose flags are accepted anywhere before `--`.
fn extract_global_options(arguments: Vec<OsString>) -> (Vec<OsString>, bool, u8) {
    let mut parsed_arguments = Vec::with_capacity(arguments.len());
    let mut quiet = false;
    let mut verbosity = 0_u8;
    let mut shared_options_are_enabled = true;
    for (index, argument) in arguments.into_iter().enumerate() {
        if index == 0 {
            parsed_arguments.push(argument);
            continue;
        }
        if argument == OsStr::new("--") {
            shared_options_are_enabled = false;
            parsed_arguments.push(argument);
            continue;
        }
        if !shared_options_are_enabled {
            parsed_arguments.push(argument);
            continue;
        }
        match common_option(&argument) {
            Some((CommonOption::Quiet, remainder)) => {
                quiet = true;
                parsed_arguments.extend(remainder);
            }
            Some((CommonOption::Verbose(count), remainder)) => {
                verbosity = verbosity.saturating_add(count);
                parsed_arguments.extend(remainder);
            }
            Some((CommonOption::QuietAndVerbose(count), remainder)) => {
                quiet = true;
                verbosity = verbosity.saturating_add(count);
                parsed_arguments.extend(remainder);
            }
            None => parsed_arguments.push(argument),
        }
    }
    (parsed_arguments, quiet, verbosity)
}

/// Recognizes a quiet or verbose flag, including supported short clusters.
fn common_option(argument: &OsStr) -> Option<(CommonOption, Option<OsString>)> {
    match argument.to_str()? {
        "-q" | "--quiet" => Some((CommonOption::Quiet, None)),
        "-v" | "--verbose" | "--verbosity" => Some((CommonOption::Verbose(1), None)),
        value => common_short_cluster(value).map(|(option, remainder)| {
            (
                option,
                remainder.map(|suffix| OsString::from(format!("-{suffix}"))),
            )
        }),
    }
}

/// Reports whether a short cluster contains only shared options.
pub(super) fn is_common_cluster(value: &str) -> bool {
    let Some(short_options) = value.strip_prefix('-') else {
        return false;
    };
    short_options.len() > 1
        && short_options
            .chars()
            .all(|option| matches!(option, 'q' | 'v'))
}

/// Extracts leading shared flags and leaves any attached value option intact.
fn common_short_cluster(value: &str) -> Option<(CommonOption, Option<&str>)> {
    let short_options = value.strip_prefix('-')?;
    if short_options.starts_with('-') {
        return None;
    }
    let common_end = short_options
        .char_indices()
        .find_map(|(index, option)| (!matches!(option, 'q' | 'v')).then_some(index))
        .unwrap_or(short_options.len());
    if common_end == 0 {
        return None;
    }
    let (common_options, remainder) = short_options.split_at(common_end);
    if remainder
        .chars()
        .next()
        .is_some_and(|option| !matches!(option, 'j' | 'l' | 't'))
    {
        return None;
    }
    let quiet = common_options.contains('q');
    let verbosity = u8::try_from(
        common_options
            .chars()
            .filter(|option| *option == 'v')
            .count(),
    )
    .unwrap_or(u8::MAX);
    let common_option = match (quiet, verbosity) {
        (true, 0) => CommonOption::Quiet,
        (false, count) => CommonOption::Verbose(count),
        (true, count) => CommonOption::QuietAndVerbose(count),
    };
    Some((common_option, (!remainder.is_empty()).then_some(remainder)))
}

#[cfg(test)]
mod tests {
    //! Tests for localized parsing and the shared root flags.

    use ortho_config::NoOpLocalizer;

    use super::{Cli, Command, is_common_cluster};

    #[test]
    fn common_flags_are_accepted_before_and_after_subcommands() {
        let before = Cli::try_parse_localized_from(
            ["whitaker", "-q", "install", "--dry-run"],
            &NoOpLocalizer::new(),
        )
        .expect("quiet before install should parse");
        assert!(before.quiet, "quiet should be stored on the root command");
        let after = Cli::try_parse_localized_from(
            ["whitaker", "install", "-vv", "--dry-run"],
            &NoOpLocalizer::new(),
        )
        .expect("repeated verbosity after install should parse");
        assert_eq!(after.verbosity, 2, "repeated verbosity should be counted");
    }

    #[test]
    fn root_flags_are_applied_to_the_shared_installer_request() {
        let cli = Cli::try_parse_localized_from(
            ["whitaker", "install", "--quiet", "--dry-run"],
            &NoOpLocalizer::new(),
        )
        .expect("install command should parse");
        let Command::Install(parsed) = &cli.command else {
            panic!("command should be install");
        };
        let effective = cli.install_arguments(parsed);
        assert!(
            effective.quiet,
            "shared quiet option should reach installer"
        );
        assert!(
            effective.execution.dry_run,
            "installer option should remain set"
        );
    }

    #[test]
    fn attached_value_options_are_not_classified_as_global_clusters() {
        assert!(is_common_cluster("-vq"));
        assert!(!is_common_cluster("-vj2"));
        assert!(!is_common_cluster("-qj2"));
    }

    #[cfg(unix)]
    #[test]
    fn invalid_utf8_arguments_remain_parseable_as_os_strings() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let invalid = OsString::from_vec(vec![0xff]);
        let parsed = Cli::try_parse_localized_from(
            [
                OsString::from("whitaker"),
                OsString::from("install"),
                invalid,
            ],
            &NoOpLocalizer::new(),
        );
        assert!(
            parsed.is_err(),
            "invalid install argument should be rejected"
        );
    }
}

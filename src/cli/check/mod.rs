//! The `whitaker check` command: readiness, the decision flow, and its output.
//!
//! The command has three parts and they are separated on purpose. This module
//! is the adapter between the [`CheckServices`] port and the pure flow; the
//! flow itself decides what to do and cannot perform I/O; and `messages`
//! turns the result into the two lines a person reads. Keeping the decision
//! free of ports is what lets the Kani harnesses drive every branch, and
//! telemetry observes the flow through a wrapper over [`CheckPorts`] so that
//! no port records its own answers.

mod args;
pub mod argv;
pub mod cargo;
mod classify;
pub mod exit;
pub mod flow;
pub mod listing;
mod messages;
pub mod policy;
pub mod selection;

use std::{ffi::OsString, io::Write};

pub use args::CheckArgs;
pub use cargo::CargoOptions;
use ortho_config::Localizer;

use self::{
    argv::assemble_dylint_args,
    classify::{decision, is_operational_failure, preflight_class, repair_fault, run_fault},
    exit::CheckOutcome,
    flow::{CheckPorts, RepairFault, RunFault, run_check_flow},
    listing::ListingRequest,
};
use crate::{
    adapters::check::telemetry::{
        MonotonicClock,
        observe_invocation,
        observe_preflight,
        observe_repair,
    },
    cli::exit::Outcome,
    diagnostics::DiagnosticSettings,
    ports::{
        CheckPrepared,
        CheckServices,
        Preflight,
        RepairVerbosity,
        WorkspaceQuery,
        classify_preflight_failure,
    },
};

/// What the user asked `check` to do, and the root flags that colour it.
///
/// The root flags travel with the request so an implicit repair inherits them:
/// `whitaker -q check` must not print an installer's progress.
#[derive(Debug, Clone, Copy)]
pub struct CheckRequest<'a> {
    /// The subcommand's own arguments, including the repair policy.
    pub arguments: &'a CheckArgs,
    /// The verbosity flags the root command held.
    pub settings: DiagnosticSettings,
    /// The directory the run will execute in.
    ///
    /// It decides Cargo's default package selection, so the coverage listing
    /// must use the same directory the child will. `None` means the process
    /// could not name its own directory; the listing is then unavailable and
    /// the run proceeds, exactly as Cargo will.
    pub current_dir: Option<&'a camino::Utf8Path>,
}

impl<'a> CheckRequest<'a> {
    /// Pair a parsed `check` invocation with the root flags it was parsed under.
    #[must_use]
    pub const fn new(
        arguments: &'a CheckArgs,
        settings: DiagnosticSettings,
        current_dir: Option<&'a camino::Utf8Path>,
    ) -> Self {
        Self {
            arguments,
            settings,
            current_dir,
        }
    }
}

/// Read the process working directory as a UTF-8 path.
///
/// The coverage listing needs the directory Cargo will use, because that is
/// what decides Cargo's default package selection. A directory that cannot be
/// read, or that is not UTF-8, leaves the listing unavailable; the run itself
/// is unaffected, and Dylint's own argument route is already UTF-8 only.
#[must_use]
pub fn working_directory() -> Option<camino::Utf8PathBuf> {
    std::env::current_dir()
        .ok()
        .and_then(|path| camino::Utf8PathBuf::from_path_buf(path).ok())
}

/// The adapters one `check` invocation drives.
///
/// The readiness/run port and the workspace-metadata port are separate
/// concerns with separate failure contracts, so they stay separate traits; this
/// value groups them at the call boundary where they travel together.
#[derive(Clone, Copy)]
pub struct CheckAdapters<'a> {
    /// Readiness inspection, repair and the lint run.
    pub check: &'a dyn CheckServices,
    /// Workspace facts for the `-v` coverage listing.
    pub packages: &'a dyn WorkspaceQuery,
}

/// Run `whitaker check` and report how it ended.
///
/// # Examples
///
/// ```no_run
/// use std::io;
///
/// use ortho_config::NoOpLocalizer;
/// use whitaker::{
///     adapters::SystemServices,
///     cli::{
///         check::{CheckAdapters, CheckArgs, CheckRequest, run_check_command},
///         exit::Outcome,
///     },
///     diagnostics::DiagnosticSettings,
/// };
///
/// let services = SystemServices::new();
/// // `--no-install` forbids provisioning. What this host then reports depends
/// // on whether it is provisioned already: an unprovisioned machine refuses
/// // with `provision.refused`, and a provisioned one lints. The command layer
/// // is therefore asserted against a scripted port in `cli::check::tests`,
/// // where the answer is deterministic; here only the call shape is shown.
/// let arguments = CheckArgs {
///     no_install: true,
///     ..CheckArgs::default()
/// };
/// let request = CheckRequest::new(&arguments, DiagnosticSettings::default(), None);
/// let adapters = CheckAdapters {
///     check: &services,
///     packages: &services,
/// };
/// let outcome = run_check_command(request, &NoOpLocalizer::new(), adapters, &mut io::sink());
/// println!("check ended with {outcome:?}");
/// ```
///
/// A host with no installation and `--no-install` reports
/// [`CheckOutcome::Refused`], which exits `4` with the `provision.refused`
/// key; `cli::check::tests` pins that answer against a scripted port.
pub fn run_check_command(
    request: CheckRequest<'_>,
    localizer: &dyn Localizer,
    adapters: CheckAdapters<'_>,
    stderr: &mut dyn Write,
) -> Outcome {
    let policy = policy::policy_for(
        request.arguments.no_install,
        adapters.check.library_path_is_user_managed(),
    );
    let mut runner = CheckRunner {
        services: adapters.check,
        packages: adapters.packages,
        settings: request.settings,
        cargo: &request.arguments.cargo,
        cargo_args: &request.arguments.cargo_args,
        current_dir: request.current_dir,
        stderr,
    };
    let outcome = {
        // Scoped so the observation wrapper's borrow ends before the outcome
        // is reported through the runner's own stream.
        let mut ports = RecordedPorts { inner: &mut runner };
        // The adapter records the invocation, so the command layer states what
        // happened and names no telemetry function or metric. The decision is
        // recorded inside the span that covers one invocation.
        observe_invocation(
            || run_check_flow(&mut ports, policy),
            |outcome| (decision(*outcome), is_operational_failure(*outcome)),
            |outcome| match outcome {
                CheckOutcome::Refused(reason) => Some(*reason),
                _ => None,
            },
        )
    };
    report(outcome, localizer, runner.stderr);
    Outcome::Check(outcome)
}

/// Write the diagnostic a `check` outcome deserves.
///
/// The router calls this when a command failed before `check` ran, such as a
/// help request whose text never reached standard output, so that a `check`
/// failure always names its reason key even when the first diagnostic was
/// lost.
pub(crate) fn report_outcome(
    outcome: CheckOutcome,
    localizer: &dyn Localizer,
    stderr: &mut dyn Write,
) {
    report(outcome, localizer, stderr);
}

/// Write the diagnostic an outcome deserves, without letting it mask the code.
///
/// A lost diagnostic cannot change an outcome that has already been decided,
/// so it is reported through `tracing` instead of being folded into the status
/// the caller sees.
fn report(outcome: CheckOutcome, localizer: &dyn Localizer, stderr: &mut dyn Write) {
    if let Err(error) = messages::report(outcome, localizer, stderr) {
        tracing::error!(%error, "failed to write the check diagnostic to stderr");
    }
}

/// Adapts the `check` port to the pure flow for one invocation.
struct CheckRunner<'a> {
    /// The readiness and run adapter.
    services: &'a dyn CheckServices,
    /// The workspace facts the coverage listing reads.
    packages: &'a dyn WorkspaceQuery,
    /// The root command's own diagnostic policy, passed to an implicit install.
    settings: DiagnosticSettings,
    /// The Cargo selectors this invocation re-emits.
    cargo: &'a CargoOptions,
    /// The arguments this invocation forwards, before Dylint's own are added.
    cargo_args: &'a [OsString],
    /// The directory the child will run in, when the process can name it.
    current_dir: Option<&'a camino::Utf8Path>,
    /// Where the child's own diagnostics are written.
    stderr: &'a mut dyn Write,
}

impl CheckPorts for CheckRunner<'_> {
    type Prepared = CheckPrepared;

    /// Translate the port's readiness answer into the flow's vocabulary.
    ///
    /// The adapter hands back either evidence Dylint may run or a typed
    /// failure, and the failure is what the flow classifies: repairable reasons
    /// permit a repair, host faults never do.
    fn preflight(&mut self) -> Preflight<CheckPrepared> {
        match self.services.approve_readiness() {
            Ok(prepared) => Preflight::Ready(prepared),
            Err(error) => classify_preflight_failure(&error).into_preflight(),
        }
    }

    /// Provision the installation under the adapter's own lock.
    ///
    /// The flow permits this only when [`policy`] allowed it, so the adapter
    /// does not re-check the policy: it re-checks the installation, which is
    /// what makes a concurrent install converge instead of repeat.
    fn repair(&mut self) -> Result<(), RepairFault> {
        // The port takes the two facts an installer needs, not this binary's
        // diagnostic policy: the adapter translates them into its own flags.
        self.services
            .repair(
                RepairVerbosity {
                    quiet: self.settings.quiet,
                    verbosity: self.settings.verbosity,
                },
                self.stderr,
            )
            .map_err(|error| {
                // The flow acts on the bounded class, but which class it was
                // would otherwise be lost at this boundary. Log the category,
                // never the error's own text: its payload can name a path the
                // user chose, and the typed error still reaches the caller.
                tracing::warn!(
                    category = error.category(),
                    "repair failed; classifying the cause"
                );
                repair_fault(&error)
            })
    }

    /// Run Dylint with the arguments this invocation forwarded.
    ///
    /// The caller's Cargo suffix is assembled here rather than in the flow, and
    /// the preparation the flow selected is passed through unchanged, so the
    /// run uses exactly the installation readiness approved.
    fn run(&mut self, prepared: CheckPrepared) -> Result<i32, RunFault> {
        self.announce_deprecated_alias();
        self.list_coverage();
        let arguments = assemble_dylint_args(self.cargo, self.cargo_args);
        self.services
            .check_run(prepared, &arguments, self.stderr)
            .map_err(|error| {
                // As for the repair above: the category is bounded, the
                // payload may carry a suite path.
                tracing::warn!(
                    category = error.category(),
                    "lint run failed; classifying the cause"
                );
                run_fault(error)
            })
    }
}

impl CheckRunner<'_> {
    /// Name the deprecated alias so the user knows which selector took effect.
    ///
    /// Cargo documents `--all` as a deprecated alias for `--workspace` but is
    /// silent at runtime, and Whitaker's own legacy path uses `--all` for
    /// "load every lint library". Saying it once, on the diagnostic stream,
    /// keeps the two meanings from being confused; `-q` suppresses it as it
    /// suppresses every other Whitaker diagnostic.
    fn announce_deprecated_alias(&mut self) {
        if self.settings.quiet || !self.cargo.all {
            return;
        }
        announce_alias(self.stderr);
    }

    /// Show the packages the child is about to check.
    ///
    /// Suppressed by `-q`, by an absent verbosity, and by a process that cannot
    /// name its own working directory; in the last case Cargo will still run
    /// and report whatever makes the directory unusable.
    fn list_coverage(&mut self) {
        if !listing::is_requested(self.settings.quiet, self.settings.verbosity) {
            return;
        }
        let Some(current_dir) = self.current_dir else {
            tracing::debug!("skipping the check listing: the working directory is unknown");
            return;
        };
        listing::write_listing(
            self.packages,
            &mut ListingRequest {
                options: self.cargo,
                current_dir,
                stderr: self.stderr,
            },
        );
    }
}

/// Write the deprecated-alias notice, tolerating a closed diagnostic stream.
fn announce_alias(stderr: &mut dyn Write) {
    if let Err(error) = writeln!(stderr, "{}", cargo::ALL_DEPRECATION_NOTICE) {
        tracing::error!(%error, "failed to write the deprecated-alias notice");
    }
}

/// Record the answers the flow consumes, without the ports recording them.
///
/// The flow decides when readiness is asked — including the second question
/// after a repair — so a wrapper sees each answer the decision acted on
/// exactly once. Readiness is not a pure query: an answer records an approval,
/// and this wrapper observes the answer without standing in its way. The flow
/// keeps its own telemetry policy, and the Kani harnesses keep driving the
/// same [`CheckPorts`] seam this wrapper forwards.
struct RecordedPorts<'a, P> {
    /// The ports the flow drives.
    inner: &'a mut P,
}

impl<P: CheckPorts> CheckPorts for RecordedPorts<'_, P> {
    type Prepared = P::Prepared;

    /// Ask the port, then record which class of answer it gave.
    fn preflight(&mut self) -> Preflight<Self::Prepared> {
        // The adapter records the class; the command states it.
        observe_preflight(|| self.inner.preflight(), preflight_class)
    }

    /// Provision through the port, then record whether it succeeded.
    fn repair(&mut self) -> Result<(), RepairFault> {
        // The adapter times the repair, so this layer picks no clock and names
        // no metric; the duration is a field on the event and the span, never a
        // label.
        observe_repair(&MonotonicClock, || self.inner.repair(), Result::is_ok)
    }

    /// Run against the prepared value, unchanged.
    fn run(&mut self, prepared: Self::Prepared) -> Result<i32, RunFault> {
        self.inner.run(prepared)
    }
}

#[cfg(test)]
mod tests;

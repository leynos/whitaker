//! Compile-pass guard for the root crate's public const API.
//!
//! Each item below is evaluated, or its body const-checked, at compile time.
//! Removing `const` from any function it calls turns this fixture into a
//! compile error and fails the trybuild case, so the const-ness downstream
//! callers can rely on stays a contract rather than an accident.
//!
//! ## What is deliberately absent
//!
//! `SystemServices::new` is *not* a const function, and this fixture records
//! that: it is called at run time only, because it snapshots the process
//! environment, which no const context can read. `ApprovalId` values are
//! produced by the adapter that issues them, so the fixture only pins that the
//! constructor and accessor are const.

use whitaker::{
    cli::{
        Cli,
        check::{
            CheckArgs,
            CheckRequest,
            flow::Policy,
            policy::policy_for,
        },
    },
    diagnostics::DiagnosticSettings,
    ports::ApprovalId,
};

/// Policy selection is a pure truth table, so it is usable in a const item.
const REFUSING: Policy = policy_for(true, false);

/// An approval number is data, so its constructor and accessor are const.
const APPROVAL: ApprovalId = ApprovalId::new(7);
const APPROVAL_NUMBER: u64 = APPROVAL.number();

/// Forward to `CheckRequest::new` so this body is const-checked.
///
/// The body compiles only while `CheckRequest::new` is a `const fn`, even
/// though the forwarding function is never called in a constant context.
const fn check_request<'a>(
    arguments: &'a CheckArgs,
    settings: DiagnosticSettings,
) -> CheckRequest<'a> {
    CheckRequest::new(arguments, settings, None)
}

/// Forward to `Cli::diagnostic_settings` so this body is const-checked.
const fn settings_of(cli: &Cli) -> DiagnosticSettings { cli.diagnostic_settings() }

fn main() {
    let _: fn(&CheckArgs, DiagnosticSettings) -> CheckRequest<'_> = check_request;
    let _: fn(&Cli) -> DiagnosticSettings = settings_of;
    assert!(APPROVAL_NUMBER == 7);
    let _ = REFUSING;
}

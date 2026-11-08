//! Compile-fail guard for the former bare-status return contract.

use whitaker_installer::deps::{check_dylint_tools, CommandExecutor, DylintToolStatus};

/// Require the removed bare-status result type.
fn legacy_check(executor: &dyn CommandExecutor) {
    let status: DylintToolStatus = check_dylint_tools(executor);
    let _ = status;
}

/// Keep the legacy API check reachable during compilation.
fn main() {
    let _ = legacy_check;
}

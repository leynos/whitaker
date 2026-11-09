//! Warm the pinned Dylint driver before offline test execution.

/// Prepare the Dylint driver and print its path.
///
/// # Errors
///
/// Return an error if the pinned driver cannot be prepared.
fn main() -> anyhow::Result<()> { dylint_testing::warm_driver::run_from_process() }

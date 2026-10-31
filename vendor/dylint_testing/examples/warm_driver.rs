//! Warm the pinned Dylint driver before offline test execution.

use anyhow::Context as _;
use dylint_testing::prepare_driver;

/// Prepare the Dylint driver and print its path.
///
/// # Errors
///
/// Return an error if the pinned driver cannot be prepared.
fn main() -> anyhow::Result<()> {
    let driver = prepare_driver().context("failed to prepare Dylint driver")?;
    println!("{}", driver.display());
    Ok(())
}

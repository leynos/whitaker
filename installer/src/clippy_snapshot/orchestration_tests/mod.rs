//! Verification and installation orchestration over scripted ports.
//!
//! These tests never run Git or touch the filesystem. They show that the
//! orchestration asks the ports in the intended order, stops at the first
//! failed check, and neither publishes nor records metadata after a failure.

mod install;
mod provision;
mod support;
mod verify;

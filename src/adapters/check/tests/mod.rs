//! What the `check` adapter accepts as a usable library path, what it
//! re-checks before Dylint starts, and what a repair preserves.
//!
//! The concerns are tested apart because they answer different questions: one
//! decides whether Dylint may start at all, another whether the answer still
//! holds once the child is about to run, and the last what a repair may change
//! on a machine that is already provisioned.

mod fixtures;
mod recheck;
mod repair;
mod repair_flow;
mod repair_streams;
mod suite;

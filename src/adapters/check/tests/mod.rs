//! What the `check` adapter accepts as a usable library path, and what a
//! repair preserves.
//!
//! The two concerns are tested apart because they answer different questions:
//! one decides whether Dylint may start at all, the other what a repair may
//! change on a machine that is already provisioned.

mod fixtures;
mod repair;
mod repair_flow;
mod suite;

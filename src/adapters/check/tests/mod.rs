//! What the `check` adapter accepts as a usable library path, what it
//! re-checks before Dylint starts, what a repair preserves, and which status a
//! completed run reports.
//!
//! The concerns are tested apart because they answer different questions: one
//! decides whether Dylint may start at all, another whether the answer still
//! holds once the child is about to run, the next what a repair may change on a
//! machine that is already provisioned, and the last what the run says once it
//! has finished.

mod approval;
mod approval_run;
mod fixtures;
mod listing;
mod recheck;
mod repair;
mod repair_flow;
mod repair_streams;
mod status;
mod suite;

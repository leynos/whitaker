//! Public command-line application boundary for Whitaker.
//!
//! The root package owns command selection and process exit policy. Installation,
//! inventory scanning, and Dylint execution remain behind installer adapters.

pub mod adapters;
pub mod cli;
pub mod ports;

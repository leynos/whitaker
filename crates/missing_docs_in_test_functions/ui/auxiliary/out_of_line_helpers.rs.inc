//! Out-of-line module file for `out_of_line_module.rs`.
//!
//! The helper lives in a module file rather than inline so the module-file
//! pre-expansion run is exercised. It carries no documentation, and the
//! crate-root allow does not reach this run.

pub fn module_level_function() {}
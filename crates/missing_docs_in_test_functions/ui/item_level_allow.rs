//! The two-attribute `allow` workaround suppresses one function, not the file.
//!
//! `cfg_attr(dylint_lib = ..., allow(...))` is unavailable before expansion, so
//! the documented workaround pairs `#[allow(unknown_lints)]` with a plain
//! `#[allow(missing_docs_in_test_functions)]`. The control function below proves
//! the pair suppresses only the item it decorates.

#[allow(unknown_lints)]
#[allow(missing_docs_in_test_functions)]
fn suppressed_with_the_workaround() {}

fn undocumented_control() {}

/// Entry point, documented so it is not reported.
fn main() {}
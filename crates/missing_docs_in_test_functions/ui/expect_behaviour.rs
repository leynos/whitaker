//! Measurement fixture: what `#[expect]` does before expansion.
//!
//! RFC 0003 records that `cfg_attr(dylint_lib = ..., allow(...))` is
//! unavailable before expansion, so the supported suppression is the
//! two-attribute `allow` workaround. This fixture measures whether `#[expect]`
//! suppresses the diagnostic or instead raises `unfulfilled_lint_expectations`,
//! because that decides what the users' guide may prescribe.
//!
//! The lint is denied on the command line like every other `ui/` fixture, so a
//! diagnostic appears here as an error. Each function therefore has one of two
//! observable outcomes: silence, meaning the expectation suppressed the lint and
//! was fulfilled; or an error, either the lint itself or
//! `unfulfilled_lint_expectations`.

#![allow(dead_code, unexpected_cfgs)]

#[expect(missing_docs_in_test_functions)]
fn expect_on_plain_fn() {}

#[cfg_attr(any(), expect(missing_docs_in_test_functions))]
fn expect_via_cfg_attr() {}

#[allow(unknown_lints)]
#[allow(missing_docs_in_test_functions)]
fn allow_workaround() {}

/// Entry point, documented so it is not reported.
fn main() {}

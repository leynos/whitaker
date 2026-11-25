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

#![expect(dead_code, reason = "UI test fixture; functions are analysed but never invoked")]

#[expect(missing_docs_in_test_functions)]
fn expect_on_plain_fn() {}

// Inactive control: `any()` never holds, so this expectation is never
// installed. It records that the attribute form is accepted, nothing more.
#[cfg_attr(any(), expect(missing_docs_in_test_functions))]
fn expect_via_inactive_cfg_attr() {}

// Active case: `test` holds in this harness compilation, so the expectation is
// installed and can suppress the diagnostic. This is the plain-function
// `cfg_attr` case the users' guide tabulates.
#[cfg_attr(test, expect(missing_docs_in_test_functions))]
fn expect_via_active_cfg_attr() {}

#[allow(unknown_lints)]
#[allow(missing_docs_in_test_functions)]
fn allow_workaround() {}

/// Entry point, documented so it is not reported.
fn main() {}

//! Shared helpers for strict `rstest` test, fixture, and parameter detection.

mod detection;
mod parameter;

pub use detection::{
    ExpansionTrace, RstestDetectionOptions, is_rstest_fixture, is_rstest_fixture_with,
    is_rstest_test, is_rstest_test_with,
};
pub use parameter::{
    ParameterBinding, RstestParameter, RstestParameterKind, classify_rstest_parameter,
    fixture_local_names,
};

#[cfg(test)]
mod tests;

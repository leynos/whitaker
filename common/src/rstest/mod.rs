//! Shared helpers for strict `rstest` detection, parameter analysis, span
//! recovery, and fingerprint data models.
//!
//! The module exports detection helpers such as [`ExpansionTrace`],
//! [`RstestDetectionOptions`], [`is_rstest_fixture`], and [`is_rstest_test`];
//! parameter helpers such as [`ParameterBinding`], [`RstestParameter`],
//! [`RstestParameterKind`], [`classify_rstest_parameter`], and
//! [`fixture_local_names`]; span-recovery helpers such as
//! [`SpanRecoveryFrame`], [`UserEditableSpan`], and
//! [`recover_user_editable_span`]; and fingerprint models such as
//! [`ArgFingerprint`] and [`ParagraphFingerprint`].

mod argument_fingerprint;
mod detection;
mod paragraph_fingerprint;
mod parameter;
mod span;

pub use argument_fingerprint::{ArgAtom, ArgFingerprint};
pub use detection::{
    ExpansionTrace, RstestDetectionOptions, is_rstest_fixture, is_rstest_fixture_with,
    is_rstest_test, is_rstest_test_with,
};
pub use paragraph_fingerprint::{
    CalleeShape, ExprShape, LocalSlot, ParagraphFingerprint, ParagraphNormalizer, StmtShape,
};
pub use parameter::{
    ParameterBinding, RstestParameter, RstestParameterKind, classify_rstest_parameter,
    fixture_local_names,
};
pub use span::{SpanRecoveryFrame, UserEditableSpan, recover_user_editable_span};

#[cfg(test)]
mod tests;

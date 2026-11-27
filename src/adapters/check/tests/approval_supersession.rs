//! Which answers supersede which, and who may spend them.
//!
//! Two promises live here that the holder's own state transitions do not
//! state. An identifier is scoped to the store that issued it, because two
//! adapters both number their first answer one and a value one of them
//! approved would otherwise name the other's answer. And a readiness call that
//! refuses still supersedes the answer before it, because the caller asked
//! again and the installation did not answer: the earlier approval no longer
//! describes the machine.

use super::{
    super::{Approval, ApprovedRuns, settled},
    fixtures::{directory_with_suite, prepared_direct},
};
use crate::ports::{ApprovalId, CheckPreflightError, CheckPrepared, CheckRunError};

/// Read the approval a direct answer returned.
///
/// The direct route holds no run, so its prepared value is the only thing the
/// answer leaves behind; reading the identifier from it is how a test names
/// the answer it is about to spend.
fn direct_id(prepared: &CheckPrepared) -> ApprovalId {
    let CheckPrepared::Direct { approval, .. } = prepared else {
        panic!("a direct path must approve the direct route");
    };
    *approval
}

/// Refuse an identifier that another adapter's store issued.
///
/// Two adapters both number their first answer one, so the number alone cannot
/// tell them apart: a value one instance approved would name the other's first
/// answer and run a different installation. The issuer is part of the
/// identifier for that reason, and the refusal must leave the current answer
/// spendable.
#[test]
fn an_approval_another_store_issued_is_refused() {
    let directory = directory_with_suite();
    let first = ApprovedRuns::new();
    let second = ApprovedRuns::new();
    let from_first = direct_id(&Approval::Direct(prepared_direct(directory.path())).record(&first));
    let from_second =
        direct_id(&Approval::Direct(prepared_direct(directory.path())).record(&second));

    assert_eq!(
        from_first.number(),
        from_second.number(),
        "two fresh stores must both number their first answer one"
    );
    assert_ne!(
        from_first.issuer(),
        from_second.issuer(),
        "two stores must not share an issuer"
    );
    assert!(
        matches!(second.consume(from_first), Err(CheckRunError::Unapproved)),
        "a value the store did not issue must be refused"
    );
    assert!(
        second.consume(from_second).is_ok(),
        "refusing the foreign value must leave the store's own answer spendable"
    );
}

/// Drop the previous answer when a readiness call refuses.
///
/// The caller asked again and the installation did not answer, so the earlier
/// approval no longer describes the machine. Leaving it in place would let the
/// flow run against an installation the newest answer had already refused once
/// the defect was mended.
#[test]
fn a_refused_readiness_call_invalidates_the_previous_answer() {
    let directory = directory_with_suite();
    let held = ApprovedRuns::new();
    let previous = direct_id(&Approval::Direct(prepared_direct(directory.path())).record(&held));

    let refused = settled(Err(CheckPreflightError::UserPathHasNoSuite), &held);

    assert!(
        matches!(refused, Err(CheckPreflightError::UserPathHasNoSuite)),
        "the refusal must reach the caller unchanged"
    );
    assert!(
        matches!(held.consume(previous), Err(CheckRunError::Unapproved)),
        "the answer a refusal supersedes must no longer be spendable"
    );
}

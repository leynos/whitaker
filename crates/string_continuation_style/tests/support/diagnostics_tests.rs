//! Keeps fallback wording and configured applicability mapping explicit.

use rustc_lint::Applicability;

use crate::config::ApplicabilityMode;

#[test]
fn manual_applicability_never_enables_automated_fixes_by_default() {
    assert_eq!(
        super::applicability(ApplicabilityMode::Manual),
        Applicability::MaybeIncorrect
    );
    assert_eq!(
        super::applicability(ApplicabilityMode::Machine),
        Applicability::MachineApplicable
    );
}

#[test]
fn fallback_message_set_contains_each_diagnostic_component() {
    let messages = super::fallback_messages();

    assert_eq!(
        messages.primary(),
        "join these string fragments with `concat!()`"
    );
    assert!(messages.note().contains("physical newline"));
    assert!(messages.help().contains("source-line join"));
}

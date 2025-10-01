use crate::context::ContextSignals;
use rstest::rstest;
use rstest_bdd::{StepError, assert_step_ok};
use rstest_bdd_macros::{given, then, when};

#[given("context has_test {has_test} cfg {has_cfg} ancestor {ancestor}")]
fn context_signals(
    has_test: bool,
    has_cfg: bool,
    ancestor: bool,
) -> Result<ContextSignals, StepError> {
    Ok(ContextSignals {
        has_test_attr: has_test,
        has_cfg_test: has_cfg,
        ancestor_cfg_test: ancestor,
    })
}

#[when("the context is evaluated for test-like behaviour")]
fn evaluate_context(signals: ContextSignals) -> Result<bool, StepError> {
    Ok(signals.is_test_like())
}

#[then("the outcome should be {expected}")]
fn assert_outcome(result: bool, expected: bool) -> Result<(), StepError> {
    assert_eq!(result, expected);
    Ok(())
}

#[rstest]
#[case(true, false, false, true)]
#[case(false, true, false, true)]
#[case(false, false, true, true)]
#[case(false, false, false, false)]
fn bdd_context_signals(
    #[case] has_test: bool,
    #[case] has_cfg: bool,
    #[case] ancestor: bool,
    #[case] expected: bool,
) {
    let signals = assert_step_ok!(context_signals(has_test, has_cfg, ancestor));
    let result = assert_step_ok!(evaluate_context(signals));
    assert_step_ok!(assert_outcome(result, expected));
}

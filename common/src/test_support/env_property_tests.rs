//! Property tests for scoped environment restoration.
//!
//! Generated transitions exercise the same set, remove, and restore contract
//! across several variables at once, including repeated keys and panics.

use std::{
    ffi::OsStr,
    panic::{AssertUnwindSafe, catch_unwind},
};

use proptest::prelude::*;

use super::{with_env_var, with_env_var_removed, with_env_vars, with_env_vars_selected};

const PROPERTY_KEY: &str = "WHITAKER_TEST_SUPPORT_PROPERTY";
const PROPERTY_COMPANION_KEY: &str = "WHITAKER_TEST_SUPPORT_PROPERTY_COMPANION";

#[derive(Clone, Copy)]
enum NestedMutation {
    Set,
    Remove,
}

/// Baseline value of one property variable, where `None` means absent.
type BaselineValue = Option<&'static str>;

/// Starting environment the generated transitions must restore.
#[derive(Clone, Copy)]
struct Baseline {
    primary: BaselineValue,
    companion: BaselineValue,
}

/// Apply one baseline value, removing the key when the value is absent.
fn with_baseline_value<R>(
    key: &'static str,
    value: BaselineValue,
    callback: impl FnOnce() -> R,
) -> R {
    match value {
        Some(set_value) => with_env_var(key, set_value, callback),
        None => with_env_var_removed(key, callback),
    }
}

/// Run `callback` with both property variables at their generated baseline.
fn with_baseline<R>(baseline: Baseline, callback: impl FnOnce() -> R) -> R {
    with_baseline_value(PROPERTY_KEY, baseline.primary, || {
        with_baseline_value(PROPERTY_COMPANION_KEY, baseline.companion, callback)
    })
}

/// Check that both property variables match the generated baseline.
fn assert_baseline_restored(baseline: Baseline) -> Result<(), TestCaseError> {
    let primary = std::env::var_os(PROPERTY_KEY);
    prop_assert_eq!(primary.as_deref(), baseline.primary.map(OsStr::new));

    let companion = std::env::var_os(PROPERTY_COMPANION_KEY);
    prop_assert_eq!(companion.as_deref(), baseline.companion.map(OsStr::new));
    Ok(())
}

/// Build the settings that set every generated key to the same value.
fn settings_for(keys: &[&'static str], value: &'static str) -> Vec<(&'static str, &'static str)> {
    keys.iter().map(|key| (*key, value)).collect()
}

/// Check that every generated key currently holds `value`.
fn every_key_has(keys: &[&'static str], value: &'static str) -> bool {
    keys.iter()
        .all(|key| std::env::var_os(key).as_deref() == Some(OsStr::new(value)))
}

/// Exercise `with_env_vars` over a generated key list.
fn transition_with_env_vars_sets_every_key(keys: &[&'static str]) -> bool {
    let mut callback_ran = false;
    let applied = with_env_vars(settings_for(keys, "vars temporary"), || {
        callback_ran = true;
        every_key_has(keys, "vars temporary")
    });
    // An empty list takes the helper's lock-free short circuit, so the callback
    // must still run for the transition to be meaningful.
    callback_ran && applied
}

/// Exercise `with_env_vars_selected` over a generated key list.
fn transition_with_env_vars_selected_sets_every_key(keys: &[&'static str]) -> bool {
    let mut callback_ran = false;
    let applied = with_env_vars_selected(
        || settings_for(keys, "selected temporary"),
        || {
            callback_ran = true;
            every_key_has(keys, "selected temporary")
        },
    );
    callback_ran && applied
}

/// Exercise nested multi-variable scopes and restoration of the outer values.
fn transition_nests_multi_variable_scopes(keys: &[&'static str]) -> bool {
    with_env_vars(settings_for(keys, "vars outer"), || {
        let inner_applied = with_env_vars(settings_for(keys, "vars inner"), || {
            every_key_has(keys, "vars inner")
        });
        let outer_restored = every_key_has(keys, "vars outer");
        inner_applied && outer_restored
    })
}

/// Exercise a repeated key inside one multi-variable scope.
fn transition_keeps_the_last_repeated_value(keys: &[&'static str]) -> bool {
    let mut settings = settings_for(keys, "vars temporary");
    settings.push((PROPERTY_KEY, "repeated first"));
    settings.push((PROPERTY_KEY, "repeated second"));
    with_env_vars(settings, || {
        let repeated_kept =
            std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("repeated second"));
        let others_kept = keys
            .iter()
            .copied()
            .filter(|key| *key != PROPERTY_KEY)
            .all(|key| std::env::var_os(key).as_deref() == Some(OsStr::new("vars temporary")));
        repeated_kept && others_kept
    })
}

/// Exercise restoration after a multi-variable callback panics.
fn transition_multi_variable_restores_after_panic(keys: &[&'static str]) -> bool {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_vars(settings_for(keys, "vars temporary"), || {
            panic!("test panic")
        });
    }));
    panic.is_err()
}

/// Exercise restoration after a selected multi-variable callback panics.
fn transition_selected_multi_variable_restores_after_panic(keys: &[&'static str]) -> bool {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_vars_selected(
            || settings_for(keys, "selected temporary"),
            || panic!("test panic"),
        );
    }));
    panic.is_err()
}

/// Exercise scoped environment transitions from several starting values.
fn transition_exposes_temporary_value() -> bool {
    with_env_var(PROPERTY_KEY, "temporary", || {
        std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("temporary"))
    })
}

/// Exercise scoped removal while the generated baseline is active.
fn transition_removes_temporary_value() -> bool {
    with_env_var_removed(PROPERTY_KEY, || std::env::var_os(PROPERTY_KEY).is_none())
}

/// Exercise nested mutation and restoration of the outer value.
fn transition_restores_outer_value_after_nested_mutation(nested_mutation: NestedMutation) -> bool {
    with_env_var(PROPERTY_KEY, "outer", || {
        let inner_mutation_applied = match nested_mutation {
            NestedMutation::Set => with_env_var(PROPERTY_KEY, "inner", || {
                std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("inner"))
            }),
            NestedMutation::Remove => {
                with_env_var_removed(PROPERTY_KEY, || std::env::var_os(PROPERTY_KEY).is_none())
            }
        };
        let outer_restored = std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("outer"));
        inner_mutation_applied && outer_restored
    })
}

/// Exercise restoration after a set callback panics.
fn transition_restores_value_after_set_callback_panic() -> bool {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_var(PROPERTY_KEY, "temporary", || panic!("test panic"));
    }));
    panic.is_err()
}

/// Exercise restoration after a removal callback panics.
fn transition_restores_value_after_remove_callback_panic() -> bool {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_var_removed(PROPERTY_KEY, || panic!("test panic"));
    }));
    panic.is_err()
}

/// Apply one generated transition and report whether its own checks passed.
fn apply_transition(transition: u8, keys: &[&'static str]) -> Result<bool, TestCaseError> {
    let succeeded = match transition {
        0 => transition_exposes_temporary_value(),
        1 => transition_removes_temporary_value(),
        2 => transition_restores_outer_value_after_nested_mutation(NestedMutation::Remove),
        3 => transition_restores_outer_value_after_nested_mutation(NestedMutation::Set),
        4 => transition_restores_value_after_set_callback_panic(),
        5 => transition_restores_value_after_remove_callback_panic(),
        6 => transition_with_env_vars_sets_every_key(keys),
        7 => transition_with_env_vars_selected_sets_every_key(keys),
        8 => transition_nests_multi_variable_scopes(keys),
        9 => transition_keeps_the_last_repeated_value(keys),
        10 => transition_multi_variable_restores_after_panic(keys),
        11 => transition_selected_multi_variable_restores_after_panic(keys),
        other => {
            return Err(TestCaseError::fail(format!(
                "the transition strategy only yields values below twelve, got {other}"
            )));
        }
    };
    Ok(succeeded)
}

/// Run a generated sequence and confirm every scoped transition restores its baseline.
fn run_transitions(
    baseline: Baseline,
    keys: &[&'static str],
    transitions: &[u8],
) -> Result<(), TestCaseError> {
    for transition in transitions {
        let transition_succeeded = apply_transition(*transition, keys)?;
        prop_assert!(transition_succeeded);

        assert_baseline_restored(baseline)?;
    }
    Ok(())
}

proptest! {
    /// Check that varied scoped mutations restore the original environment values.
    #[test]
    fn scoped_environment_transitions_restore_the_initial_value(
        primary in prop_oneof![Just(None), Just(Some("first")), Just(Some("second"))],
        companion in prop_oneof![Just(None), Just(Some("companion"))],
        keys in prop::collection::vec(
            prop_oneof![Just(PROPERTY_KEY), Just(PROPERTY_COMPANION_KEY)],
            0..4,
        ),
        transitions in prop::collection::vec(0_u8..12, 1..8),
    ) {
        let baseline = Baseline { primary, companion };
        with_baseline(baseline, || run_transitions(baseline, &keys, &transitions))?;
    }
}

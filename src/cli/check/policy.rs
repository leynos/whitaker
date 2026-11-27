//! When `whitaker check` may provision Whitaker for itself.
//!
//! Only two facts decide it, and both are the user's: an explicit
//! `--no-install`, and a `DYLINT_LIBRARY_PATH` the user manages. Repair is
//! permitted only when neither applies. The rule lives here, apart from the
//! command, so the truth table below is the whole specification.

use super::flow::Policy;

/// Derive the repair policy for one invocation.
///
/// `--no-install` wins over everything: a CI job that asked for no changes to
/// its machine must not download a suite because a later rule thought it was
/// helpful. A user-managed library path is equally authoritative, because
/// installing Whitaker elsewhere cannot add a library to a directory the user
/// chose.
///
/// # Examples
///
/// ```
/// use whitaker::cli::check::policy::policy_for;
///
/// assert!(
///     policy_for(false, false).repair_allowed,
///     "the default invocation must allow repair",
/// );
/// assert!(
///     !policy_for(true, false).repair_allowed,
///     "`--no-install` must forbid repair",
/// );
/// assert!(
///     !policy_for(false, true).repair_allowed,
///     "a user-managed library path must forbid repair",
/// );
/// ```
#[must_use]
pub const fn policy_for(no_install: bool, user_managed_path: bool) -> Policy {
    Policy {
        repair_allowed: !no_install && !user_managed_path,
    }
}

#[cfg(test)]
mod tests {
    //! The truth table for `CHK-INV-POLICY`.

    use rstest::rstest;

    use super::policy_for;

    /// Permit repair only when neither veto applies to the invocation.
    #[rstest]
    #[case::default_invocation(false, false, true)]
    #[case::no_install_refuses(true, false, false)]
    #[case::user_path_refuses(false, true, false)]
    #[case::both_refuse(true, true, false)]
    fn repair_is_allowed_only_for_a_default_invocation(
        #[case] no_install: bool,
        #[case] user_managed_path: bool,
        #[case] expected: bool,
    ) {
        assert_eq!(
            policy_for(no_install, user_managed_path).repair_allowed,
            expected,
            "wrong policy for no_install={no_install}, user_managed_path={user_managed_path}"
        );
    }
}

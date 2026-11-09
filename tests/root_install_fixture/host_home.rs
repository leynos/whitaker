//! Host home directories the install child must keep visible.
//!
//! The fixture redirects the child's `HOME` so the directories the install
//! writes stay inside a temporary tree. `rustup` derives `CARGO_HOME` from
//! `HOME`, so that redirect also hides rustup's own installation: the toolchain
//! download succeeds, and then rustup aborts with
//! `rustup is not installed at '<home>/.cargo'`. Passing the host's
//! `CARGO_HOME` and `RUSTUP_HOME` through keeps rustup usable, and makes the
//! child see the toolchain that [`is_toolchain_installed`] probed in the parent
//! environment.
//!
//! [`is_toolchain_installed`]: super::is_toolchain_installed

use std::{ffi::OsString, path::PathBuf};

/// Resolve a host home variable, falling back to its conventional directory.
///
/// `rustup` and Cargo name these directories through `CARGO_HOME` and
/// `RUSTUP_HOME`, and default to `~/.cargo` and `~/.rustup` when a variable is
/// unset or empty. Returning `None` when the host names no home at all leaves
/// the child's environment untouched, so a runner without `HOME` or
/// `USERPROFILE` keeps its previous behaviour instead of gaining an empty
/// path.
pub fn host_home(variable: &str, conventional: &str) -> Option<PathBuf> {
    resolve(non_empty_os(variable), user_home(), conventional)
}

/// Apply the variable, then the conventional fallback, to the values read.
///
/// Separating the decision from the environment reads lets the precedence be
/// tested without mutating the process environment.
fn resolve(
    variable: Option<OsString>,
    home: Option<PathBuf>,
    conventional: &str,
) -> Option<PathBuf> {
    variable
        .map(PathBuf::from)
        .or_else(|| home.map(|user_home| user_home.join(conventional)))
}

/// Read a non-empty environment variable as an [`OsString`].
fn non_empty_os(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

/// Return the host's user home from either platform's variable.
fn user_home() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .find_map(non_empty_os)
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::resolve;

    /// A named variable wins even when the user's home is known.
    #[test]
    fn a_named_variable_takes_precedence() {
        let resolved = resolve(
            Some(OsString::from("/opt/cargo")),
            Some(PathBuf::from("/home/user")),
            ".cargo",
        );

        assert_eq!(resolved, Some(PathBuf::from("/opt/cargo")));
    }

    /// An unset variable falls back to the conventional directory under home.
    #[test]
    fn an_absent_variable_falls_back_to_the_users_home() {
        let home = PathBuf::from("/home/user");
        let expected = home.join(".rustup");

        assert_eq!(
            resolve(None, Some(home), ".rustup").as_deref(),
            Some(expected.as_path())
        );
    }

    /// A host naming neither a variable nor a home leaves the child alone.
    #[test]
    fn no_variable_and_no_home_resolve_to_nothing() {
        assert_eq!(resolve(None, None, ".cargo"), None);
    }
}

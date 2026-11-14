//! Pinning a Git command to the repository the caller names.
//!
//! `-C` and `current_dir` only choose a starting directory. Repository
//! selection variables are applied afterwards and outrank both, so every
//! adapter that must read one specific checkout drains them first.

use std::process::Command;

/// Inherited variables that choose which repository Git reads or where its
/// objects come from (from `git rev-parse --local-env-vars`).
///
/// Left in place, `GIT_DIR` or `GIT_ALTERNATE_OBJECT_DIRECTORIES` can point a
/// command at another repository, or supply objects from outside the one under
/// inspection. The list lives here rather than beside a single caller so the
/// adapters cannot drift apart. Configuration variables such as
/// `GIT_CONFIG_COUNT` stay: authenticated clones pass their header that way.
/// `GIT_NO_REPLACE_OBJECTS` stays too, since it only ever disables replacement.
pub(crate) const REPOSITORY_SELECTION_VARIABLES: [&str; 11] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_INDEX_FILE",
    "GIT_GRAFT_FILE",
    "GIT_SHALLOW_FILE",
    "GIT_REPLACE_REF_BASE",
    "GIT_NAMESPACE",
];

/// Drain every repository selector from a command's environment.
pub(crate) fn clear_repository_selectors(command: &mut Command) {
    for name in REPOSITORY_SELECTION_VARIABLES {
        command.env_remove(name);
    }
}

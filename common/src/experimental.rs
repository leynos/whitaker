//! Per-lint opt-in for experimental Whitaker lints.
//!
//! Every experimental lint ships compiled into the prebuilt suite, but none
//! runs until a repository names it. The list lives in the repository's
//! `dylint.toml`, which is the contract the Dylint driver reads:
//!
//! ```toml
//! [whitaker]
//! experimental = ["macro_aware_cognitive_complexity"]
//! ```
//!
//! This module is the single validator for that list. It is pure: it takes
//! names and returns a selection or an error, and it depends on neither
//! `rustc`, `serde`, nor any configuration loader. The suite calls it at
//! registration time; the installer and the forthcoming `whitaker` command
//! can call the same function over a list taken from any source (for example
//! `whitaker.toml`), so every consumer accepts and rejects the same names.
//!
//! Scope and reuse policy: this module owns the vocabulary of lifecycle
//! stages (experimental, promoted) and name validation. It must not read
//! files, touch the environment, or print. Callers own I/O and presentation.
//!
//! The stages are documented in the developers' guide under "Lint
//! lifecycle"; the decision is recorded in ADR 0014.

use std::collections::BTreeSet;

use thiserror::Error;

/// Lints that ship in the suite but run only when a repository opts in.
///
/// Names are the canonical lint names, which equal the lint crate names.
pub const EXPERIMENTAL_LINTS: &[&str] = &[
    "macro_aware_cognitive_complexity",
    "rstest_helper_should_be_fixture",
    "string_continuation_style",
];

/// Lints that were experimental and now run by default.
///
/// A repository may still list them in `[whitaker] experimental`; the entry is
/// accepted and reported as redundant, so promotion never breaks a consumer's
/// configuration.
pub const PROMOTED_LINTS: &[&str] = &[];

/// Failure to resolve an experimental opt-in list.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error(
    "unknown experimental lint(s) in `[whitaker] experimental`: {}; known \
     experimental lints: {}",
    unknown.join(", "),
    known.join(", ")
)]
pub struct ExperimentalOptInError {
    /// Names that are neither experimental nor promoted, in request order.
    pub unknown: Vec<String>,
    /// Names the caller could have used.
    pub known: Vec<&'static str>,
}

/// The experimental lints a repository has chosen to run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExperimentalSelection {
    enabled: BTreeSet<&'static str>,
    redundant: Vec<&'static str>,
}

impl ExperimentalSelection {
    /// Selects no experimental lint, the default for a repository that has not
    /// opted in.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::experimental::ExperimentalSelection;
    ///
    /// assert!(!ExperimentalSelection::none().contains("string_continuation_style"));
    /// ```
    #[must_use]
    pub fn none() -> Self { Self::default() }

    /// Selects every experimental lint, as a local `--experimental` build does.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::experimental::ExperimentalSelection;
    ///
    /// assert!(ExperimentalSelection::all().contains("string_continuation_style"));
    /// ```
    #[must_use]
    pub fn all() -> Self {
        Self {
            enabled: EXPERIMENTAL_LINTS.iter().copied().collect(),
            redundant: Vec::new(),
        }
    }

    /// Reports whether the named experimental lint is selected.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool { self.enabled.contains(name) }

    /// Iterates over the selected experimental lints in name order.
    pub fn enabled(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.enabled.iter().copied()
    }

    /// Names listed although the lint has been promoted and now runs by
    /// default, in request order without duplicates.
    #[must_use]
    pub fn redundant(&self) -> &[&'static str] { &self.redundant }

    /// Records a redundant entry once, in request order.
    fn note_redundant(&mut self, name: &'static str) {
        if !self.redundant.contains(&name) {
            self.redundant.push(name);
        }
    }

    /// Adds every lint selected by `other`, keeping both redundancy notes.
    #[must_use]
    pub fn union(mut self, other: &Self) -> Self {
        self.enabled.extend(other.enabled.iter().copied());
        other
            .redundant
            .iter()
            .for_each(|name| self.note_redundant(name));
        self
    }
}

/// Resolves a requested opt-in list against the current lifecycle stages.
///
/// Duplicates collapse. A promoted lint is accepted and recorded as redundant.
/// Any other unknown name fails, and the error names every unknown entry so a
/// repository fixes its configuration in one pass rather than one typo at a
/// time. Unknown names are never ignored: a silently dropped entry would leave
/// a repository believing a lint ran when it did not.
///
/// # Errors
///
/// Returns [`ExperimentalOptInError`] when any name is neither experimental
/// nor promoted.
///
/// # Examples
///
/// ```
/// use whitaker_common::experimental::resolve;
///
/// let selection = resolve(&["macro_aware_cognitive_complexity"]).expect("known lint");
/// assert!(selection.contains("macro_aware_cognitive_complexity"));
///
/// let error = resolve(&["no_such_lint"]).expect_err("unknown lint");
/// assert_eq!(error.unknown, ["no_such_lint"]);
/// ```
pub fn resolve<S: AsRef<str>>(
    requested: &[S],
) -> Result<ExperimentalSelection, ExperimentalOptInError> {
    resolve_against(requested, EXPERIMENTAL_LINTS, PROMOTED_LINTS)
}

/// Resolves a requested opt-in list against explicit stage tables.
///
/// [`resolve`] calls this with the shipped tables. Callers that carry their own
/// tables use it directly, chiefly tests, which can exercise the promotion path
/// before any real lint has been promoted.
///
/// # Errors
///
/// Returns [`ExperimentalOptInError`] when any name is in neither table.
///
/// # Examples
///
/// ```
/// use whitaker_common::experimental::resolve_against;
///
/// let selection =
///     resolve_against(&["alpha", "gamma"], &["alpha"], &["gamma"]).expect("both names are known");
/// assert!(selection.contains("alpha"));
/// assert_eq!(selection.redundant(), ["gamma"]);
/// ```
pub fn resolve_against<S: AsRef<str>>(
    requested: &[S],
    experimental: &'static [&'static str],
    promoted: &'static [&'static str],
) -> Result<ExperimentalSelection, ExperimentalOptInError> {
    let mut selection = ExperimentalSelection::none();
    let mut unknown: Vec<String> = Vec::new();

    for name in requested.iter().map(AsRef::as_ref) {
        match classify(name, experimental, promoted) {
            Stage::Experimental(known) => {
                selection.enabled.insert(known);
            }
            Stage::Promoted(known) => selection.note_redundant(known),
            Stage::Unknown => push_unique(&mut unknown, name),
        }
    }

    if unknown.is_empty() {
        Ok(selection)
    } else {
        Err(ExperimentalOptInError {
            unknown,
            known: experimental.iter().chain(promoted).copied().collect(),
        })
    }
}

/// Where a requested name sits in the lint lifecycle.
enum Stage {
    /// Ships but runs only on opt-in.
    Experimental(&'static str),
    /// Runs by default now; an opt-in entry is redundant.
    Promoted(&'static str),
    /// Neither.
    Unknown,
}

/// Looks `name` up in the experimental table, then the promoted one.
fn classify(
    name: &str,
    experimental: &'static [&'static str],
    promoted: &'static [&'static str],
) -> Stage {
    let find = |table: &'static [&'static str]| table.iter().copied().find(|known| *known == name);
    find(experimental).map_or_else(
        || find(promoted).map_or(Stage::Unknown, Stage::Promoted),
        Stage::Experimental,
    )
}

/// Appends `name` unless it is already present.
fn push_unique(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|seen| seen == name) {
        names.push(name.to_owned());
    }
}

#[cfg(test)]
#[path = "experimental_tests.rs"]
mod tests;

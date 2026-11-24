//! The consumer crates the opt-in smoke test lints.
//!
//! Each consumer is one `OptIn` state: what its `dylint.toml` says about the
//! experimental lint, and what its source carries. They differ only there, so a
//! difference in the Dylint run is attributable to the opt-in alone.

use super::LINT_NAME;

/// What a consumer's `dylint.toml` says about the experimental lint.
#[derive(Clone, Copy)]
pub(super) enum OptIn {
    /// Lists the lint under `[whitaker] experimental`.
    Listed,
    /// Configures the lint's threshold but never lists it.
    Unlisted,
    /// As `Unlisted`, with a leftover `#[expect]` instead of an `#[allow]`.
    UnlistedWithExpect,
    /// As `Unlisted`, run against the `experimental-enable-all` suite.
    EnableAll,
    /// Lists a name that is not a lint.
    Misspelt,
}

impl OptIn {
    /// Directory name under the state directory for this consumer.
    pub(super) const fn directory(self) -> &'static str {
        match self {
            Self::Listed => "listed",
            Self::Unlisted => "unlisted",
            Self::UnlistedWithExpect => "unlisted-expect",
            Self::EnableAll => "enable-all",
            Self::Misspelt => "misspelt",
        }
    }

    /// The consumer's `lib.rs`. A consumer that has not opted in also carries a
    /// leftover allowance, which must stay silent, or a leftover expectation,
    /// which is reported as unfulfilled.
    pub(super) fn source(self) -> String {
        match self {
            Self::Unlisted => format!("{CONSUMER_SOURCE}{LEFTOVER_ALLOW_SOURCE}"),
            Self::UnlistedWithExpect => format!("{CONSUMER_SOURCE}{LEFTOVER_EXPECT_SOURCE}"),
            Self::Listed | Self::EnableAll | Self::Misspelt => CONSUMER_SOURCE.to_owned(),
        }
    }

    /// The consumer's `dylint.toml`.
    pub(super) fn dylint_config(self) -> String {
        let threshold = CONSUMER_DYLINT_CONFIG;
        match self {
            Self::Listed => format!("[whitaker]\nexperimental = [\"{LINT_NAME}\"]\n\n{threshold}"),
            Self::Unlisted | Self::UnlistedWithExpect | Self::EnableAll => threshold.to_owned(),
            Self::Misspelt => {
                format!("[whitaker]\nexperimental = [\"{LINT_NAME}_typo\"]\n\n{threshold}")
            }
        }
    }
}

/// A dependency-free consumer with one authored branch.
pub(super) const CONSUMER_MANIFEST: &str = concat!(
    "[package]\n",
    "name = \"macro_aware_smoke_consumer\"\n",
    "version = \"0.0.0\"\n",
    "edition = \"2021\"\n",
    "\n",
    "[workspace]\n",
);

/// Consumer source whose single `if` scores one, above a zero limit.
pub(super) const CONSUMER_SOURCE: &str = concat!(
    "//! Smoke consumer for the macro-aware cognitive complexity lint.\n",
    "\n",
    "/// Contains one authored branch, which exceeds a zero limit.\n",
    "pub fn authored_if(flag: bool) {\n",
    "    if flag {}\n",
    "}\n",
);

/// Extra source for a consumer that has not opted in: a leftover allowance
/// naming the lint. If the unselected lint were not declared, rustc would
/// report `unknown_lints` here and `-D warnings` would fail the run.
const LEFTOVER_ALLOW_SOURCE: &str = concat!(
    "\n",
    "/// Carries an allowance for a lint this repository has not opted in to.\n",
    "#[allow(macro_aware_cognitive_complexity)]\n",
    "pub fn leftover_allow(flag: bool) {\n",
    "    if flag {}\n",
    "}\n",
);

/// Extra source for a consumer that expects a lint it has not opted in to. The
/// lint never fires, so rustc reports the expectation as unfulfilled.
const LEFTOVER_EXPECT_SOURCE: &str = concat!(
    "\n",
    "/// Carries an expectation for a lint this repository has not opted in to.\n",
    "#[expect(macro_aware_cognitive_complexity)]\n",
    "pub fn leftover_expect(flag: bool) {\n",
    "    if flag {}\n",
    "}\n",
);

/// Consumer configuration that makes any authored branch a finding.
const CONSUMER_DYLINT_CONFIG: &str = concat!(
    "[macro_aware_cognitive_complexity]\n",
    "max_complexity = 0\n"
);

//! Configuration and input bundle for the AST pass.

use std::num::NonZeroUsize;

use crate::{
    AcceptedPair,
    CandidatePair,
    SimilarityThreshold,
    TokenFragment,
    run0::TokenPassConfig,
};

/// Default minimum normalized-node count for conclusive AST evidence.
pub const DEFAULT_MIN_NODES: usize = 10;

/// Default maximum covering-span expansion factor.
pub const DEFAULT_MAX_COVER_EXPANSION: usize = 2;

/// Numerator of the default Type-3 cosine threshold (`17/20`).
///
/// K1 measured the shipped trigram cosine and recorded its distribution in
/// the `ExecPlan`: the near-miss floor is above this value, while some
/// structurally similar but unrelated pairs also exceed it. Changing the
/// value would also require reordering the Type-2 threshold, which is out of
/// scope, so the approved default stands pending maintainer review.
const DEFAULT_TYPE3_NUMERATOR: usize = 17;

/// Denominator of the default Type-3 cosine threshold (`17/20`).
const DEFAULT_TYPE3_DENOMINATOR: usize = 20;

/// Immutable configuration for AST-pass refinement and Run 1 emission.
///
/// Defaults are `type3 = 17/20`, `min_nodes = 10` and
/// `max_cover_expansion = 2`. The producer name is not configuration: the
/// emitter always labels Run 1 with its own constant, so a caller cannot
/// mislabel it, and this domain type carries no SARIF vocabulary.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{AstPassConfig, SimilarityThreshold};
///
/// let config = AstPassConfig::new("0.3.0")
///     .with_type3_threshold(SimilarityThreshold::new("type3", 9, 10).expect("valid threshold"));
///
/// assert_eq!(config.type3_threshold().numerator(), 9);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstPassConfig {
    tool_version: String,
    type3_threshold: SimilarityThreshold,
    min_nodes: usize,
    max_cover_expansion: usize,
}

impl AstPassConfig {
    /// Creates a configuration with the design-document defaults.
    #[must_use]
    pub fn new(tool_version: impl Into<String>) -> Self {
        Self {
            tool_version: tool_version.into(),
            type3_threshold: SimilarityThreshold::new_unchecked(
                "type3",
                DEFAULT_TYPE3_NUMERATOR,
                DEFAULT_TYPE3_DENOMINATOR,
            ),
            min_nodes: DEFAULT_MIN_NODES,
            max_cover_expansion: DEFAULT_MAX_COVER_EXPANSION,
        }
    }

    /// Overrides the Type-3 cosine threshold.
    #[must_use]
    pub const fn with_type3_threshold(mut self, threshold: SimilarityThreshold) -> Self {
        self.type3_threshold = threshold;
        self
    }

    /// Overrides the minimum normalized-node count for conclusive evidence.
    #[must_use]
    pub const fn with_min_nodes(mut self, min_nodes: NonZeroUsize) -> Self {
        self.min_nodes = min_nodes.get();
        self
    }

    /// Overrides the maximum covering-span expansion factor.
    ///
    /// A fragment whose lowered covering node is more than this many times
    /// longer than the fragment itself is treated as inconclusive, because the
    /// comparison then measures enclosing context rather than the fragment.
    #[must_use]
    pub const fn with_max_cover_expansion(mut self, factor: NonZeroUsize) -> Self {
        self.max_cover_expansion = factor.get();
        self
    }

    /// Returns the SARIF producer version.
    #[must_use]
    pub const fn tool_version(&self) -> &str { self.tool_version.as_str() }

    /// Returns the Type-3 cosine threshold.
    #[must_use]
    pub const fn type3_threshold(&self) -> SimilarityThreshold { self.type3_threshold }

    /// Returns the minimum normalized-node count for conclusive evidence.
    #[must_use]
    pub const fn min_nodes(&self) -> usize { self.min_nodes }

    /// Returns the maximum covering-span expansion factor.
    #[must_use]
    pub const fn max_cover_expansion(&self) -> usize { self.max_cover_expansion }
}

/// Everything the AST pass needs to refine the token pass's results.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{RefinementInput, TokenPassConfig};
///
/// let token_config = TokenPassConfig::new("whitaker_clones_cli@token", "0.3.0");
/// let input = RefinementInput::new(&[], &[], &[], &token_config);
///
/// assert!(input.fragments().is_empty());
/// ```
#[derive(Clone, Copy, Debug)]
pub struct RefinementInput<'a> {
    fragments: &'a [TokenFragment],
    candidates: &'a [CandidatePair],
    accepted: &'a [AcceptedPair],
    token_config: &'a TokenPassConfig,
}

impl<'a> RefinementInput<'a> {
    /// Bundles the token pass's outputs for refinement.
    #[must_use]
    pub const fn new(
        fragments: &'a [TokenFragment],
        candidates: &'a [CandidatePair],
        accepted: &'a [AcceptedPair],
        token_config: &'a TokenPassConfig,
    ) -> Self {
        Self {
            fragments,
            candidates,
            accepted,
            token_config,
        }
    }

    /// Returns the token-pass fragments.
    #[must_use]
    pub const fn fragments(&self) -> &'a [TokenFragment] { self.fragments }

    /// Returns the LSH candidate pairs.
    #[must_use]
    pub const fn candidates(&self) -> &'a [CandidatePair] { self.candidates }

    /// Returns the accepted token-pass pairs.
    #[must_use]
    pub const fn accepted(&self) -> &'a [AcceptedPair] { self.accepted }

    /// Returns the token-pass configuration.
    #[must_use]
    pub const fn token_config(&self) -> &'a TokenPassConfig { self.token_config }
}

#[cfg(test)]
mod tests {
    //! Configuration default and override tests.

    use super::*;

    /// Check that the design defaults are applied.
    #[test]
    fn defaults_follow_the_design() {
        let config = AstPassConfig::new("0.3.0");

        assert_eq!(
            (
                config.tool_version(),
                config.type3_threshold().numerator(),
                config.type3_threshold().denominator(),
                config.min_nodes(),
                config.max_cover_expansion(),
            ),
            ("0.3.0", 17, 20, 10, 2),
            "the configuration defaults must match the design document"
        );
    }

    /// Check that overrides replace the defaults.
    #[test]
    fn overrides_replace_the_defaults() {
        let Some(min_nodes) = NonZeroUsize::new(4) else {
            panic!("four is non-zero");
        };
        let Some(expansion) = NonZeroUsize::new(3) else {
            panic!("three is non-zero");
        };
        let threshold = match SimilarityThreshold::new("type3", 9, 10) {
            Ok(threshold) => threshold,
            Err(error) => panic!("nine tenths is a valid threshold: {error}"),
        };
        let config = AstPassConfig::new("1.0.0")
            .with_min_nodes(min_nodes)
            .with_max_cover_expansion(expansion)
            .with_type3_threshold(threshold);

        assert_eq!(config.min_nodes(), 4, "the node floor must be overridable");
        assert_eq!(
            config.max_cover_expansion(),
            3,
            "the expansion factor must be overridable"
        );
        assert_eq!(
            config.type3_threshold().numerator(),
            9,
            "the Type-3 threshold must be overridable"
        );
    }
}

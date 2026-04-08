//! Public input and output types for token-pass Run 0 emission.

use crate::{CandidatePair, Fingerprint, FragmentId, NormProfile};

use super::score::{SimilarityRatio, SimilarityThreshold};

const DEFAULT_SHINGLE_SIZE: usize = 25;
const DEFAULT_WINNOW_WINDOW: usize = 16;

/// A token-pass fragment ready for acceptance scoring and SARIF emission.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{Fingerprint, FragmentId, NormProfile, TokenFragment};
///
/// let fragment = TokenFragment::new(
///     FragmentId::from("src/lib.rs:0..12:T1"),
///     NormProfile::T1,
///     "src/lib.rs",
///     "fn demo() {}\n",
/// )
/// .with_retained_fingerprints(vec![Fingerprint::new(7, 0..11)]);
///
/// assert_eq!(fragment.profile(), NormProfile::T1);
/// assert_eq!(fragment.file_uri(), "src/lib.rs");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenFragment {
    id: FragmentId,
    profile: NormProfile,
    file_uri: String,
    source_text: String,
    retained_fingerprints: Vec<Fingerprint>,
}

impl TokenFragment {
    /// Creates a token fragment from stable identity and source text.
    #[must_use]
    pub fn new(
        id: FragmentId,
        profile: NormProfile,
        file_uri: impl Into<String>,
        source_text: impl Into<String>,
    ) -> Self {
        Self {
            id,
            profile,
            file_uri: file_uri.into(),
            source_text: source_text.into(),
            retained_fingerprints: Vec::new(),
        }
    }

    /// Replaces the retained fingerprints stored for this fragment.
    #[must_use]
    pub fn with_retained_fingerprints(mut self, retained_fingerprints: Vec<Fingerprint>) -> Self {
        self.retained_fingerprints = retained_fingerprints;
        self
    }

    /// Returns the stable fragment identifier.
    #[must_use]
    pub const fn id(&self) -> &FragmentId {
        &self.id
    }

    /// Returns the normalization profile used to produce this fragment.
    #[must_use]
    pub const fn profile(&self) -> NormProfile {
        self.profile
    }

    /// Returns the source artifact URI used in SARIF output.
    #[must_use]
    pub fn file_uri(&self) -> &str {
        self.file_uri.as_str()
    }

    /// Returns the original source text used for byte-range mapping.
    #[must_use]
    pub fn source_text(&self) -> &str {
        self.source_text.as_str()
    }

    /// Returns the retained token fingerprints for this fragment.
    #[must_use]
    pub fn retained_fingerprints(&self) -> &[Fingerprint] {
        self.retained_fingerprints.as_slice()
    }
}

/// Immutable configuration for token-pass acceptance and Run 0 emission.
///
/// Defaults follow the design document: `k = 25`, `window = 16`,
/// `type1 = 19/20`, and `type2 = 9/10`.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{SimilarityThreshold, TokenPassConfig};
///
/// let config = TokenPassConfig::new("whitaker_clones_cli@token", "0.2.1")
///     .with_type1_threshold(SimilarityThreshold::new("type1", 1, 1).expect("valid threshold"));
///
/// assert_eq!(config.shingle_size(), 25);
/// assert_eq!(config.tool_name(), "whitaker_clones_cli@token");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenPassConfig {
    tool_name: String,
    tool_version: String,
    shingle_size: usize,
    winnow_window: usize,
    type1_threshold: SimilarityThreshold,
    type2_threshold: SimilarityThreshold,
}

impl TokenPassConfig {
    /// Creates a configuration with the design-document defaults.
    #[must_use]
    pub fn new(tool_name: impl Into<String>, tool_version: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            tool_version: tool_version.into(),
            shingle_size: DEFAULT_SHINGLE_SIZE,
            winnow_window: DEFAULT_WINNOW_WINDOW,
            type1_threshold: SimilarityThreshold::new_unchecked("type1", 19, 20),
            type2_threshold: SimilarityThreshold::new_unchecked("type2", 9, 10),
        }
    }

    /// Overrides the configured shingle size recorded in Whitaker properties.
    #[must_use]
    pub const fn with_shingle_size(mut self, shingle_size: usize) -> Self {
        self.shingle_size = shingle_size;
        self
    }

    /// Overrides the configured winnow window recorded in Whitaker properties.
    #[must_use]
    pub const fn with_winnow_window(mut self, winnow_window: usize) -> Self {
        self.winnow_window = winnow_window;
        self
    }

    /// Overrides the Type-1 Jaccard acceptance threshold.
    #[must_use]
    pub const fn with_type1_threshold(mut self, threshold: SimilarityThreshold) -> Self {
        self.type1_threshold = threshold;
        self
    }

    /// Overrides the Type-2 Jaccard acceptance threshold.
    #[must_use]
    pub const fn with_type2_threshold(mut self, threshold: SimilarityThreshold) -> Self {
        self.type2_threshold = threshold;
        self
    }

    /// Returns the SARIF producer name.
    #[must_use]
    pub fn tool_name(&self) -> &str {
        self.tool_name.as_str()
    }

    /// Returns the SARIF producer version.
    #[must_use]
    pub fn tool_version(&self) -> &str {
        self.tool_version.as_str()
    }

    /// Returns the configured shingle size.
    #[must_use]
    pub const fn shingle_size(&self) -> usize {
        self.shingle_size
    }

    /// Returns the configured winnow window.
    #[must_use]
    pub const fn winnow_window(&self) -> usize {
        self.winnow_window
    }

    /// Returns the Type-1 acceptance threshold.
    #[must_use]
    pub const fn type1_threshold(&self) -> SimilarityThreshold {
        self.type1_threshold
    }

    /// Returns the Type-2 acceptance threshold.
    #[must_use]
    pub const fn type2_threshold(&self) -> SimilarityThreshold {
        self.type2_threshold
    }
}

/// An accepted token-pass candidate pair and its final rule classification.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{
///     AcceptedPair, CandidatePair, FragmentId, NormProfile, SimilarityRatio,
/// };
///
/// let pair = CandidatePair::new(FragmentId::from("alpha"), FragmentId::from("beta"))
///     .expect("distinct fragments");
/// let accepted = AcceptedPair::new(
///     pair,
///     NormProfile::T1,
///     SimilarityRatio::new(4, 4),
/// );
///
/// assert_eq!(accepted.profile(), NormProfile::T1);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedPair {
    pair: CandidatePair,
    profile: NormProfile,
    score: SimilarityRatio,
}

impl AcceptedPair {
    /// Creates an accepted pair with its resolved rule profile and score.
    #[must_use]
    pub const fn new(pair: CandidatePair, profile: NormProfile, score: SimilarityRatio) -> Self {
        Self {
            pair,
            profile,
            score,
        }
    }

    /// Returns the canonical fragment pair.
    #[must_use]
    pub const fn pair(&self) -> &CandidatePair {
        &self.pair
    }

    /// Returns the rule profile to emit for this pair.
    #[must_use]
    pub const fn profile(&self) -> NormProfile {
        self.profile
    }

    /// Returns the accepted Jaccard score.
    #[must_use]
    pub const fn score(&self) -> SimilarityRatio {
        self.score
    }
}

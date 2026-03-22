//! Shared MinHash and LSH index types.

use std::{fmt, num::NonZeroUsize, slice::ChunksExact};

use super::{IndexError, IndexResult};

/// The fixed MinHash sketch width for roadmap item 7.2.2.
pub const MINHASH_SIZE: usize = 128;

/// Opaque fragment identifier used by candidate generation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FragmentId(String);

impl FragmentId {
    /// Creates a new fragment identifier.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_clones_core::FragmentId;
    ///
    /// let id = FragmentId::new("src/lib.rs:10..20");
    /// assert_eq!(id.as_str(), "src/lib.rs:10..20");
    /// ```
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the fragment identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Consumes the identifier and returns the owned string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_clones_core::FragmentId;
    ///
    /// let id = FragmentId::from("fragment-a");
    /// assert_eq!(id.into_inner(), "fragment-a".to_owned());
    /// ```
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<&str> for FragmentId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FragmentId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for FragmentId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FragmentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A canonical fragment pair emitted by the LSH candidate filter.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CandidatePair {
    left: FragmentId,
    right: FragmentId,
}

impl CandidatePair {
    /// Creates a canonical fragment pair or returns `None` for a self-pair.
    ///
    /// The pair is sorted lexically so callers receive stable ordering even
    /// when fragments collide in different insertion orders.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_clones_core::{CandidatePair, FragmentId};
    ///
    /// let pair = CandidatePair::new(FragmentId::from("beta"), FragmentId::from("alpha"));
    /// assert_eq!(
    ///     pair.map(|pair| (pair.left().as_str().to_owned(), pair.right().as_str().to_owned())),
    ///     Some(("alpha".to_owned(), "beta".to_owned()))
    /// );
    /// ```
    #[must_use]
    pub fn new(left: FragmentId, right: FragmentId) -> Option<Self> {
        if left == right {
            return None;
        }
        if left < right {
            return Some(Self { left, right });
        }
        Some(Self {
            left: right,
            right: left,
        })
    }

    /// Returns the left fragment identifier.
    #[must_use]
    pub const fn left(&self) -> &FragmentId {
        &self.left
    }

    /// Returns the right fragment identifier.
    #[must_use]
    pub const fn right(&self) -> &FragmentId {
        &self.right
    }
}

/// Validated LSH settings for the fixed-width MinHash sketch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LshConfig {
    bands: NonZeroUsize,
    rows: NonZeroUsize,
}

impl LshConfig {
    /// Creates a validated LSH configuration.
    ///
    /// The product of `bands * rows` must equal the fixed
    /// [`MINHASH_SIZE`][crate::MINHASH_SIZE].
    ///
    /// # Errors
    ///
    /// Returns [`IndexError::ZeroBands`], [`IndexError::ZeroRows`], or
    /// [`IndexError::InvalidBandRowProduct`] when the inputs do not satisfy the
    /// fixed-width sketch invariant.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_clones_core::{LshConfig, MINHASH_SIZE};
    ///
    /// let config = LshConfig::new(1, MINHASH_SIZE)?;
    /// assert_eq!(config.bands(), 1);
    /// assert_eq!(config.rows(), MINHASH_SIZE);
    /// # Ok::<(), whitaker_clones_core::IndexError>(())
    /// ```
    pub fn new(bands: usize, rows: usize) -> IndexResult<Self> {
        let Some(bands) = NonZeroUsize::new(bands) else {
            return Err(IndexError::ZeroBands);
        };
        let Some(rows) = NonZeroUsize::new(rows) else {
            return Err(IndexError::ZeroRows);
        };
        validate_product(bands, rows)?;
        Ok(Self { bands, rows })
    }

    /// Returns the number of LSH bands.
    #[must_use]
    pub const fn bands(self) -> usize {
        self.bands.get()
    }

    /// Returns the number of rows in each band.
    #[must_use]
    pub const fn rows(self) -> usize {
        self.rows.get()
    }
}

fn validate_product(bands: NonZeroUsize, rows: NonZeroUsize) -> IndexResult<()> {
    match bands.get().checked_mul(rows.get()) {
        Some(MINHASH_SIZE) => Ok(()),
        Some(_) | None => Err(IndexError::invalid_band_row_product(
            bands.get(),
            rows.get(),
        )),
    }
}

/// A fixed-width MinHash sketch over retained fingerprint hashes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinHashSignature([u64; MINHASH_SIZE]);

impl MinHashSignature {
    #[must_use]
    pub(crate) const fn new(values: [u64; MINHASH_SIZE]) -> Self {
        Self(values)
    }

    /// Returns the sketch values in order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_clones_core::{Fingerprint, MinHasher};
    ///
    /// let hasher = MinHasher::new();
    /// let signature = hasher.sketch(&[
    ///     Fingerprint::new(11, 0..1),
    ///     Fingerprint::new(22, 1..2),
    /// ])?;
    /// assert_eq!(signature.values().len(), 128);
    /// # Ok::<(), whitaker_clones_core::IndexError>(())
    /// ```
    #[must_use]
    pub const fn values(&self) -> &[u64; MINHASH_SIZE] {
        &self.0
    }

    pub(crate) fn bands(&self, rows: usize) -> ChunksExact<'_, u64> {
        self.0.chunks_exact(rows)
    }
}

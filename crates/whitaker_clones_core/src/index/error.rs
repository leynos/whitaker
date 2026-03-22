//! Error types for MinHash sketching and LSH configuration.

use thiserror::Error;

use super::MINHASH_SIZE;

/// Result alias for index operations.
pub type IndexResult<T> = std::result::Result<T, IndexError>;

/// Errors raised while configuring LSH or sketching fragment fingerprints.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum IndexError {
    /// The number of bands must be greater than zero.
    #[error("LSH bands must be greater than zero")]
    ZeroBands,
    /// The number of rows per band must be greater than zero.
    #[error("LSH rows must be greater than zero")]
    ZeroRows,
    /// The band and row product must equal the fixed MinHash sketch size.
    #[error("LSH bands ({bands}) multiplied by rows ({rows}) must equal {expected}")]
    InvalidBandRowProduct {
        /// Requested number of bands.
        bands: usize,
        /// Requested number of rows per band.
        rows: usize,
        /// Required fixed MinHash sketch size.
        expected: usize,
    },
    /// MinHash requires at least one retained fingerprint hash.
    #[error("retained fingerprints must not be empty")]
    EmptyFingerprintSet,
}

impl IndexError {
    /// Builds the product-mismatch error for the fixed 7.2.2 sketch size.
    #[must_use]
    pub const fn invalid_band_row_product(bands: usize, rows: usize) -> Self {
        Self::InvalidBandRowProduct {
            bands,
            rows,
            expected: MINHASH_SIZE,
        }
    }
}

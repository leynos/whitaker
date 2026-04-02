//! Observable vector-algebra seams for decomposition advice tests.

use crate::MethodProfile;
use crate::decomposition_advice::{build_feature_vector, dot_product};

/// Observable runtime vector-algebra results for two methods.
///
/// # Examples
///
/// ```ignore
/// use common::test_support::decomposition::{MethodInput, method_vector_algebra, profile};
///
/// let left = profile(MethodInput {
///     name: "parse_tokens",
///     fields: &["grammar"],
///     signature_types: &[],
///     local_types: &[],
///     domains: &[],
/// });
/// let right = profile(MethodInput {
///     name: "parse_nodes",
///     fields: &["grammar"],
///     signature_types: &[],
///     local_types: &[],
///     domains: &[],
/// });
///
/// let report = method_vector_algebra(&left, &right);
/// assert_eq!(report.left_dot_right(), report.right_dot_left());
/// assert!(report.left_norm_squared() > 0);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodVectorAlgebraReport {
    left_dot_right: u64,
    right_dot_left: u64,
    left_norm_squared: u64,
    right_norm_squared: u64,
}

impl MethodVectorAlgebraReport {
    /// Returns the result of [`MethodVectorAlgebraReport::left_dot_right`].
    ///
    /// This is the dot product of the left and right method vectors.
    ///
    /// ```rust
    /// use common::test_support::decomposition::{MethodInput, method_vector_algebra, profile};
    ///
    /// let left = profile(MethodInput {
    ///     name: "parse_tokens",
    ///     fields: &["grammar"],
    ///     signature_types: &[],
    ///     local_types: &[],
    ///     domains: &[],
    /// });
    /// let right = profile(MethodInput {
    ///     name: "parse_nodes",
    ///     fields: &["grammar"],
    ///     signature_types: &[],
    ///     local_types: &[],
    ///     domains: &[],
    /// });
    ///
    /// let report = method_vector_algebra(&left, &right);
    /// assert_eq!(report.left_dot_right(), 40);
    /// ```
    #[must_use]
    pub fn left_dot_right(self) -> u64 {
        self.left_dot_right
    }

    /// Returns the result of [`MethodVectorAlgebraReport::right_dot_left`].
    ///
    /// This is the dot product of the right and left method vectors.
    ///
    /// ```rust
    /// use common::test_support::decomposition::{MethodInput, method_vector_algebra, profile};
    ///
    /// let left = profile(MethodInput {
    ///     name: "parse_tokens",
    ///     fields: &["grammar"],
    ///     signature_types: &[],
    ///     local_types: &[],
    ///     domains: &[],
    /// });
    /// let right = profile(MethodInput {
    ///     name: "parse_nodes",
    ///     fields: &["grammar"],
    ///     signature_types: &[],
    ///     local_types: &[],
    ///     domains: &[],
    /// });
    ///
    /// let report = method_vector_algebra(&left, &right);
    /// assert_eq!(report.right_dot_left(), 40);
    /// ```
    #[must_use]
    pub fn right_dot_left(self) -> u64 {
        self.right_dot_left
    }

    /// Returns the result of [`MethodVectorAlgebraReport::left_norm_squared`].
    ///
    /// This is the squared L2 norm of the left method vector.
    ///
    /// ```rust
    /// use common::test_support::decomposition::{MethodInput, method_vector_algebra, profile};
    ///
    /// let left = profile(MethodInput {
    ///     name: "parse_tokens",
    ///     fields: &["grammar"],
    ///     signature_types: &[],
    ///     local_types: &[],
    ///     domains: &[],
    /// });
    /// let right = profile(MethodInput {
    ///     name: "parse_nodes",
    ///     fields: &["grammar"],
    ///     signature_types: &[],
    ///     local_types: &[],
    ///     domains: &[],
    /// });
    ///
    /// let report = method_vector_algebra(&left, &right);
    /// assert_eq!(report.left_norm_squared(), 44);
    /// ```
    #[must_use]
    pub fn left_norm_squared(self) -> u64 {
        self.left_norm_squared
    }

    /// Returns the result of [`MethodVectorAlgebraReport::right_norm_squared`].
    ///
    /// This is the squared L2 norm of the right method vector.
    ///
    /// ```rust
    /// use common::test_support::decomposition::{MethodInput, method_vector_algebra, profile};
    ///
    /// let left = profile(MethodInput {
    ///     name: "parse_tokens",
    ///     fields: &["grammar"],
    ///     signature_types: &[],
    ///     local_types: &[],
    ///     domains: &[],
    /// });
    /// let right = profile(MethodInput {
    ///     name: "parse_nodes",
    ///     fields: &["grammar"],
    ///     signature_types: &[],
    ///     local_types: &[],
    ///     domains: &[],
    /// });
    ///
    /// let report = method_vector_algebra(&left, &right);
    /// assert_eq!(report.right_norm_squared(), 44);
    /// ```
    #[must_use]
    pub fn right_norm_squared(self) -> u64 {
        self.right_norm_squared
    }
}

/// Computes the shipped vector-algebra helper values for two methods.
///
/// This helper exists for behaviour tests that need to observe the runtime
/// `dot_product` and `norm_squared` results without widening the production
/// decomposition API.
///
/// # Examples
///
/// ```ignore
/// use common::test_support::decomposition::{MethodInput, method_vector_algebra, profile};
///
/// let left = profile(MethodInput {
///     name: "parse_tokens",
///     fields: &["grammar"],
///     signature_types: &[],
///     local_types: &[],
///     domains: &[],
/// });
/// let right = profile(MethodInput {
///     name: "save_to_disk",
///     fields: &[],
///     signature_types: &[],
///     local_types: &["PathBuf"],
///     domains: &["std::fs"],
/// });
///
/// let report = method_vector_algebra(&left, &right);
/// assert_eq!(report.left_dot_right(), 0);
/// ```
#[must_use]
pub fn method_vector_algebra(
    left: &MethodProfile,
    right: &MethodProfile,
) -> MethodVectorAlgebraReport {
    let left_vector = build_feature_vector(left);
    let right_vector = build_feature_vector(right);

    MethodVectorAlgebraReport {
        left_dot_right: dot_product(left_vector.weights(), right_vector.weights()),
        right_dot_left: dot_product(right_vector.weights(), left_vector.weights()),
        left_norm_squared: left_vector.norm_squared(),
        right_norm_squared: right_vector.norm_squared(),
    }
}

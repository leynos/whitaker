//! Shared decomposition-advice fixtures for unit and integration tests.
//!
//! These helpers keep the recurring grammar/serde/filesystem and transport
//! method communities aligned across diagnostic unit tests and behaviour
//! coverage.

use crate::decomposition_advice::{
    DecompositionContext, DecompositionSuggestion, MethodProfile, MethodProfileBuilder,
    SubjectKind, build_feature_vector, dot_product,
    methods_meet_cosine_threshold as runtime_methods_meet_cosine_threshold, suggest_decomposition,
};

/// Input data for building a [`MethodProfile`] in tests.
///
/// # Examples
///
/// ```ignore
/// use common::test_support::decomposition::{MethodInput, profile};
///
/// let profile = profile(MethodInput {
///     name: "parse_tokens",
///     fields: &["grammar"],
///     signature_types: &[],
///     local_types: &[],
///     domains: &[],
/// });
///
/// assert_eq!(profile.name(), "parse_tokens");
/// ```
pub struct MethodInput<'a> {
    pub name: &'a str,
    pub fields: &'a [&'a str],
    pub signature_types: &'a [&'a str],
    pub local_types: &'a [&'a str],
    pub domains: &'a [&'a str],
}

/// Builds a [`MethodProfile`] from declarative test input.
///
/// # Examples
///
/// ```ignore
/// use common::test_support::decomposition::{MethodInput, profile};
///
/// let profile = profile(MethodInput {
///     name: "save_to_disk",
///     fields: &[],
///     signature_types: &[],
///     local_types: &["PathBuf"],
///     domains: &["std::fs"],
/// });
///
/// assert_eq!(profile.name(), "save_to_disk");
/// ```
#[must_use]
pub fn profile(input: MethodInput<'_>) -> MethodProfile {
    let mut builder = MethodProfileBuilder::new(input.name);
    for field in input.fields {
        builder.record_accessed_field(*field);
    }
    for type_name in input.signature_types {
        builder.record_signature_type(*type_name);
    }
    for type_name in input.local_types {
        builder.record_local_type(*type_name);
    }
    for domain in input.domains {
        builder.record_external_domain(*domain);
    }
    builder.build()
}

/// Builds the recurring parser/serde/filesystem fixture.
///
/// # Examples
///
/// ```ignore
/// use common::test_support::decomposition::parser_serde_fs_fixture;
///
/// let methods = parser_serde_fs_fixture();
/// assert_eq!(methods.len(), 6);
/// ```
#[must_use]
pub fn parser_serde_fs_fixture() -> Vec<MethodProfile> {
    vec![
        profile(MethodInput {
            name: "parse_tokens",
            fields: &["grammar", "tokens"],
            signature_types: &["TokenStream"],
            local_types: &[],
            domains: &[],
        }),
        profile(MethodInput {
            name: "parse_nodes",
            fields: &["grammar", "ast"],
            signature_types: &[],
            local_types: &["ParseState"],
            domains: &[],
        }),
        profile(MethodInput {
            name: "encode_json",
            fields: &[],
            signature_types: &["Serializer"],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "decode_json",
            fields: &[],
            signature_types: &["Deserializer"],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "load_from_disk",
            fields: &[],
            signature_types: &[],
            local_types: &["PathBuf"],
            domains: &["std::fs"],
        }),
        profile(MethodInput {
            name: "save_to_disk",
            fields: &[],
            signature_types: &[],
            local_types: &["PathBuf"],
            domains: &["std::fs"],
        }),
    ]
}

/// Builds the recurring transport trait fixture.
///
/// # Examples
///
/// ```ignore
/// use common::test_support::decomposition::transport_trait_fixture;
///
/// let methods = transport_trait_fixture();
/// assert_eq!(methods.len(), 4);
/// ```
#[must_use]
pub fn transport_trait_fixture() -> Vec<MethodProfile> {
    vec![
        profile(MethodInput {
            name: "encode_request",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "decode_request",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "read_frame",
            fields: &[],
            signature_types: &["IoBuffer"],
            local_types: &[],
            domains: &["std::io"],
        }),
        profile(MethodInput {
            name: "write_frame",
            fields: &[],
            signature_types: &["IoBuffer"],
            local_types: &[],
            domains: &["std::io"],
        }),
    ]
}

/// Computes decomposition suggestions for `methods`.
///
/// # Examples
///
/// ```ignore
/// use common::decomposition_advice::SubjectKind;
/// use common::test_support::decomposition::{
///     decomposition_suggestions,
///     parser_serde_fs_fixture,
/// };
///
/// let methods = parser_serde_fs_fixture();
/// let (_context, suggestions) =
///     decomposition_suggestions("Foo", SubjectKind::Type, &methods);
/// assert!(!suggestions.is_empty());
/// ```
#[must_use]
pub fn decomposition_suggestions(
    subject: &str,
    kind: SubjectKind,
    methods: &[MethodProfile],
) -> (DecompositionContext, Vec<DecompositionSuggestion>) {
    let context = DecompositionContext::new(subject, kind);
    let suggestions = suggest_decomposition(&context, methods);
    (context, suggestions)
}

/// Evaluates whether two methods satisfy Whitaker's cosine threshold.
///
/// This helper exists for behaviour tests that need an observable seam without
/// widening the production decomposition API.
///
/// # Examples
///
/// ```ignore
/// use common::test_support::decomposition::{MethodInput, methods_meet_cosine_threshold, profile};
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
/// assert!(methods_meet_cosine_threshold(&left, &right));
/// ```
#[must_use]
pub fn methods_meet_cosine_threshold(left: &MethodProfile, right: &MethodProfile) -> bool {
    runtime_methods_meet_cosine_threshold(left, right)
}

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

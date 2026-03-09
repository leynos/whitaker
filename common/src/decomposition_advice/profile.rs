//! Public input types for decomposition analysis.

use std::collections::BTreeSet;

/// The kind of subject being analysed for decomposition advice.
///
/// # Examples
///
/// ```
/// use common::decomposition_advice::SubjectKind;
///
/// assert_eq!(SubjectKind::Type, SubjectKind::Type);
/// assert_ne!(SubjectKind::Type, SubjectKind::Trait);
/// ```
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SubjectKind {
    /// Analyse methods that belong to a type.
    Type,
    /// Analyse methods that belong to a trait.
    Trait,
}

impl std::str::FromStr for SubjectKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "type" => Ok(Self::Type),
            "trait" => Ok(Self::Trait),
            _ => Err(format!("unknown subject kind: {s}")),
        }
    }
}

/// Context about the analysed subject.
///
/// # Examples
///
/// ```
/// use common::decomposition_advice::{DecompositionContext, SubjectKind};
///
/// let context = DecompositionContext::new("Parser", SubjectKind::Type);
/// assert_eq!(context.subject_name(), "Parser");
/// assert_eq!(context.subject_kind(), SubjectKind::Type);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecompositionContext {
    subject_name: String,
    subject_kind: SubjectKind,
}

impl DecompositionContext {
    /// Creates a new decomposition-analysis context.
    #[must_use]
    pub fn new(subject_name: impl Into<String>, subject_kind: SubjectKind) -> Self {
        Self {
            subject_name: subject_name.into(),
            subject_kind,
        }
    }

    /// Returns the analysed subject name.
    #[must_use]
    pub fn subject_name(&self) -> &str {
        &self.subject_name
    }

    /// Returns the analysed subject kind.
    #[must_use]
    pub fn subject_kind(&self) -> SubjectKind {
        self.subject_kind
    }
}

/// Immutable per-method metadata used to build feature vectors.
///
/// The profile stores the inputs called for by the design document:
/// accessed fields, signature types, local-variable types, external domains,
/// and the method name itself.
///
/// # Examples
///
/// ```
/// use common::decomposition_advice::MethodProfileBuilder;
///
/// let mut builder = MethodProfileBuilder::new("parse_tokens");
/// builder
///     .record_accessed_field("grammar")
///     .record_accessed_field("tokens")
///     .record_signature_type("TokenStream");
///
/// let profile = builder.build();
/// assert_eq!(profile.name(), "parse_tokens");
/// assert_eq!(profile.accessed_fields().len(), 2);
/// assert_eq!(profile.signature_types().len(), 1);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodProfile {
    name: String,
    accessed_fields: BTreeSet<String>,
    signature_types: BTreeSet<String>,
    local_types: BTreeSet<String>,
    external_domains: BTreeSet<String>,
}

impl MethodProfile {
    /// Returns the method name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns accessed fields.
    #[must_use]
    pub fn accessed_fields(&self) -> &BTreeSet<String> {
        &self.accessed_fields
    }

    /// Returns types used in the method signature.
    #[must_use]
    pub fn signature_types(&self) -> &BTreeSet<String> {
        &self.signature_types
    }

    /// Returns types used in local variables.
    #[must_use]
    pub fn local_types(&self) -> &BTreeSet<String> {
        &self.local_types
    }

    /// Returns external domains used by the method.
    #[must_use]
    pub fn external_domains(&self) -> &BTreeSet<String> {
        &self.external_domains
    }
}

/// Mutable builder for [`MethodProfile`].
///
/// This follows the existing builder pattern used elsewhere in `common` and
/// keeps callers away from constructors with too many arguments.
///
/// # Examples
///
/// ```
/// use common::decomposition_advice::MethodProfileBuilder;
///
/// let mut builder = MethodProfileBuilder::new("write_cache");
/// builder
///     .record_external_domain("std::fs")
///     .record_local_type("PathBuf");
///
/// let profile = builder.build();
/// assert_eq!(profile.external_domains().iter().next().map(String::as_str), Some("std::fs"));
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MethodProfileBuilder {
    name: String,
    accessed_fields: BTreeSet<String>,
    signature_types: BTreeSet<String>,
    local_types: BTreeSet<String>,
    external_domains: BTreeSet<String>,
}

impl MethodProfileBuilder {
    /// Creates an empty builder for the given method name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Records an accessed field.
    pub fn record_accessed_field(&mut self, field_name: impl Into<String>) -> &mut Self {
        self.accessed_fields.insert(field_name.into());
        self
    }

    /// Records a signature type.
    pub fn record_signature_type(&mut self, type_name: impl Into<String>) -> &mut Self {
        self.signature_types.insert(type_name.into());
        self
    }

    /// Records a local-variable type.
    pub fn record_local_type(&mut self, type_name: impl Into<String>) -> &mut Self {
        self.local_types.insert(type_name.into());
        self
    }

    /// Records an external domain.
    pub fn record_external_domain(&mut self, domain: impl Into<String>) -> &mut Self {
        self.external_domains.insert(domain.into());
        self
    }

    /// Builds the immutable profile.
    #[must_use]
    pub fn build(self) -> MethodProfile {
        MethodProfile {
            name: self.name,
            accessed_fields: self.accessed_fields,
            signature_types: self.signature_types,
            local_types: self.local_types,
            external_domains: self.external_domains,
        }
    }
}

//! Pure parameter classification helpers for `rstest`-driven functions.

use super::RstestDetectionOptions;
use crate::attributes::Attribute;
use std::collections::BTreeSet;

/// Represents the supported parameter binding shapes for version-one `rstest`
/// classification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterBinding {
    /// A simple identifier binding such as `db`.
    Ident(String),
    /// Any unsupported pattern, such as destructuring.
    Unsupported,
}

/// Simplified metadata for a function parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RstestParameter {
    binding: ParameterBinding,
    attributes: Vec<Attribute>,
}

impl RstestParameter {
    /// Builds a parameter from a binding and its attributes.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::attributes::{Attribute, AttributeKind, AttributePath};
    /// use whitaker_common::rstest::{ParameterBinding, RstestParameter};
    ///
    /// let parameter = RstestParameter::new(
    ///     ParameterBinding::Ident("db".to_string()),
    ///     vec![Attribute::new(AttributePath::from("case"), AttributeKind::Outer)],
    /// );
    /// assert_eq!(parameter.attributes().len(), 1);
    /// ```
    #[must_use]
    pub fn new(binding: ParameterBinding, attributes: Vec<Attribute>) -> Self {
        Self {
            binding,
            attributes,
        }
    }

    /// Builds a simple identifier-backed parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::RstestParameter;
    ///
    /// let parameter = RstestParameter::ident("db");
    /// assert_eq!(parameter.binding_name(), Some("db"));
    /// ```
    #[must_use]
    pub fn ident(name: impl Into<String>) -> Self {
        Self::new(ParameterBinding::Ident(name.into()), Vec::new())
    }

    /// Builds an unsupported-pattern parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::RstestParameter;
    ///
    /// let parameter = RstestParameter::unsupported();
    /// assert_eq!(parameter.binding_name(), None);
    /// ```
    #[must_use]
    pub fn unsupported() -> Self {
        Self::new(ParameterBinding::Unsupported, Vec::new())
    }

    /// Returns the binding metadata.
    #[must_use]
    pub const fn binding(&self) -> &ParameterBinding {
        &self.binding
    }

    /// Returns the parameter attributes.
    #[must_use]
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    /// Returns the identifier binding name when available.
    #[must_use]
    pub fn binding_name(&self) -> Option<&str> {
        match &self.binding {
            ParameterBinding::Ident(name) => Some(name),
            ParameterBinding::Unsupported => None,
        }
    }
}

/// Classification outcome for an `rstest` parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RstestParameterKind {
    /// A fixture-local identifier binding.
    FixtureLocal { name: String },
    /// A provider-driven input such as `#[case]` or `#[values]`.
    Provider,
    /// A binding shape that version one does not support.
    UnsupportedPattern,
}

/// Classifies a simplified `rstest` parameter for fixture extraction logic.
///
/// # Examples
///
/// ```
/// use whitaker_common::attributes::{Attribute, AttributeKind, AttributePath};
/// use whitaker_common::rstest::{
///     RstestDetectionOptions, RstestParameter, RstestParameterKind, classify_rstest_parameter,
/// };
///
/// let parameter = RstestParameter::new(
///     whitaker_common::rstest::ParameterBinding::Ident("db".to_string()),
///     vec![Attribute::new(AttributePath::from("case"), AttributeKind::Outer)],
/// );
/// let kind = classify_rstest_parameter(&parameter, &RstestDetectionOptions::default());
/// assert_eq!(kind, RstestParameterKind::Provider);
/// ```
#[must_use]
pub fn classify_rstest_parameter(
    parameter: &RstestParameter,
    options: &RstestDetectionOptions,
) -> RstestParameterKind {
    match parameter.binding() {
        ParameterBinding::Ident(name) => {
            if parameter_has_provider_attribute(parameter.attributes(), options) {
                RstestParameterKind::Provider
            } else {
                RstestParameterKind::FixtureLocal { name: name.clone() }
            }
        }
        ParameterBinding::Unsupported => RstestParameterKind::UnsupportedPattern,
    }
}

/// Collects supported fixture-local parameter names in deterministic order.
///
/// # Examples
///
/// ```
/// use whitaker_common::rstest::{RstestDetectionOptions, RstestParameter, fixture_local_names};
///
/// let parameters = vec![RstestParameter::ident("db"), RstestParameter::ident("clock")];
/// let names = fixture_local_names(&parameters, &RstestDetectionOptions::default());
/// assert!(names.contains("db"));
/// assert!(names.contains("clock"));
/// ```
#[must_use]
pub fn fixture_local_names(
    parameters: &[RstestParameter],
    options: &RstestDetectionOptions,
) -> BTreeSet<String> {
    parameters
        .iter()
        .filter_map(
            |parameter| match classify_rstest_parameter(parameter, options) {
                RstestParameterKind::FixtureLocal { name } => Some(name),
                RstestParameterKind::Provider | RstestParameterKind::UnsupportedPattern => None,
            },
        )
        .collect()
}

fn parameter_has_provider_attribute(
    attributes: &[Attribute],
    options: &RstestDetectionOptions,
) -> bool {
    attributes.iter().any(|attribute| {
        options.provider_param_attributes().iter().any(|candidate| {
            attribute
                .path()
                .matches(candidate.segments().iter().map(String::as_str))
        })
    })
}

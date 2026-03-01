//! Metric collection for brain trait detection.
//!
//! Provides pure data structures and helper functions for collecting the
//! three signals used by the `brain_trait` lint:
//!
//! - interface size (trait item counts),
//! - default method cognitive complexity aggregation, and
//! - implementor burden (required method count).
//!
//! These helpers are compiler-independent and accept pre-extracted metadata.
//! Lint drivers can populate this module from HIR traversal without adding
//! `rustc_private` dependencies to `common`.

#[cfg(test)]
mod tests;

/// The category of a trait item considered by `brain_trait` metrics.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::TraitItemKind;
///
/// let kind = TraitItemKind::RequiredMethod;
/// assert!(matches!(kind, TraitItemKind::RequiredMethod));
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraitItemKind {
    /// A method declaration without a default body.
    RequiredMethod,
    /// A method declaration with a default body.
    DefaultMethod,
    /// An associated type declaration.
    AssociatedType,
    /// An associated const declaration.
    AssociatedConst,
}

/// Per-item metadata used to compute trait-level metrics.
///
/// `default_method_cc` is set only for default methods.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::{TraitItemKind, TraitItemMetrics};
///
/// let item = TraitItemMetrics::default_method("render", 12);
/// assert_eq!(item.kind(), TraitItemKind::DefaultMethod);
/// assert_eq!(item.default_method_cc(), Some(12));
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitItemMetrics {
    name: String,
    kind: TraitItemKind,
    default_method_cc: Option<usize>,
}

impl TraitItemMetrics {
    /// Creates metadata for a required method.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_trait_metrics::{TraitItemKind, TraitItemMetrics};
    ///
    /// let item = TraitItemMetrics::required_method("parse");
    /// assert_eq!(item.kind(), TraitItemKind::RequiredMethod);
    /// assert_eq!(item.default_method_cc(), None);
    /// ```
    #[must_use]
    pub fn required_method(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: TraitItemKind::RequiredMethod,
            default_method_cc: None,
        }
    }

    /// Creates metadata for a default method.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_trait_metrics::{TraitItemKind, TraitItemMetrics};
    ///
    /// let item = TraitItemMetrics::default_method("render", 17);
    /// assert_eq!(item.kind(), TraitItemKind::DefaultMethod);
    /// assert_eq!(item.default_method_cc(), Some(17));
    /// ```
    #[must_use]
    pub fn default_method(name: impl Into<String>, cognitive_complexity: usize) -> Self {
        Self {
            name: name.into(),
            kind: TraitItemKind::DefaultMethod,
            default_method_cc: Some(cognitive_complexity),
        }
    }

    /// Creates metadata for an associated type.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_trait_metrics::{TraitItemKind, TraitItemMetrics};
    ///
    /// let item = TraitItemMetrics::associated_type("Output");
    /// assert_eq!(item.kind(), TraitItemKind::AssociatedType);
    /// ```
    #[must_use]
    pub fn associated_type(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: TraitItemKind::AssociatedType,
            default_method_cc: None,
        }
    }

    /// Creates metadata for an associated const.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_trait_metrics::{TraitItemKind, TraitItemMetrics};
    ///
    /// let item = TraitItemMetrics::associated_const("VERSION");
    /// assert_eq!(item.kind(), TraitItemKind::AssociatedConst);
    /// ```
    #[must_use]
    pub fn associated_const(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: TraitItemKind::AssociatedConst,
            default_method_cc: None,
        }
    }

    /// Returns the trait item name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the trait item kind.
    #[must_use]
    pub fn kind(&self) -> TraitItemKind {
        self.kind
    }

    /// Returns default method cognitive complexity when present.
    #[must_use]
    pub fn default_method_cc(&self) -> Option<usize> {
        self.default_method_cc
    }

    /// Returns `true` when this item is a required method.
    #[must_use]
    pub fn is_required_method(&self) -> bool {
        self.kind == TraitItemKind::RequiredMethod
    }

    /// Returns `true` when this item is a default method.
    #[must_use]
    pub fn is_default_method(&self) -> bool {
        self.kind == TraitItemKind::DefaultMethod
    }
}

/// Returns the total number of trait items.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::{TraitItemMetrics, trait_item_count};
///
/// let items = vec![
///     TraitItemMetrics::required_method("parse"),
///     TraitItemMetrics::associated_type("Output"),
/// ];
/// assert_eq!(trait_item_count(&items), 2);
/// ```
#[must_use]
pub fn trait_item_count(items: &[TraitItemMetrics]) -> usize {
    items.len()
}

/// Returns the number of required methods.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::{TraitItemMetrics, required_method_count};
///
/// let items = vec![
///     TraitItemMetrics::required_method("parse"),
///     TraitItemMetrics::default_method("render", 12),
/// ];
/// assert_eq!(required_method_count(&items), 1);
/// ```
#[must_use]
pub fn required_method_count(items: &[TraitItemMetrics]) -> usize {
    items
        .iter()
        .filter(|item| item.is_required_method())
        .count()
}

/// Returns the number of default methods.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::{TraitItemMetrics, default_method_count};
///
/// let items = vec![
///     TraitItemMetrics::required_method("parse"),
///     TraitItemMetrics::default_method("render", 12),
/// ];
/// assert_eq!(default_method_count(&items), 1);
/// ```
#[must_use]
pub fn default_method_count(items: &[TraitItemMetrics]) -> usize {
    items.iter().filter(|item| item.is_default_method()).count()
}

/// Returns the sum of default method cognitive complexity values.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::{TraitItemMetrics, default_method_cc_sum};
///
/// let items = vec![
///     TraitItemMetrics::default_method("render", 12),
///     TraitItemMetrics::default_method("serialise", 8),
/// ];
/// assert_eq!(default_method_cc_sum(&items), 20);
/// ```
#[must_use]
pub fn default_method_cc_sum(items: &[TraitItemMetrics]) -> usize {
    items
        .iter()
        .filter_map(TraitItemMetrics::default_method_cc)
        .sum()
}

/// Aggregated metrics for one trait.
///
/// # Examples
///
/// ```
/// use common::brain_trait_metrics::TraitMetricsBuilder;
///
/// let mut builder = TraitMetricsBuilder::new("Parser");
/// builder.add_required_method("parse");
/// builder.add_default_method("render", 12, false);
/// let metrics = builder.build();
///
/// assert_eq!(metrics.total_item_count(), 2);
/// assert_eq!(metrics.required_method_count(), 1);
/// assert_eq!(metrics.default_method_cc_sum(), 12);
/// assert_eq!(metrics.implementor_burden(), 1);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMetrics {
    trait_name: String,
    total_item_count: usize,
    required_method_count: usize,
    default_method_count: usize,
    default_method_cc_sum: usize,
    implementor_burden: usize,
}

impl TraitMetrics {
    /// Returns the trait name.
    #[must_use]
    pub fn trait_name(&self) -> &str {
        &self.trait_name
    }

    /// Returns the total number of trait items.
    #[must_use]
    pub fn total_item_count(&self) -> usize {
        self.total_item_count
    }

    /// Returns the number of required methods.
    #[must_use]
    pub fn required_method_count(&self) -> usize {
        self.required_method_count
    }

    /// Returns the number of default methods.
    #[must_use]
    pub fn default_method_count(&self) -> usize {
        self.default_method_count
    }

    /// Returns the sum of default method cognitive complexity values.
    #[must_use]
    pub fn default_method_cc_sum(&self) -> usize {
        self.default_method_cc_sum
    }

    /// Returns implementor burden as the required method count.
    #[must_use]
    pub fn implementor_burden(&self) -> usize {
        self.implementor_burden
    }
}

/// Incremental builder for [`TraitMetrics`].
#[derive(Clone, Debug, Default)]
pub struct TraitMetricsBuilder {
    trait_name: String,
    items: Vec<TraitItemMetrics>,
}

impl TraitMetricsBuilder {
    /// Creates a new builder for the provided trait name.
    #[must_use]
    pub fn new(trait_name: impl Into<String>) -> Self {
        Self {
            trait_name: trait_name.into(),
            items: Vec::new(),
        }
    }

    /// Adds pre-built trait item metadata.
    pub fn add_item(&mut self, item: TraitItemMetrics) {
        self.items.push(item);
    }

    /// Adds a required method.
    pub fn add_required_method(&mut self, name: impl Into<String>) {
        self.items.push(TraitItemMetrics::required_method(name));
    }

    /// Adds a default method, optionally filtering macro-expanded entries.
    ///
    /// When `is_from_expansion` is `true`, the method is discarded and does
    /// not contribute to interface-size or complexity metrics.
    pub fn add_default_method(
        &mut self,
        name: impl Into<String>,
        cognitive_complexity: usize,
        is_from_expansion: bool,
    ) {
        if is_from_expansion {
            return;
        }

        self.items
            .push(TraitItemMetrics::default_method(name, cognitive_complexity));
    }

    /// Adds an associated type.
    pub fn add_associated_type(&mut self, name: impl Into<String>) {
        self.items.push(TraitItemMetrics::associated_type(name));
    }

    /// Adds an associated const.
    pub fn add_associated_const(&mut self, name: impl Into<String>) {
        self.items.push(TraitItemMetrics::associated_const(name));
    }

    /// Returns `true` when no items have been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Consumes the builder and returns aggregated trait metrics.
    #[must_use]
    pub fn build(self) -> TraitMetrics {
        let total_item_count = trait_item_count(&self.items);
        let required_method_count = required_method_count(&self.items);
        let default_method_count = default_method_count(&self.items);
        let default_method_cc_sum = default_method_cc_sum(&self.items);

        TraitMetrics {
            trait_name: self.trait_name,
            total_item_count,
            required_method_count,
            default_method_count,
            default_method_cc_sum,
            implementor_burden: required_method_count,
        }
    }
}

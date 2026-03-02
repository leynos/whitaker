//! Trait item modelling and helper aggregations for brain trait metrics.

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
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_trait_metrics::TraitItemMetrics;
    ///
    /// let item = TraitItemMetrics::required_method("parse");
    /// assert_eq!(item.name(), "parse");
    /// ```
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the trait item kind.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_trait_metrics::{TraitItemKind, TraitItemMetrics};
    ///
    /// let item = TraitItemMetrics::required_method("parse");
    /// assert_eq!(item.kind(), TraitItemKind::RequiredMethod);
    /// ```
    #[must_use]
    pub fn kind(&self) -> TraitItemKind {
        self.kind
    }

    /// Returns default method cognitive complexity when present.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_trait_metrics::TraitItemMetrics;
    ///
    /// let item = TraitItemMetrics::default_method("render", 9);
    /// assert_eq!(item.default_method_cc(), Some(9));
    /// ```
    #[must_use]
    pub fn default_method_cc(&self) -> Option<usize> {
        self.default_method_cc
    }

    /// Returns `true` when this item is a required method.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_trait_metrics::TraitItemMetrics;
    ///
    /// let item = TraitItemMetrics::required_method("parse");
    /// assert!(item.is_required_method());
    /// ```
    #[must_use]
    pub fn is_required_method(&self) -> bool {
        self.kind == TraitItemKind::RequiredMethod
    }

    /// Returns `true` when this item is a default method.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_trait_metrics::TraitItemMetrics;
    ///
    /// let item = TraitItemMetrics::default_method("render", 9);
    /// assert!(item.is_default_method());
    /// ```
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
///     TraitItemMetrics::default_method("serialize", 8),
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

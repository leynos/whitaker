//! Trait-level aggregates and incremental builder for brain trait metrics.

use super::{TraitItemKind, TraitItemMetrics};

/// Aggregated metrics for one trait.
///
/// # Examples
///
/// ```
/// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
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
}

impl TraitMetrics {
    /// Returns the trait name.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let metrics = TraitMetricsBuilder::new("Parser").build();
    /// assert_eq!(metrics.trait_name(), "Parser");
    /// ```
    #[must_use]
    pub fn trait_name(&self) -> &str {
        &self.trait_name
    }

    /// Returns the total number of trait items.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_required_method("parse");
    /// assert_eq!(builder.build().total_item_count(), 1);
    /// ```
    #[must_use]
    pub fn total_item_count(&self) -> usize {
        self.total_item_count
    }

    /// Returns the number of required methods.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_required_method("parse");
    /// assert_eq!(builder.build().required_method_count(), 1);
    /// ```
    #[must_use]
    pub fn required_method_count(&self) -> usize {
        self.required_method_count
    }

    /// Returns the number of default methods.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_default_method("render", 3, false);
    /// assert_eq!(builder.build().default_method_count(), 1);
    /// ```
    #[must_use]
    pub fn default_method_count(&self) -> usize {
        self.default_method_count
    }

    /// Returns the sum of default method cognitive complexity values.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_default_method("render", 4, false);
    /// builder.add_default_method("serialize", 8, false);
    /// assert_eq!(builder.build().default_method_cc_sum(), 12);
    /// ```
    #[must_use]
    pub fn default_method_cc_sum(&self) -> usize {
        self.default_method_cc_sum
    }

    /// Returns implementor burden as the required method count.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_required_method("parse");
    /// builder.add_required_method("validate");
    /// assert_eq!(builder.build().implementor_burden(), 2);
    /// ```
    #[must_use]
    pub fn implementor_burden(&self) -> usize {
        self.required_method_count
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
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let metrics = TraitMetricsBuilder::new("Parser").build();
    /// assert_eq!(metrics.trait_name(), "Parser");
    /// ```
    #[must_use]
    pub fn new(trait_name: impl Into<String>) -> Self {
        Self {
            trait_name: trait_name.into(),
            items: Vec::new(),
        }
    }

    /// Adds pre-built trait item metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::{TraitItemMetrics, TraitMetricsBuilder};
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_item(TraitItemMetrics::required_method("parse"));
    /// assert_eq!(builder.build().required_method_count(), 1);
    /// ```
    pub fn add_item(&mut self, item: TraitItemMetrics) {
        self.items.push(item);
    }

    /// Adds a required method.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_required_method("parse");
    /// assert_eq!(builder.build().required_method_count(), 1);
    /// ```
    pub fn add_required_method(&mut self, name: impl Into<String>) {
        self.items.push(TraitItemMetrics::required_method(name));
    }

    /// Adds a default method, optionally filtering macro-expanded entries.
    ///
    /// When `is_from_expansion` is `true`, the method is discarded and does
    /// not contribute to interface-size or complexity metrics.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_default_method("generated_helper", 30, true);
    /// builder.add_default_method("render", 12, false);
    /// let metrics = builder.build();
    ///
    /// assert_eq!(metrics.default_method_count(), 1);
    /// assert_eq!(metrics.default_method_cc_sum(), 12);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_associated_type("Output");
    /// assert_eq!(builder.build().total_item_count(), 1);
    /// ```
    pub fn add_associated_type(&mut self, name: impl Into<String>) {
        self.items.push(TraitItemMetrics::associated_type(name));
    }

    /// Adds an associated const.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_associated_const("VERSION");
    /// assert_eq!(builder.build().total_item_count(), 1);
    /// ```
    pub fn add_associated_const(&mut self, name: impl Into<String>) {
        self.items.push(TraitItemMetrics::associated_const(name));
    }

    /// Returns `true` when no items have been recorded.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// assert!(TraitMetricsBuilder::new("Parser").is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Consumes the builder and returns aggregated trait metrics.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::brain_trait_metrics::TraitMetricsBuilder;
    ///
    /// let mut builder = TraitMetricsBuilder::new("Parser");
    /// builder.add_required_method("parse");
    /// builder.add_default_method("render", 5, false);
    /// let metrics = builder.build();
    ///
    /// assert_eq!(metrics.total_item_count(), 2);
    /// assert_eq!(metrics.required_method_count(), 1);
    /// assert_eq!(metrics.default_method_cc_sum(), 5);
    /// ```
    #[must_use]
    pub fn build(self) -> TraitMetrics {
        let (total_item_count, required_method_count, default_method_count, default_method_cc_sum) =
            self.items.iter().fold(
                (0, 0, 0, 0),
                |(
                    total_item_count,
                    required_method_count,
                    default_method_count,
                    default_method_cc_sum,
                ),
                 item| {
                    let total_item_count = total_item_count + 1;
                    let (required_method_count, default_method_count, default_method_cc_sum) =
                        match item.kind() {
                            TraitItemKind::RequiredMethod => (
                                required_method_count + 1,
                                default_method_count,
                                default_method_cc_sum,
                            ),
                            TraitItemKind::DefaultMethod => (
                                required_method_count,
                                default_method_count + 1,
                                default_method_cc_sum + item.default_method_cc().unwrap_or(0),
                            ),
                            TraitItemKind::AssociatedType | TraitItemKind::AssociatedConst => (
                                required_method_count,
                                default_method_count,
                                default_method_cc_sum,
                            ),
                        };

                    (
                        total_item_count,
                        required_method_count,
                        default_method_count,
                        default_method_cc_sum,
                    )
                },
            );

        TraitMetrics {
            trait_name: self.trait_name,
            total_item_count,
            required_method_count,
            default_method_count,
            default_method_cc_sum,
        }
    }
}

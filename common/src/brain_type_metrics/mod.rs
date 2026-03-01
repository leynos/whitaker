//! Metric collection for brain type detection.
//!
//! Provides pure data structures and computation functions for the four
//! signals used by the `brain_type` lint: Weighted Methods Count (WMC),
//! brain method detection, Lack of Cohesion in Methods version 4 (LCOM4)
//! integration, and foreign reach. These helpers operate on pre-extracted
//! method metadata and do not depend on `rustc_private` or any High-level
//! Intermediate Representation (HIR) types.
//!
//! The lint driver (roadmap 6.2.2) walks the HIR, computes per-method
//! cognitive complexity (CC) and lines of code (LOC), and feeds the values
//! into [`TypeMetricsBuilder`]. This module aggregates and evaluates those
//! values without any compiler dependency.
//!
//! See `docs/brain-trust-lints-design.md` §`brain_type` signals for the
//! full design rationale.

pub mod cognitive_complexity;
pub mod diagnostic;
pub mod evaluation;
pub mod foreign_reach;

pub use cognitive_complexity::CognitiveComplexityBuilder;
pub use foreign_reach::{ForeignReferenceSet, foreign_reach_count};

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// Per-method metrics
// ---------------------------------------------------------------------------

/// Metrics collected for a single method in a type.
///
/// The lint driver populates this struct from HIR analysis. The CC value
/// is the SonarSource-style cognitive complexity count; LOC is the span
/// line count. Both values are pre-computed by the caller.
///
/// # Examples
///
/// ```
/// use common::brain_type_metrics::MethodMetrics;
///
/// let m = MethodMetrics::new("parse", 31, 140);
/// assert_eq!(m.name(), "parse");
/// assert_eq!(m.cognitive_complexity(), 31);
/// assert_eq!(m.lines_of_code(), 140);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodMetrics {
    name: String,
    cognitive_complexity: usize,
    lines_of_code: usize,
}

impl MethodMetrics {
    /// Creates a new per-method metrics record.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_type_metrics::MethodMetrics;
    ///
    /// let m = MethodMetrics::new("validate", 10, 45);
    /// assert_eq!(m.name(), "validate");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>, cognitive_complexity: usize, lines_of_code: usize) -> Self {
        Self {
            name: name.into(),
            cognitive_complexity,
            lines_of_code,
        }
    }

    /// Returns the method name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the cognitive complexity (CC) value.
    #[must_use]
    pub fn cognitive_complexity(&self) -> usize {
        self.cognitive_complexity
    }

    /// Returns the lines of code (LOC) count.
    #[must_use]
    pub fn lines_of_code(&self) -> usize {
        self.lines_of_code
    }

    /// Returns `true` when this method qualifies as a "brain method".
    ///
    /// A brain method has cognitive complexity >= `cc_threshold` **and**
    /// lines of code >= `loc_threshold`. Both conditions must hold.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_type_metrics::MethodMetrics;
    ///
    /// let m = MethodMetrics::new("parse", 30, 100);
    /// assert!(m.is_brain_method(25, 80));
    /// assert!(!m.is_brain_method(25, 200));
    /// ```
    #[must_use]
    pub fn is_brain_method(&self, cc_threshold: usize, loc_threshold: usize) -> bool {
        self.cognitive_complexity >= cc_threshold && self.lines_of_code >= loc_threshold
    }
}

// ---------------------------------------------------------------------------
// WMC and brain method detection
// ---------------------------------------------------------------------------

/// Computes the Weighted Methods Count (WMC) as the sum of cognitive
/// complexity values across all methods.
///
/// Returns `0` for an empty slice.
///
/// # Examples
///
/// ```
/// use common::brain_type_metrics::{MethodMetrics, weighted_methods_count};
///
/// let methods = vec![
///     MethodMetrics::new("a", 10, 50),
///     MethodMetrics::new("b", 20, 60),
/// ];
/// assert_eq!(weighted_methods_count(&methods), 30);
/// ```
#[must_use]
pub fn weighted_methods_count(methods: &[MethodMetrics]) -> usize {
    methods
        .iter()
        .map(MethodMetrics::cognitive_complexity)
        .sum()
}

/// Returns the subset of methods that qualify as brain methods under
/// the given thresholds.
///
/// A method qualifies when its cognitive complexity >= `cc_threshold`
/// **and** its lines of code >= `loc_threshold`. The returned vector
/// preserves input order.
///
/// # Examples
///
/// ```
/// use common::brain_type_metrics::{MethodMetrics, brain_methods};
///
/// let methods = vec![
///     MethodMetrics::new("parse", 30, 100),
///     MethodMetrics::new("helper", 5, 20),
/// ];
/// let brains = brain_methods(&methods, 25, 80);
/// assert_eq!(brains.len(), 1);
/// assert_eq!(brains[0].name(), "parse");
/// ```
#[must_use]
pub fn brain_methods(
    methods: &[MethodMetrics],
    cc_threshold: usize,
    loc_threshold: usize,
) -> Vec<&MethodMetrics> {
    methods
        .iter()
        .filter(|m| m.is_brain_method(cc_threshold, loc_threshold))
        .collect()
}

// ---------------------------------------------------------------------------
// Aggregate type-level metrics
// ---------------------------------------------------------------------------

/// Aggregated metrics for a single type, combining all signals needed
/// for brain type evaluation.
///
/// This struct is the interface between metric collection (6.2.1) and
/// threshold evaluation (6.2.2). The lint driver in 6.2.2 constructs
/// this via [`TypeMetricsBuilder`] and evaluates rule sets against it.
///
/// # Examples
///
/// ```
/// use common::brain_type_metrics::TypeMetricsBuilder;
///
/// let mut builder = TypeMetricsBuilder::new("Foo", 25, 80);
/// builder.add_method("parse", 31, 140);
/// builder.set_lcom4(3);
/// builder.set_foreign_reach(5);
/// let tm = builder.build();
/// assert_eq!(tm.type_name(), "Foo");
/// assert_eq!(tm.wmc(), 31);
/// assert_eq!(tm.brain_method_count(), 1);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeMetrics {
    type_name: String,
    wmc: usize,
    brain_methods: Vec<MethodMetrics>,
    lcom4: usize,
    foreign_reach: usize,
    method_count: usize,
}

impl TypeMetrics {
    /// Returns the type name.
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Weighted Methods Count (sum of CC across all methods).
    #[must_use]
    pub fn wmc(&self) -> usize {
        self.wmc
    }

    /// Brain methods with their full metric details.
    #[must_use]
    pub fn brain_methods(&self) -> &[MethodMetrics] {
        &self.brain_methods
    }

    /// Returns an iterator over the names of brain methods.
    ///
    /// Callers that need a collected `Vec` should use `.collect()`.
    pub fn brain_method_names(&self) -> impl Iterator<Item = &str> {
        self.brain_methods.iter().map(|m| m.name())
    }

    /// Number of brain methods detected.
    #[must_use]
    pub fn brain_method_count(&self) -> usize {
        self.brain_methods.len()
    }

    /// LCOM4 connected component count (1 = cohesive, >= 2 = low cohesion).
    #[must_use]
    pub fn lcom4(&self) -> usize {
        self.lcom4
    }

    /// Count of distinct external modules or types referenced.
    #[must_use]
    pub fn foreign_reach(&self) -> usize {
        self.foreign_reach
    }

    /// Total number of methods in the type.
    #[must_use]
    pub fn method_count(&self) -> usize {
        self.method_count
    }
}

// ---------------------------------------------------------------------------
// Builder for incremental construction
// ---------------------------------------------------------------------------

/// Builder for constructing [`TypeMetrics`] incrementally from
/// method-level data.
///
/// The lint driver creates a builder for each type, adds method
/// metrics as they become available during HIR traversal, then builds
/// the final aggregate. Brain method thresholds are provided at
/// construction time so the builder can identify brain methods during
/// [`build`](TypeMetricsBuilder::build).
///
/// # Examples
///
/// ```
/// use common::brain_type_metrics::TypeMetricsBuilder;
///
/// let mut builder = TypeMetricsBuilder::new("Foo", 25, 80);
/// builder.add_method("parse", 31, 140);
/// builder.add_method("helper", 5, 20);
/// builder.set_lcom4(2);
/// builder.set_foreign_reach(5);
///
/// let metrics = builder.build();
/// assert_eq!(metrics.wmc(), 36);
/// assert_eq!(metrics.brain_method_count(), 1);
/// assert_eq!(metrics.lcom4(), 2);
/// ```
#[derive(Clone, Debug)]
pub struct TypeMetricsBuilder {
    type_name: String,
    method_metrics: Vec<MethodMetrics>,
    lcom4: Option<usize>,
    foreign_reach: Option<usize>,
    cc_threshold: usize,
    loc_threshold: usize,
}

impl TypeMetricsBuilder {
    /// Creates a new builder with the given type name and brain method
    /// thresholds.
    #[must_use]
    pub fn new(type_name: impl Into<String>, cc_threshold: usize, loc_threshold: usize) -> Self {
        Self {
            type_name: type_name.into(),
            method_metrics: Vec::new(),
            lcom4: None,
            foreign_reach: None,
            cc_threshold,
            loc_threshold,
        }
    }

    /// Adds a method's metrics to the builder.
    pub fn add_method(
        &mut self,
        name: impl Into<String>,
        cognitive_complexity: usize,
        lines_of_code: usize,
    ) {
        self.method_metrics.push(MethodMetrics::new(
            name,
            cognitive_complexity,
            lines_of_code,
        ));
    }

    /// Records the LCOM4 value (connected component count).
    pub fn set_lcom4(&mut self, lcom4: usize) {
        self.lcom4 = Some(lcom4);
    }

    /// Records the foreign reach count.
    pub fn set_foreign_reach(&mut self, count: usize) {
        self.foreign_reach = Some(count);
    }

    /// Consumes the builder and returns the completed [`TypeMetrics`].
    ///
    /// WMC is computed as the sum of cognitive complexity across all
    /// added methods. Brain methods are identified using the thresholds
    /// provided at construction. LCOM4 and foreign reach default to `0`
    /// if not explicitly set.
    #[must_use]
    pub fn build(self) -> TypeMetrics {
        let wmc = weighted_methods_count(&self.method_metrics);
        let brain: Vec<MethodMetrics> =
            brain_methods(&self.method_metrics, self.cc_threshold, self.loc_threshold)
                .into_iter()
                .cloned()
                .collect();
        let method_count = self.method_metrics.len();

        TypeMetrics {
            type_name: self.type_name,
            wmc,
            brain_methods: brain,
            lcom4: self.lcom4.unwrap_or(0),
            foreign_reach: self.foreign_reach.unwrap_or(0),
            method_count,
        }
    }
}

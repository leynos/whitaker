//! Foreign reach computation for brain type analysis.
//!
//! "Foreign reach" counts distinct external modules or types referenced by
//! a type's methods, analogous to Access to Foreign Data (ATFD) in
//! object-oriented metrics. This module provides the counting and
//! deduplication logic as a pure function. The HIR walker (in roadmap
//! 6.2.2) feeds in string representations of external references.
//!
//! See `docs/brain-trust-lints-design.md` §`brain_type` signals for the
//! full design rationale.

use std::collections::BTreeSet;

/// A set of foreign (external) type or module references observed during
/// method body analysis.
///
/// Each reference is stored as a string path (e.g. `"std::collections"` or
/// `"serde::Deserialize"`). The set automatically deduplicates entries.
/// Macro-expanded references are filtered via the `is_from_expansion`
/// parameter on [`record_reference`](ForeignReferenceSet::record_reference),
/// following the same pattern as
/// [`MethodInfoBuilder`](crate::lcom4::MethodInfoBuilder).
///
/// # Examples
///
/// ```
/// use common::brain_type_metrics::ForeignReferenceSet;
///
/// let mut refs = ForeignReferenceSet::new();
/// refs.record_reference("std::collections", false);
/// refs.record_reference("serde::Deserialize", false);
/// refs.record_reference("std::collections", false); // duplicate
///
/// assert_eq!(refs.count(), 2);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForeignReferenceSet {
    references: BTreeSet<String>,
}

impl ForeignReferenceSet {
    /// Creates a new empty reference set.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_type_metrics::ForeignReferenceSet;
    ///
    /// let refs = ForeignReferenceSet::new();
    /// assert!(refs.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a reference to an external module or type path.
    ///
    /// When `is_from_expansion` is `true` the reference is silently
    /// ignored, preventing macro-generated code from inflating the
    /// foreign reach count. Duplicate paths are naturally deduplicated
    /// by the underlying `BTreeSet`.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_type_metrics::ForeignReferenceSet;
    ///
    /// let mut refs = ForeignReferenceSet::new();
    /// refs.record_reference("std::fmt", true);  // macro — filtered
    /// refs.record_reference("serde::Serialize", false);
    ///
    /// assert_eq!(refs.count(), 1);
    /// ```
    pub fn record_reference(&mut self, path: impl Into<String>, is_from_expansion: bool) {
        if !is_from_expansion {
            self.references.insert(path.into());
        }
    }

    /// Returns the number of distinct foreign references recorded.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_type_metrics::ForeignReferenceSet;
    ///
    /// let mut refs = ForeignReferenceSet::new();
    /// refs.record_reference("tokio::fs", false);
    /// assert_eq!(refs.count(), 1);
    /// ```
    #[must_use]
    pub fn count(&self) -> usize {
        self.references.len()
    }

    /// Returns `true` when no references have been recorded.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_type_metrics::ForeignReferenceSet;
    ///
    /// let refs = ForeignReferenceSet::new();
    /// assert!(refs.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
    }

    /// Returns the set of recorded references, for diagnostic display.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::brain_type_metrics::ForeignReferenceSet;
    ///
    /// let mut refs = ForeignReferenceSet::new();
    /// refs.record_reference("std::io", false);
    /// assert!(refs.references().contains("std::io"));
    /// ```
    #[must_use]
    pub fn references(&self) -> &BTreeSet<String> {
        &self.references
    }
}

/// Counts distinct foreign references from an iterator of
/// `(path, is_from_expansion)` pairs.
///
/// This is a convenience for callers that have all references available
/// upfront. It constructs a [`ForeignReferenceSet`], records every
/// reference, and returns the deduplicated count.
///
/// # Examples
///
/// ```
/// use common::brain_type_metrics::foreign_reach_count;
///
/// let refs = vec![
///     ("std::io".into(), false),
///     ("std::io".into(), false),       // duplicate
///     ("serde::de".into(), false),
///     ("macro_gen".into(), true),       // macro — filtered
/// ];
/// assert_eq!(foreign_reach_count(refs), 2);
/// ```
#[must_use]
pub fn foreign_reach_count(references: impl IntoIterator<Item = (String, bool)>) -> usize {
    let mut set = ForeignReferenceSet::new();
    for (path, from_expansion) in references {
        set.record_reference(path, from_expansion);
    }
    set.count()
}

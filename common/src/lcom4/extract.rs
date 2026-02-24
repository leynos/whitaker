//! Method metadata extraction for LCOM4 cohesion analysis.
//!
//! Provides [`MethodInfoBuilder`], a builder that incrementally accumulates
//! field accesses and method calls for a single method while filtering out
//! macro-expanded spans. Lint drivers walk method bodies and call
//! [`MethodInfoBuilder::record_field_access`] and
//! [`MethodInfoBuilder::record_method_call`] for each relevant HIR node.
//! The builder produces a [`MethodInfo`] suitable for
//! [`cohesion_components`](super::cohesion_components).
//!
//! This module is a pure library with no `rustc_private` dependency. Macro-span
//! filtering is expressed through an `is_from_expansion: bool` parameter that
//! the caller (the HIR walker) populates from `expr.span.from_expansion()`. This
//! mirrors the pattern used by `bumpy_road_function`'s `SegmentBuilder`, where
//! macro-span checks happen in the HIR walker before feeding data to the pure
//! signal builder in `common`.

use std::collections::BTreeSet;

use super::MethodInfo;

/// Incrementally builds a [`MethodInfo`] from field accesses and method calls
/// observed during a method body walk.
///
/// Entries where `is_from_expansion` is `true` are silently discarded, preventing
/// macro-generated code from inflating the cohesion graph.
///
/// # Examples
///
/// ```
/// use common::lcom4::MethodInfoBuilder;
///
/// let mut builder = MethodInfoBuilder::new("process");
/// builder.record_field_access("data", false);
/// builder.record_field_access("generated_field", true); // macro — filtered
/// builder.record_method_call("validate", false);
///
/// let info = builder.build();
/// assert!(info.accessed_fields().contains("data"));
/// assert!(!info.accessed_fields().contains("generated_field"));
/// assert!(info.called_methods().contains("validate"));
/// ```
#[derive(Clone, Debug)]
pub struct MethodInfoBuilder {
    name: String,
    accessed_fields: BTreeSet<String>,
    called_methods: BTreeSet<String>,
}

impl MethodInfoBuilder {
    /// Creates a new builder for the named method with empty field and call
    /// sets.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::lcom4::MethodInfoBuilder;
    ///
    /// let builder = MethodInfoBuilder::new("read");
    /// assert!(builder.is_empty());
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            accessed_fields: BTreeSet::new(),
            called_methods: BTreeSet::new(),
        }
    }

    /// Records a field access observed in the method body.
    ///
    /// When `is_from_expansion` is `true` the access is silently ignored,
    /// preventing macro-generated field references from inflating the
    /// cohesion graph. Duplicate field names are naturally deduplicated
    /// by the underlying `BTreeSet`.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::lcom4::MethodInfoBuilder;
    ///
    /// let mut builder = MethodInfoBuilder::new("render");
    /// builder.record_field_access("canvas", false);
    /// builder.record_field_access("macro_field", true);
    ///
    /// let info = builder.build();
    /// assert!(info.accessed_fields().contains("canvas"));
    /// assert!(!info.accessed_fields().contains("macro_field"));
    /// ```
    pub fn record_field_access(&mut self, field_name: &str, is_from_expansion: bool) {
        if !is_from_expansion {
            self.accessed_fields.insert(field_name.to_owned());
        }
    }

    /// Records a method call on the same type observed in the method body.
    ///
    /// When `is_from_expansion` is `true` the call is silently ignored,
    /// preventing macro-generated calls from inflating the cohesion graph.
    /// Duplicate method names are naturally deduplicated by the underlying
    /// `BTreeSet`.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::lcom4::MethodInfoBuilder;
    ///
    /// let mut builder = MethodInfoBuilder::new("dispatch");
    /// builder.record_method_call("validate", false);
    /// builder.record_method_call("macro_helper", true);
    ///
    /// let info = builder.build();
    /// assert!(info.called_methods().contains("validate"));
    /// assert!(!info.called_methods().contains("macro_helper"));
    /// ```
    pub fn record_method_call(&mut self, method_name: &str, is_from_expansion: bool) {
        if !is_from_expansion {
            self.called_methods.insert(method_name.to_owned());
        }
    }

    /// Returns `true` when no (non-filtered) field accesses or method calls
    /// have been recorded.
    ///
    /// This is useful for lint drivers that wish to skip methods with no
    /// observable state interaction before building the full `MethodInfo`.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::lcom4::MethodInfoBuilder;
    ///
    /// let mut builder = MethodInfoBuilder::new("noop");
    /// assert!(builder.is_empty());
    ///
    /// builder.record_field_access("x", false);
    /// assert!(!builder.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accessed_fields.is_empty() && self.called_methods.is_empty()
    }

    /// Consumes the builder and returns the completed [`MethodInfo`].
    ///
    /// # Examples
    ///
    /// ```
    /// use common::lcom4::MethodInfoBuilder;
    ///
    /// let mut builder = MethodInfoBuilder::new("process");
    /// builder.record_field_access("data", false);
    /// let info = builder.build();
    ///
    /// assert_eq!(info.name(), "process");
    /// assert!(info.accessed_fields().contains("data"));
    /// ```
    #[must_use]
    pub fn build(self) -> MethodInfo {
        MethodInfo::new(self.name, self.accessed_fields, self.called_methods)
    }
}

/// Builds a [`Vec<MethodInfo>`] from an iterator of completed builders.
///
/// This is a convenience for lint drivers that collect one builder per method
/// during traversal and then need the final `MethodInfo` slice for
/// [`cohesion_components`](super::cohesion_components).
///
/// # Examples
///
/// ```
/// use common::lcom4::{MethodInfoBuilder, collect_method_infos};
///
/// let mut b1 = MethodInfoBuilder::new("read");
/// b1.record_field_access("buf", false);
/// let mut b2 = MethodInfoBuilder::new("write");
/// b2.record_field_access("buf", false);
///
/// let infos = collect_method_infos(vec![b1, b2]);
/// assert_eq!(infos.len(), 2);
/// assert_eq!(infos[0].name(), "read");
/// assert_eq!(infos[1].name(), "write");
/// ```
#[must_use]
pub fn collect_method_infos(
    builders: impl IntoIterator<Item = MethodInfoBuilder>,
) -> Vec<MethodInfo> {
    builders.into_iter().map(MethodInfoBuilder::build).collect()
}

#[cfg(test)]
mod tests {
    //! rstest-based unit tests for [`super::MethodInfoBuilder`] and
    //! [`super::collect_method_infos`].

    use super::*;
    use rstest::rstest;

    /// Applies field and call records to a builder and asserts against
    /// expected sets.
    fn assert_extraction(
        field_records: &[(&str, bool)],
        call_records: &[(&str, bool)],
        expected_fields: &[&str],
        expected_calls: &[&str],
    ) {
        let mut builder = MethodInfoBuilder::new("test_method");
        for &(name, from_exp) in field_records {
            builder.record_field_access(name, from_exp);
        }
        for &(name, from_exp) in call_records {
            builder.record_method_call(name, from_exp);
        }
        let info = builder.build();

        for &field in expected_fields {
            assert!(
                info.accessed_fields().contains(field),
                "expected field '{field}' to be present"
            );
        }
        for &method in expected_calls {
            assert!(
                info.called_methods().contains(method),
                "expected method '{method}' to be present"
            );
        }
        assert_eq!(info.accessed_fields().len(), expected_fields.len());
        assert_eq!(info.called_methods().len(), expected_calls.len());
    }

    // --- Happy paths ---

    #[rstest]
    fn single_field_access_is_recorded() {
        assert_extraction(&[("data", false)], &[], &["data"], &[]);
    }

    #[rstest]
    fn single_method_call_is_recorded() {
        assert_extraction(&[], &[("validate", false)], &[], &["validate"]);
    }

    #[rstest]
    fn multiple_fields_and_calls_accumulate() {
        assert_extraction(
            &[("alpha", false), ("beta", false)],
            &[("do_work", false), ("validate", false)],
            &["alpha", "beta"],
            &["do_work", "validate"],
        );
    }

    #[rstest]
    fn duplicate_field_names_are_deduplicated() {
        assert_extraction(&[("x", false), ("x", false)], &[], &["x"], &[]);
    }

    #[rstest]
    fn duplicate_method_names_are_deduplicated() {
        assert_extraction(&[], &[("run", false), ("run", false)], &[], &["run"]);
    }

    // --- Macro-span filtering ---

    #[rstest]
    fn field_from_expansion_is_filtered() {
        assert_extraction(&[("generated", true)], &[], &[], &[]);
    }

    #[rstest]
    fn method_from_expansion_is_filtered() {
        assert_extraction(&[], &[("macro_helper", true)], &[], &[]);
    }

    #[rstest]
    fn mixed_expansion_and_regular_entries() {
        assert_extraction(
            &[("real", false), ("generated", true)],
            &[("helper", false), ("macro_call", true)],
            &["real"],
            &["helper"],
        );
    }

    #[rstest]
    fn all_from_expansion_yields_empty_sets() {
        assert_extraction(&[("a", true), ("b", true)], &[("c", true)], &[], &[]);
    }

    // --- Edge cases ---

    #[rstest]
    fn builder_name_preserved() {
        let builder = MethodInfoBuilder::new("my_method");
        let info = builder.build();
        assert_eq!(info.name(), "my_method");
    }

    #[rstest]
    fn empty_builder_yields_empty_method_info() {
        let builder = MethodInfoBuilder::new("empty");
        let info = builder.build();
        assert!(info.accessed_fields().is_empty());
        assert!(info.called_methods().is_empty());
    }

    #[rstest]
    fn is_empty_true_when_no_records() {
        let builder = MethodInfoBuilder::new("noop");
        assert!(builder.is_empty());
    }

    #[rstest]
    fn is_empty_false_after_field_record() {
        let mut builder = MethodInfoBuilder::new("reader");
        builder.record_field_access("buf", false);
        assert!(!builder.is_empty());
    }

    #[rstest]
    fn is_empty_false_after_method_record() {
        let mut builder = MethodInfoBuilder::new("caller");
        builder.record_method_call("helper", false);
        assert!(!builder.is_empty());
    }

    #[rstest]
    fn is_empty_true_after_only_expansion_records() {
        let mut builder = MethodInfoBuilder::new("generated");
        builder.record_field_access("a", true);
        builder.record_method_call("b", true);
        assert!(builder.is_empty());
    }

    // --- collect_method_infos ---

    #[rstest]
    fn collect_empty_iterator() {
        let infos = collect_method_infos(Vec::new());
        assert!(infos.is_empty());
    }

    #[rstest]
    fn collect_preserves_order() {
        let mut b1 = MethodInfoBuilder::new("alpha");
        b1.record_field_access("x", false);
        let mut b2 = MethodInfoBuilder::new("beta");
        b2.record_method_call("alpha", false);
        let b3 = MethodInfoBuilder::new("gamma");

        let infos = collect_method_infos(vec![b1, b2, b3]);
        assert_eq!(infos.len(), 3);
        assert_eq!(infos[0].name(), "alpha");
        assert_eq!(infos[1].name(), "beta");
        assert_eq!(infos[2].name(), "gamma");
        assert!(infos[0].accessed_fields().contains("x"));
        assert!(infos[1].called_methods().contains("alpha"));
    }
}

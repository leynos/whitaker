//! Context tracking utilities for analysing traversal stacks.

use crate::attributes::{
    Attribute, AttributePath, has_test_like_attribute, has_test_like_attribute_with,
};

/// Categorises a frame within the traversal stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextKind {
    /// A free function (including methods lowered to free functions).
    Function,
    /// An implementation block.
    Impl,
    /// A module or namespace boundary.
    Module,
    /// A lexical block (e.g. closure, loop, or bare block).
    Block,
}

/// Records contextual information for traversal decisions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextEntry {
    name: String,
    kind: ContextKind,
    attributes: Vec<Attribute>,
}

impl ContextEntry {
    /// Builds a new context entry with the provided attributes.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    /// use common::context::{ContextEntry, ContextKind};
    ///
    /// let entry = ContextEntry::new(
    ///     "demo",
    ///     ContextKind::Function,
    ///     vec![Attribute::new(AttributePath::from("test"), AttributeKind::Outer)],
    /// );
    /// assert_eq!(entry.name(), "demo");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>, kind: ContextKind, attributes: Vec<Attribute>) -> Self {
        Self {
            name: name.into(),
            kind,
            attributes,
        }
    }

    /// Convenience constructor for function contexts.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    /// use common::context::ContextEntry;
    ///
    /// let entry = ContextEntry::function(
    ///     "demo",
    ///     vec![Attribute::new(AttributePath::from("test"), AttributeKind::Outer)],
    /// );
    /// assert!(entry.kind().matches_function());
    /// ```
    #[must_use]
    pub fn function(name: impl Into<String>, attributes: Vec<Attribute>) -> Self {
        Self::new(name, ContextKind::Function, attributes)
    }

    /// Returns the entry name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the entry kind.
    #[must_use]
    pub const fn kind(&self) -> &ContextKind {
        &self.kind
    }

    /// Returns a snapshot of the entry attributes.
    #[must_use]
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    /// Returns a mutable reference to the attributes for in-place updates.
    #[must_use]
    pub fn attributes_mut(&mut self) -> &mut Vec<Attribute> {
        &mut self.attributes
    }

    /// Adds an attribute to the entry.
    pub fn push_attribute(&mut self, attribute: Attribute) {
        self.attributes.push(attribute);
    }
}

impl ContextKind {
    /// Returns `true` when the kind is [`ContextKind::Function`].
    #[must_use]
    pub const fn matches_function(&self) -> bool {
        matches!(self, Self::Function)
    }
}

/// Tests whether a slice of attributes marks an item as a test function.
///
/// # Examples
///
/// ```
/// use common::attributes::{Attribute, AttributeKind, AttributePath};
/// use common::context::is_test_fn;
///
/// let attrs = vec![Attribute::new(AttributePath::from("rstest"), AttributeKind::Outer)];
/// assert!(is_test_fn(&attrs));
/// ```
#[must_use]
pub fn is_test_fn(attrs: &[Attribute]) -> bool {
    has_test_like_attribute(attrs)
}

/// Tests whether a slice of attributes marks an item as a test function while
/// honouring custom attribute paths.
///
/// # Examples
///
/// ```
/// use common::attributes::{Attribute, AttributeKind, AttributePath};
/// use common::context::is_test_fn_with;
///
/// let attrs = vec![Attribute::new(AttributePath::from("custom::test"), AttributeKind::Outer)];
/// let additional = vec![AttributePath::from("custom::test")];
/// assert!(is_test_fn_with(&attrs, &additional));
/// ```
#[must_use]
pub fn is_test_fn_with(attrs: &[Attribute], additional: &[AttributePath]) -> bool {
    has_test_like_attribute_with(attrs, additional)
}

/// Returns `true` when any entry in the stack participates in a test-like context.
///
/// # Examples
///
/// ```
/// use common::attributes::{Attribute, AttributeKind, AttributePath};
/// use common::context::{in_test_like_context, ContextEntry};
///
/// let mut entry = ContextEntry::function("demo", Vec::new());
/// entry.push_attribute(Attribute::new(AttributePath::from("test"), AttributeKind::Outer));
/// assert!(in_test_like_context(&[entry]));
/// ```
#[must_use]
pub fn in_test_like_context(stack: &[ContextEntry]) -> bool {
    in_test_like_context_with(stack, &[])
}

/// Returns `true` when any entry in the stack participates in a test-like
/// context, including those provided via the `additional` attribute paths.
///
/// # Examples
///
/// ```
/// use common::attributes::{Attribute, AttributeKind, AttributePath};
/// use common::context::{in_test_like_context_with, ContextEntry};
///
/// let mut entry = ContextEntry::function("demo", Vec::new());
/// entry.push_attribute(Attribute::new(AttributePath::from("custom::test"), AttributeKind::Outer));
/// let additional = vec![AttributePath::from("custom::test")];
/// assert!(in_test_like_context_with(&[entry], &additional));
/// ```
#[must_use]
pub fn in_test_like_context_with(stack: &[ContextEntry], additional: &[AttributePath]) -> bool {
    stack
        .iter()
        .any(|entry| has_test_like_attribute_with(entry.attributes(), additional))
}

/// Detects whether the current traversal stack is inside a `main` function.
///
/// This treats any function named `main` as an entry point. Module-qualified
/// helper functions with the same name therefore satisfy the predicate.
///
/// # Examples
///
/// ```
/// use common::context::{is_in_main_fn, ContextEntry};
///
/// let stack = vec![ContextEntry::function("main", Vec::new())];
/// assert!(is_in_main_fn(&stack));
/// ```
#[must_use]
pub fn is_in_main_fn(stack: &[ContextEntry]) -> bool {
    stack
        .iter()
        .rev()
        .any(|entry| entry.kind.matches_function() && entry.name() == "main")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attributes::{Attribute, AttributeKind, AttributePath};
    use rstest::rstest;

    fn test_attribute() -> Attribute {
        Attribute::new(AttributePath::from("test"), AttributeKind::Outer)
    }

    #[rstest]
    #[case::plain(Vec::new(), false)]
    #[case::rstest(vec![test_attribute()], true)]
    fn detects_test_functions(#[case] attrs: Vec<Attribute>, #[case] expected: bool) {
        assert_eq!(is_test_fn(&attrs), expected);
    }

    #[rstest]
    fn context_detection() {
        let mut entry = ContextEntry::function("demo", Vec::new());
        entry.push_attribute(test_attribute());
        assert!(in_test_like_context(&[entry]));
    }

    #[rstest]
    fn identifies_main() {
        let stack = vec![ContextEntry::function("main", Vec::new())];
        assert!(is_in_main_fn(&stack));
    }

    #[rstest]
    fn rejects_non_main() {
        let stack = vec![ContextEntry::function("helper", Vec::new())];
        assert!(!is_in_main_fn(&stack));
    }

    #[rstest]
    fn honours_additional_attributes() {
        let additional = vec![AttributePath::from("custom::test")];
        let attrs = vec![Attribute::new(
            AttributePath::from("custom::test"),
            AttributeKind::Outer,
        )];

        assert!(is_test_fn_with(&attrs, additional.as_slice()));

        let entry = ContextEntry::function("demo", attrs);
        assert!(in_test_like_context_with(&[entry], additional.as_slice()));
    }
}

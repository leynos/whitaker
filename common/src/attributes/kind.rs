//! Attribute classification helpers.

/// Describes whether an attribute is written as `#![...]` or `#[...]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeKind {
    /// Inner attributes appear inside an item: `#![...]`.
    Inner,
    /// Outer attributes decorate an item from the outside: `#[...]`.
    Outer,
}

impl AttributeKind {
    /// Returns `true` when the attribute is an inner attribute.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributeKind;
    ///
    /// assert!(AttributeKind::Inner.is_inner());
    /// assert!(!AttributeKind::Outer.is_inner());
    /// ```
    #[must_use]
    pub const fn is_inner(self) -> bool {
        matches!(self, Self::Inner)
    }

    /// Returns `true` when the attribute is an outer attribute.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributeKind;
    ///
    /// assert!(AttributeKind::Outer.is_outer());
    /// assert!(!AttributeKind::Inner.is_outer());
    /// ```
    #[must_use]
    pub const fn is_outer(self) -> bool {
        matches!(self, Self::Outer)
    }
}

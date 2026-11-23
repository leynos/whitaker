//! Which attribute shapes count as documentation.
//!
//! Classification is by attribute shape rather than by resolved string, so
//! `#[doc = include_str!("...")]` counts even though its value is an
//! unexpanded macro call before expansion. Counting `#[cfg_attr(..., doc =
//! "...")]` errs towards silence, which is the safe direction for a predicate
//! whose contract is to diagnose only undocumented functions.

use super::facts::Documentation;

/// The documentation-relevant shape of one function attribute.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DocAttributeShape {
    /// A sugared doc comment, `///`, `//!`, `/** */`, or `/*! */`.
    DocComment,
    /// A `doc` attribute with an `=` value, including a macro-call value.
    DocNameValue,
    /// A `cfg_attr` whose attribute list carries a `doc = ...` entry.
    CfgAttrDoc,
    /// A `doc` attribute in list form, such as `#[doc(hidden)]`.
    DocList,
    /// Any attribute that is not documentation.
    Other,
}

impl DocAttributeShape {
    /// Reports whether this shape satisfies the documentation gate.
    ///
    /// Empty documentation counts as present, following rustc's
    /// `missing_docs`; documenting content quality is out of scope.
    ///
    /// # Examples
    ///
    /// ```
    /// use missing_docs_in_test_functions::policy::DocAttributeShape;
    ///
    /// assert!(DocAttributeShape::DocComment.counts_as_documentation());
    /// assert!(!DocAttributeShape::DocList.counts_as_documentation());
    /// ```
    #[must_use]
    pub const fn counts_as_documentation(self) -> bool {
        matches!(
            self,
            Self::DocComment | Self::DocNameValue | Self::CfgAttrDoc
        )
    }
}

/// Classifies a function from the shapes of all its attributes.
///
/// # Examples
///
/// Any single documentation-shaped attribute is enough:
///
/// ```
/// use missing_docs_in_test_functions::policy::{
///     DocAttributeShape,
///     Documentation,
///     documentation_status,
/// };
///
/// let shapes = [DocAttributeShape::DocList, DocAttributeShape::DocNameValue];
/// assert_eq!(documentation_status(shapes), Documentation::Documented);
///
/// assert_eq!(documentation_status([]), Documentation::Undocumented);
/// ```
#[must_use]
pub fn documentation_status(shapes: impl IntoIterator<Item = DocAttributeShape>) -> Documentation {
    if shapes
        .into_iter()
        .any(DocAttributeShape::counts_as_documentation)
    {
        Documentation::Documented
    } else {
        Documentation::Undocumented
    }
}

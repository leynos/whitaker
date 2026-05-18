//! Pure paragraph fingerprints for repeated `rstest` setup evidence.

use std::collections::BTreeMap;

/// A deterministic local-variable slot assigned by first appearance order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalSlot(u32);

impl LocalSlot {
    /// Builds a local slot from its stable ordinal.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::LocalSlot;
    ///
    /// let slot = LocalSlot::new(0);
    /// assert_eq!(slot.index(), 0);
    /// ```
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Returns the stable slot ordinal.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// Assigns deterministic local slots by first appearance order.
#[derive(Clone, Debug, Default)]
pub struct ParagraphNormalizer {
    slots: BTreeMap<String, LocalSlot>,
    next_slot: u32,
}

impl ParagraphNormalizer {
    /// Builds an empty paragraph normalizer.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::ParagraphNormalizer;
    ///
    /// let mut normalizer = ParagraphNormalizer::new();
    ///
    /// assert_eq!(normalizer.local_slot("user").index(), 0);
    /// assert_eq!(normalizer.local_slot("cache").index(), 1);
    /// assert_eq!(normalizer.local_slot("user").index(), 0);
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: BTreeMap::new(),
            next_slot: 0,
        }
    }

    /// Returns the deterministic slot for a local name.
    ///
    /// First appearance controls numbering. Later uses of the same local name
    /// reuse the original slot.
    #[must_use]
    pub fn local_slot(&mut self, local_name: impl Into<String>) -> LocalSlot {
        let local_name = local_name.into();
        if let Some(slot) = self.slots.get(&local_name) {
            return *slot;
        }

        let slot = LocalSlot::new(self.next_slot);
        self.next_slot += 1;
        self.slots.insert(local_name, slot);
        slot
    }
}

/// A normalized callee identity used inside paragraph statements.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CalleeShape {
    /// A known canonical definition path.
    DefPath(String),
    /// A present callee whose identity is not known to the shared model.
    Unknown,
}

impl CalleeShape {
    /// Builds a known definition-path callee shape.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::CalleeShape;
    ///
    /// let callee = CalleeShape::def_path("crate::make_user");
    /// assert_eq!(callee, CalleeShape::DefPath("crate::make_user".to_string()));
    /// ```
    #[must_use]
    pub fn def_path(def_path: impl Into<String>) -> Self {
        Self::DefPath(def_path.into())
    }

    /// Builds an unknown callee shape.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::CalleeShape;
    ///
    /// assert_eq!(CalleeShape::unknown(), CalleeShape::Unknown);
    /// ```
    #[must_use]
    pub const fn unknown() -> Self {
        Self::Unknown
    }
}

/// A normalized expression shape used by paragraph statements.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ExprShape {
    /// A function call with known or unknown callee identity and arity.
    Call { callee: CalleeShape, argc: usize },
    /// A method call with method name and arity.
    MethodCall { method: String, argc: usize },
    /// A stable path expression.
    Path,
    /// A stable literal expression.
    Lit,
    /// A present expression shape outside the supported model.
    Other,
}

impl ExprShape {
    /// Builds a function-call expression shape.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::{CalleeShape, ExprShape};
    ///
    /// let shape = ExprShape::call(CalleeShape::def_path("crate::make_user"), 2);
    /// assert_eq!(
    ///     shape,
    ///     ExprShape::Call {
    ///         callee: CalleeShape::def_path("crate::make_user"),
    ///         argc: 2,
    ///     },
    /// );
    /// ```
    #[must_use]
    pub const fn call(callee: CalleeShape, argc: usize) -> Self {
        Self::Call { callee, argc }
    }

    /// Builds a method-call expression shape.
    #[must_use]
    pub fn method_call(method: impl Into<String>, argc: usize) -> Self {
        Self::MethodCall {
            method: method.into(),
            argc,
        }
    }

    /// Builds a path expression shape.
    #[must_use]
    pub const fn path() -> Self {
        Self::Path
    }

    /// Builds a literal expression shape.
    #[must_use]
    pub const fn lit() -> Self {
        Self::Lit
    }

    /// Builds an explicit unsupported expression shape.
    #[must_use]
    pub const fn other() -> Self {
        Self::Other
    }
}

/// A normalized statement shape used for paragraph grouping.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StmtShape {
    /// A `let` statement represented by its initializer shape.
    Let { init: ExprShape },
    /// A mutating call, optionally tied to a normalized local receiver slot.
    MutCall {
        receiver: Option<LocalSlot>,
        callee: CalleeShape,
    },
}

impl StmtShape {
    /// Builds a `let` statement shape from its initializer.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::{ExprShape, StmtShape};
    ///
    /// assert_eq!(
    ///     StmtShape::let_binding(ExprShape::lit()),
    ///     StmtShape::Let { init: ExprShape::Lit },
    /// );
    /// ```
    #[must_use]
    pub const fn let_binding(init: ExprShape) -> Self {
        Self::Let { init }
    }

    /// Builds a mutating-call statement shape.
    #[must_use]
    pub const fn mutable_call(receiver: Option<LocalSlot>, callee: CalleeShape) -> Self {
        Self::MutCall { receiver, callee }
    }
}

/// A normalized fingerprint for one assertion-free setup paragraph.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ParagraphFingerprint {
    shapes: Vec<StmtShape>,
}

impl ParagraphFingerprint {
    /// Builds a paragraph fingerprint from normalized statement shapes.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::rstest::{
    ///     CalleeShape, ExprShape, ParagraphFingerprint, ParagraphNormalizer,
    ///     StmtShape,
    /// };
    ///
    /// let mut normalizer = ParagraphNormalizer::new();
    /// let first = ParagraphFingerprint::new([
    ///     StmtShape::let_binding(ExprShape::call(CalleeShape::def_path("crate::build"), 0)),
    ///     StmtShape::mutable_call(Some(normalizer.local_slot("user")), CalleeShape::unknown()),
    /// ]);
    ///
    /// let mut renamed = ParagraphNormalizer::new();
    /// let second = ParagraphFingerprint::new([
    ///     StmtShape::let_binding(ExprShape::call(CalleeShape::def_path("crate::build"), 0)),
    ///     StmtShape::mutable_call(Some(renamed.local_slot("account")), CalleeShape::unknown()),
    /// ]);
    ///
    /// assert_eq!(first, second);
    /// ```
    #[must_use]
    pub fn new<I>(shapes: I) -> Self
    where
        I: IntoIterator<Item = StmtShape>,
    {
        Self {
            shapes: shapes.into_iter().collect(),
        }
    }

    /// Returns the stored statement shapes in paragraph order.
    #[must_use]
    pub fn shapes(&self) -> &[StmtShape] {
        &self.shapes
    }

    /// Consumes the fingerprint and returns the stored statement shapes.
    #[must_use]
    pub fn into_shapes(self) -> Vec<StmtShape> {
        self.shapes
    }
}

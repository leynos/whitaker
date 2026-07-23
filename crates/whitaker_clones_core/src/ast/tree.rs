//! Owned AST tree data used by parser-independent feature extraction.

use super::{AstError, AstResult};

/// Stable, opaque node-kind id lowered from the parser's syntax kind.
///
/// `KindId` is only for equality and bucketing. It must not be persisted
/// outside parser-versioned outputs.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::ast::KindId;
///
/// assert_eq!(KindId::new(7).get(), 7);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KindId(u16);

impl KindId {
    /// Creates an opaque syntax-kind identifier.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Returns the opaque numeric value.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Tree depth relative to the lowered subtree root.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::ast::Depth;
///
/// assert_eq!(Depth::root().get(), 0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Depth(u16);

impl Depth {
    /// Returns the root depth.
    #[must_use]
    pub const fn root() -> Self {
        Self(0)
    }

    /// Creates a depth value.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Returns the underlying depth.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Normalized leaf token class for Type-2-style leaf erasure.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::ast::LeafClass;
///
/// assert_eq!(LeafClass::Ident, LeafClass::Ident);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LeafClass {
    /// Identifier-like leaf.
    Ident,
    /// Literal-like leaf.
    Literal,
    /// Any other lowered leaf.
    Other,
}

/// Owned parser-agnostic AST node.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::ast::{KindId, NormalizedNode};
///
/// let node = NormalizedNode::new(KindId::new(1), None, Vec::new());
/// assert_eq!(node.kind(), KindId::new(1));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedNode {
    kind: KindId,
    leaf: Option<LeafClass>,
    children: Vec<NormalizedNode>,
}

impl NormalizedNode {
    /// Creates a parser-independent AST node.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if `leaf` is `Some` and `children` is non-empty.
    /// A lowered leaf token carries no substructure, so the AST domain keeps the
    /// leaf tag and children mutually exclusive; canonical hashing and
    /// leaf-erasure feature extraction rely on that invariant to stay
    /// unambiguous.
    #[must_use]
    pub fn new(kind: KindId, leaf: Option<LeafClass>, children: Vec<NormalizedNode>) -> Self {
        debug_assert!(
            leaf.is_none() || children.is_empty(),
            "a leaf-tagged NormalizedNode must have no children"
        );
        Self {
            kind,
            leaf,
            children,
        }
    }

    /// Returns the node kind.
    #[must_use]
    pub const fn kind(&self) -> KindId {
        self.kind
    }

    /// Returns the optional leaf class.
    #[must_use]
    pub const fn leaf(&self) -> Option<LeafClass> {
        self.leaf
    }

    /// Returns the ordered child nodes.
    #[must_use]
    pub fn children(&self) -> &[NormalizedNode] {
        &self.children
    }
}

/// Lowered candidate subtree plus its source span.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::ast::{ByteSpan, KindId, NormalizedNode, NormalizedTree};
///
/// let span = ByteSpan::new("fn f() {}", 0, 2)?;
/// let tree = NormalizedTree::new(NormalizedNode::new(KindId::new(1), None, Vec::new()), span);
/// assert_eq!(tree.span(), span);
/// # Ok::<(), whitaker_clones_core::AstError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedTree {
    root: NormalizedNode,
    span: ByteSpan,
}

impl NormalizedTree {
    /// Creates a lowered tree.
    #[must_use]
    pub const fn new(root: NormalizedNode, span: ByteSpan) -> Self {
        Self { root, span }
    }

    /// Returns the lowered root node.
    #[must_use]
    pub const fn root(&self) -> &NormalizedNode {
        &self.root
    }

    /// Returns the source span represented by this tree.
    #[must_use]
    pub const fn span(&self) -> ByteSpan {
        self.span
    }
}

/// Half-open byte span over source text.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::ByteSpan;
///
/// let span = ByteSpan::new("let x = 1;", 0, 3)?;
/// assert_eq!((span.start(), span.end()), (0, 3));
/// # Ok::<(), whitaker_clones_core::AstError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteSpan {
    start: u32,
    end: u32,
}

impl ByteSpan {
    /// Validates and creates a half-open byte span.
    pub fn new(source_text: &str, start: u32, end: u32) -> AstResult<Self> {
        if end < start {
            return Err(AstError::InvalidSpan { start, end });
        }
        if start == end {
            return Err(AstError::EmptySpan { offset: start });
        }

        let len = source_text.len();
        let start_usize = start as usize;
        let end_usize = end as usize;
        if end_usize > len {
            return Err(AstError::SpanOutOfBounds { start, end, len });
        }
        if !source_text.is_char_boundary(start_usize) {
            return Err(AstError::NonCharBoundary { offset: start });
        }
        if !source_text.is_char_boundary(end_usize) {
            return Err(AstError::NonCharBoundary { offset: end });
        }

        Ok(Self { start, end })
    }

    /// Returns the start offset.
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    /// Returns the exclusive end offset.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }
}

//! Where a function sits in the item tree, and the stack that tracks it.
//!
//! Rustc's early-lint visitor announces an item before walking its children and
//! announces it again afterwards, so a stack that pushes exactly one frame in
//! every pre-hook and pops exactly one in every post-hook is balanced by
//! construction. The classification itself is a small recursion: a function is
//! in scope only when every enclosing item is a module, and an associated
//! function additionally takes the kind of its parent container.

/// One item that can contain other items, as far as this lint cares.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ContainerKind {
    /// An inline or out-of-line module.
    Module,
    /// An inherent implementation block, `impl Type`.
    InherentImpl,
    /// A trait implementation block, `impl Trait for Type`.
    TraitImpl,
    /// A trait declaration.
    Trait,
    /// Any other item: a function, constant, static, type, or block.
    Other,
}

/// The syntactic position a function occupies.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FunctionShape {
    /// A free function written as an item, `fn f() {}`.
    Free,
    /// An associated function written inside an `impl` or `trait`.
    Associated,
}

/// Where a function sits relative to module boundaries.
///
/// In scope are free functions whose every enclosing item is a module, inherent
/// associated functions in the same position, and trait methods with a default
/// body. Methods in trait implementations are excluded because their
/// documentation conventionally lives on the trait.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FunctionContext {
    /// A free function at module level.
    Module,
    /// An associated function in an inherent `impl` block at module level.
    InherentImpl,
    /// A trait method with a default body at module level.
    Trait,
    /// A method in a trait implementation.
    TraitImpl,
    /// A function nested inside a non-module item.
    Nested,
}

/// The stack of items currently being walked, outermost first.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContainerStack {
    frames: Vec<ContainerKind>,
}

impl ContainerStack {
    /// Pushes one frame for the item whose children are about to be walked.
    pub fn enter(&mut self, kind: ContainerKind) { self.frames.push(kind); }

    /// Pops the innermost frame, returning it, or `None` when empty.
    ///
    /// An unbalanced post-hook must not panic: the driver recomputes the
    /// expected kind and reports a mismatch instead.
    #[must_use]
    pub fn exit(&mut self) -> Option<ContainerKind> { self.frames.pop() }

    /// Classifies a function from the frames enclosing it.
    ///
    /// # Examples
    ///
    /// A free function at the crate root has no enclosing frames, so every
    /// enclosing item is vacuously a module and the function is in scope.
    ///
    /// ```
    /// # #![cfg_attr(feature = "dylint-driver", feature(rustc_private))]
    /// # #[cfg(feature = "dylint-driver")] extern crate rustc_driver;
    /// use missing_docs_in_test_functions::policy::{ContainerStack, FunctionContext, FunctionShape};
    ///
    /// let stack = ContainerStack::default();
    /// assert_eq!(
    ///     stack.classify(FunctionShape::Free),
    ///     FunctionContext::Module,
    ///     "a free function at the crate root is a module function"
    /// );
    /// ```
    #[must_use]
    pub fn classify(&self, shape: FunctionShape) -> FunctionContext {
        match shape {
            FunctionShape::Free => self.module_level_context(FunctionContext::Module),
            FunctionShape::Associated => self.associated_context(),
        }
    }

    /// Returns `context` when every enclosing frame is a module.
    fn module_level_context(&self, context: FunctionContext) -> FunctionContext {
        if self.is_module_level() {
            context
        } else {
            FunctionContext::Nested
        }
    }

    /// Classifies an associated function by the kind of its parent.
    fn associated_context(&self) -> FunctionContext {
        let Some((parent, enclosing)) = self.frames.split_last() else {
            // An associated item is always visited inside its `impl` or
            // `trait` frame, so an empty stack means the inputs are not the
            // ones this classification is defined for.
            return FunctionContext::Nested;
        };
        let context = match parent {
            ContainerKind::InherentImpl => FunctionContext::InherentImpl,
            ContainerKind::Trait => FunctionContext::Trait,
            ContainerKind::TraitImpl => FunctionContext::TraitImpl,
            ContainerKind::Module | ContainerKind::Other => return FunctionContext::Nested,
        };
        if enclosing.iter().all(|kind| *kind == ContainerKind::Module) {
            context
        } else {
            FunctionContext::Nested
        }
    }

    /// Reports whether every enclosing frame is a module.
    fn is_module_level(&self) -> bool {
        self.frames
            .iter()
            .all(|kind| *kind == ContainerKind::Module)
    }
}

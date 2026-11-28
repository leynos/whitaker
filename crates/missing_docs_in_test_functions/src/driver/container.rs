//! Maps AST items onto the container frames the stack tracks.
//!
//! Both functions are total: every item kind maps to some frame. The driver
//! relies on that totality to keep the stack balanced, because it pushes one
//! frame in every pre-hook and pops one in every post-hook without exception.

use rustc_ast::ast::{AssocItemKind, ItemKind};

use crate::policy::ContainerKind;

/// Returns the container frame introduced by an item.
///
/// Only modules, implementation blocks, and trait declarations introduce a
/// container this lint distinguishes; every other item — including a function,
/// whose body may hold nested items — is [`ContainerKind::Other`].
pub(super) const fn item_container_kind(kind: &ItemKind) -> ContainerKind {
    match kind {
        ItemKind::Mod(..) => ContainerKind::Module,
        ItemKind::Impl(impl_block) => {
            if impl_block.of_trait.is_some() {
                ContainerKind::TraitImpl
            } else {
                ContainerKind::InherentImpl
            }
        }
        ItemKind::Trait(_) => ContainerKind::Trait,
        _ => ContainerKind::Other,
    }
}

/// Returns the container frame introduced by an associated item.
///
/// Associated items are always leaves for this lint: an `impl` written inside
/// an associated function's body is reached through the ordinary item hook, not
/// through the associated-item hook, so no associated item can enclose another.
pub(super) const fn associated_item_container_kind(_kind: &AssocItemKind) -> ContainerKind {
    ContainerKind::Other
}

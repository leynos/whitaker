//! Quality gates for localisation resources.
//!
//! These tests keep placeholder usage aligned across locales, ensure Welsh help
//! text remains complete, and exercise language-specific plural forms so we can
//! catch regressions before they reach users.

mod suite;

pub use fluent_templates::fluent_bundle::FluentResource;
pub use suite::get_all_ftl_files;

#[cfg(test)]
mod ftl_smoke_behaviour;

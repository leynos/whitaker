//! Rebuild the embedded Fluent bundle when locale resources change.
//!
//! `fluent_templates::static_loader!` scans `locales/` while the macro
//! expands, so a resource file added afterwards is invisible to Cargo's
//! dependency tracking: the crate is not rebuilt, the compiled bundle keeps
//! its previous file set, and a lookup for the new key silently resolves
//! through the `en-GB` fallback. Watching the directory makes adding a locale
//! file rebuild the bundle, which is what the developers' guide promises.

/// Register the locale tree as a build input.
fn main() {
    println!("cargo:rerun-if-changed=locales");
}

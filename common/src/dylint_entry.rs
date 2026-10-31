//! Macro support for declaring Dylint library entry points.
//!
//! Dylint locates a library's lints by calling an exported `register_lints`
//! symbol, which requires `#[unsafe(no_mangle)]`. Libraries registering more
//! than one lint cannot use `dylint_linting`'s single-lint macros, and the
//! workspace forbids in-crate `unsafe` code, so this macro provides the same
//! externally-expanded escape hatch those macros rely on: the unsafe
//! attribute originates in this crate's macro definition, keeping consumer
//! crates free of unsafe tokens.
//!
//! Scope and reuse policy: intended solely for Whitaker Dylint driver crates
//! (currently `whitaker_suite`) that must export a combined `register_lints`
//! entry point. It must not be used outside Dylint entry-point wiring.

/// Declares the `register_lints` entry point Dylint resolves from a lint
/// library.
///
/// The single argument is a path to a function with the signature
/// `fn(&rustc_session::Session, &mut rustc_lint::LintStore)`; it is invoked
/// with the compiler session and lint store whenever Dylint loads the
/// library. The expansion resolves `rustc_session` and `rustc_lint` at the
/// call site, so the invoking crate must have those crates as dependencies.
///
/// # Examples
///
/// ```ignore
/// fn register_entry(sess: &rustc_session::Session, store: &mut rustc_lint::LintStore) {
///     dylint_linting::init_config(sess);
///     store.register_lints(MY_LINT_DECLS);
/// }
///
/// whitaker_common::declare_dylint_register_entry!(register_entry);
/// ```
#[macro_export]
macro_rules! declare_dylint_register_entry {
    ($register:path) => {
        /// Dylint entry point that forwards to the library's registration
        /// function.
        #[unsafe(no_mangle)]
        pub fn register_lints(
            sess: &rustc_session::Session,
            lint_store: &mut rustc_lint::LintStore,
        ) {
            $register(sess, lint_store);
        }
    };
}

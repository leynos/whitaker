//! Compile-time guards for the root CLI's public contracts.
//!
//! The root binary composes `clap` parsing with `ortho_config` so command-line
//! values and environment configuration share one structure. The derive
//! combination is a dependency contract rather than a runtime behaviour: an
//! `ortho_config` release that required a trait the `clap` derive does not
//! satisfy, or a `serde` bound the pair cannot meet, would break every user's
//! build rather than one test. `tests/ortho_config_compat.rs` covers the
//! parsed values at runtime; these fixtures pin the fact that the derives
//! compile together at all.

/// Keep the `OrthoConfig` and `clap` derives composable.
///
/// The fixture applies all four derives to one type, which is what the root
/// CLI does. It fails to compile if the crates disagree about the bounds their
/// derives require.
#[test]
fn ortho_config_and_clap_derives_compile_together() {
    trybuild::TestCases::new().pass("tests/ui/pass_ortho_config_with_clap.rs");
}

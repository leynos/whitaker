//! Build configuration guards for scoped dynamic-linking expectations.
//!
//! Flags that link the compiler's private crates belong to the individual
//! Make recipes that need them, never to the auto-discovered configuration:
//! Cargo reads `.cargo/config.toml` from every build beneath this directory,
//! including the ones `whitaker-installer` runs for a user. This guard holds
//! the decision recorded in ADR 010 and the per-invocation policy stated at
//! the top of `.cargo/config.toml`
//! (`docs/adrs/adr-0010-keep-the-fast-linker-out-of-the-auto-discovered-cargo-config.md`).

use toml::Value;

mod workspace_support;

use workspace_support::workspace_root;

#[test]
fn cargo_config_keeps_dynamic_linking_out_of_workspace_configuration() {
    let root = whitaker_common::test_support::fixture_fs::open_root(workspace_root())
        .unwrap_or_else(|err| panic!("failed to open the workspace root: {err}"));
    let contents = root
        .read_to_string(".cargo/config.toml")
        .unwrap_or_else(|err| panic!("failed to read .cargo/config.toml: {err}"));
    let value: Value = toml::from_str(&contents).expect("cargo config should parse as TOML table");

    // Cargo accepts both an array and a whitespace-separated string here, so
    // the guard must reject the key itself rather than one of its shapes.
    let rustflags = value.get("build").and_then(|table| table.get("rustflags"));

    assert!(
        rustflags.is_none(),
        "ADR 010 keeps `.cargo/config.toml` free of rustflags: dynamic-linker flags belong to the \
         Dylint-aware Make recipes, which restate them per invocation, not to the configuration \
         every build beneath this directory inherits: {rustflags:?}"
    );
}

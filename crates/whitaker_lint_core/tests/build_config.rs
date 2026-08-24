//! Build configuration guards for scoped dynamic-linking expectations.

use std::fs;

use toml::Value;

mod workspace_support;

use workspace_support::workspace_root;

#[test]
fn cargo_config_keeps_dynamic_linking_out_of_workspace_configuration() {
    let config_path = workspace_root().join(".cargo/config.toml");
    let contents = fs::read_to_string(&config_path)
        .unwrap_or_else(|err| panic!("failed to read {config_path:?}: {err}"));
    let value: Value = toml::from_str(&contents).expect("cargo config should parse as TOML table");

    let rustflags = value
        .get("build")
        .and_then(|table| table.get("rustflags"))
        .and_then(Value::as_array);

    assert!(
        rustflags.is_none(),
        "dynamic linker flags belong to the Dylint-aware Make recipes, not workspace configuration"
    );
}

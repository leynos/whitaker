//! Build configuration guards for dynamic linking expectations.

use std::fs;
use std::path::Path;

use toml::Value;

#[test]
fn cargo_config_prefers_dynamic_linking() {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cargo/config.toml");
    let contents = fs::read_to_string(&config_path)
        .unwrap_or_else(|err| panic!("failed to read {:?}: {err}", config_path));
    let value: Value = toml::from_str(&contents).expect("cargo config should parse as TOML table");

    let rustflags = value
        .get("build")
        .and_then(|table| table.get("rustflags"))
        .and_then(Value::as_array)
        .expect("build.rustflags should be an array");

    // Expect exactly the pair ["-C", "prefer-dynamic"] to guard against regressions.
    let flags: Vec<&str> = rustflags
        .iter()
        .map(|v| v.as_str().expect("rustflags entries should be strings"))
        .collect();
    assert_eq!(
        flags,
        ["-C", "prefer-dynamic"],
        "rustflags must prefer dynamic linkage"
    );
}

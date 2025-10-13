const MANIFEST_TEMPLATE: &str = r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
dylint_linting = { workspace = true }
common = { path = "../../common" }

[dev-dependencies]
whitaker = { path = "../../" }
"#;

const LIB_RS_TEMPLATE: &str = r#"//! Lint crate for `{crate_name}`.
//!
//! Replace the placeholder implementation with crate-specific logic before shipping.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

use dylint_linting::{declare_late_lint, impl_late_lint};
use rustc_lint::{LateContext, LateLintPass};

declare_late_lint!(
    pub {lint_constant},
    Warn,
    "replace the message with a short lint description",
);

pub struct {pass_struct};

impl_late_lint! {
    {lint_constant},
    {pass_struct},

    fn check_crate<'tcx>(
        &mut self,
        _cx: &LateContext<'tcx>,
        _krate: &'tcx rustc_hir::Crate<'tcx>,
    ) {
        // TODO: Update the lint implementation.
    }
}

#[cfg(test)]
mod tests {
    whitaker::declare_ui_tests!("{ui_tests_directory}");
}
"#;

pub(crate) fn render_manifest(crate_name: &str) -> String {
    MANIFEST_TEMPLATE.replace("{crate_name}", crate_name)
}

pub(crate) fn render_lib_rs(
    crate_name: &str,
    lint_constant: &str,
    pass_struct: &str,
    ui_tests_directory: &str,
) -> String {
    let mut output = LIB_RS_TEMPLATE.replace("{crate_name}", crate_name);
    output = output.replace("{lint_constant}", lint_constant);
    output = output.replace("{pass_struct}", pass_struct);
    output.replace(
        "{ui_tests_directory}",
        &escape_rust_string_literal(ui_tests_directory),
    )
}

fn escape_rust_string_literal(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_manifest_injects_crate_name() {
        let rendered = render_manifest("demo_lint");
        assert!(rendered.contains(r#"name = "demo_lint""#));
    }

    #[test]
    fn render_lib_rs_escapes_ui_directory() {
        let rendered = render_lib_rs("demo_lint", "DEMO_LINT", "DemoLint", "ui/space \"quote\"");
        assert!(rendered.contains(r#"whitaker::declare_ui_tests!("ui/space \"quote\"");"#));
    }
}

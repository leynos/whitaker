//! Renders lint crate manifest and source templates.

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
    render_template(MANIFEST_TEMPLATE, &[("crate_name", crate_name)])
}

pub(crate) fn render_lib_rs(
    crate_name: &str,
    lint_constant: &str,
    pass_struct: &str,
    ui_tests_directory: &str,
) -> String {
    let escaped_ui = escape_rust_string_literal(ui_tests_directory);
    render_template(
        LIB_RS_TEMPLATE,
        &[
            ("crate_name", crate_name),
            ("lint_constant", lint_constant),
            ("pass_struct", pass_struct),
            ("ui_tests_directory", escaped_ui.as_str()),
        ],
    )
}

fn render_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut remainder = template;

    while let Some((before, after_open)) = remainder.split_once('{') {
        output.push_str(before);

        let Some(first) = after_open.as_bytes().first().copied() else {
            output.push('{');
            remainder = "";
            break;
        };

        if !first.is_ascii_alphabetic() && first != b'_' {
            output.push('{');
            remainder = after_open;
            continue;
        }

        if let Some((key, rest)) = after_open.split_once('}') {
            if let Some((_, value)) = replacements.iter().find(|(name, _)| *name == key) {
                output.push_str(value);
            } else {
                output.push('{');
                output.push_str(key);
                output.push('}');
            }
            remainder = rest;
        } else {
            output.push('{');
            output.push_str(after_open);
            remainder = "";
            break;
        }
    }

    output.push_str(remainder);
    output
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

    #[test]
    fn render_lib_rs_escapes_backslashes_and_newlines() {
        let rendered = render_lib_rs(
            "demo_lint",
            "DEMO_LINT",
            "DemoLint",
            "ui/wave\\multiline\ncase",
        );
        assert!(rendered.contains(r#"whitaker::declare_ui_tests!("ui/wave\\multiline\ncase");"#));
    }

    #[test]
    fn render_lib_rs_handles_empty_ui_directory() {
        let rendered = render_lib_rs("demo_lint", "DEMO_LINT", "DemoLint", "");
        assert!(rendered.contains(r#"whitaker::declare_ui_tests!("");"#));
    }
}

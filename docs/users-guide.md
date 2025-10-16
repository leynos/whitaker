# Whitaker User's Guide

Whitaker ships helpers that streamline the creation of new Dylint crates. This
guide explains how to scaffold a lint crate using the shared template and how
to run the accompanying UI tests.

## Generating a lint crate from the template

The `whitaker::LintCrateTemplate` helper emits both a `Cargo.toml` manifest and
a baseline `src/lib.rs`. The manifest reuses the workspace dependency versions,
including the `rustc_*` proxy crates that re-export nightly compiler APIs,
whilst the library source wires in Whitaker's UI harness.

1. Create a directory for the lint under `crates/`.
2. Use the template to generate files, for example via a short build script:

   ```rust
   use cap_std::{ambient_authority, fs::Dir};
   use whitaker::LintCrateTemplate;

   fn main() -> Result<(), Box<dyn std::error::Error>> {
       let template = LintCrateTemplate::new("function_attrs_follow_docs")?;
       let files = template.render();

       let root = Dir::open_ambient_dir(".", ambient_authority())?;
       root.create_dir_all("crates/function_attrs_follow_docs/src")?;
       root.write("crates/function_attrs_follow_docs/Cargo.toml", files.manifest())?;
       root.write("crates/function_attrs_follow_docs/src/lib.rs", files.lib_rs())?;
       Ok(())
   }
   ```

3. Populate `ui/` fixtures for the lint. The generated `lib.rs` already declares
   the canonical `whitaker::declare_ui_tests!("ui")` test.

`LintCrateTemplate::with_ui_tests_directory` targets alternative directories
provided the path is relative. The helper normalizes Windows-style separators
to forward slashes and rejects traversal via `..` so test harnesses stay within
the crate. Template construction fails fast on empty crate names, uppercase
characters, trailing separators, or absolute paths so mistakes are surfaced
before any files are written.

## Running lint UI tests

Run `make test` from the workspace root to execute unit, behaviour, and UI
harness tests. The shared target enables `rstest` fixtures and `rstest-bdd`
scenarios, ensuring each lint crate benefits from the consistent test harness.

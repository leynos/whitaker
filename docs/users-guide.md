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

## `function_attrs_follow_docs`

Whitaker's first lint checks that doc comments sit in front of all other outer
attributes on functions, inherent methods, and trait methods. Attribute-based
documentation (`#[doc = "..."]`) is treated identically to `///` comments. When
the docs slip behind `#[inline]` or `#[allow]`, the lint emits a warning that
highlights both the misplaced comment and the attribute it should precede.

The UI fixtures demonstrate accepted and rejected layouts across the three
function kinds, while behaviour tests walk through happy, unhappy, and edge
cases. To fix the warning, move the doc comment so it appears before every
other outer attribute.

## `no_expect_outside_tests`

Whitaker's restriction lint forbids calling `.expect(...)` on `Option` or
`Result` receivers outside test contexts. The analysis inspects method calls,
confirms the receiver resolves to one of the two panic-producing types, and
walks the HIR ancestor chain to detect enclosing test attributes or `cfg(test)`
modules. When the call occurs in production code, the diagnostic explains which
function triggered the lint and echoes the receiver type, helping teams decide
where error handling should live.

Attributes that merely add metadata under `cfg_attr(test, ...)` do not mark an
item as test-only: the lint only treats the code as guarded when the attribute
directly applies a `cfg(test)` gate (for example, via
`cfg_attr(test, cfg(test))`). This prevents production functions that enable
extra warnings or allowances in tests from slipping past the check.

The UI fixtures demonstrate accepted usage inside a `#[test]` function and the
failures emitted for ordinary functions. Behaviour-driven tests cover context
summaries for non-test functions, explicit test attributes, and modules guarded
by `cfg(test)`.

Doctests compiled by `rustdoc` are detected via the compiler-provided
`Crate::is_doctest` flag. When that flag is set, the lint pass skips all
checks, allowing documentation examples to continue using `.expect(...)` while
keeping production code guarded.

The recognised test attributes can be extended through `dylint.toml` when teams
rely on bespoke harness macros. Add the fully qualified attribute paths under
the lint's configuration namespace:

```toml
[tool.dylint.libraries.no_expect_outside_tests]
additional_test_attributes = ["my_framework::test"]
```

Any functions annotated with those attributes are treated as tests for the
purpose of this lint, matching the behaviour of built-in markers such as
`#[test]` and `#[rstest]`.

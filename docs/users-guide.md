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

## Getting the lints

Install `cargo-dylint` and `dylint-link` once, then load the Whitaker lint
suite directly from Git so the exact binaries that will ship are tested:

```sh
cargo dylint list --git https://github.com/leynos/whitaker --rev v0.1.0 --all
```

Swap `v0.1.0` for the tag to exercise; omit `--rev` or set `GIT_TAG=HEAD` to
trial the current branch tip.

## Running lint UI tests

Run `make test` from the workspace root to execute unit, behaviour, and UI
harness tests. The shared target enables `rstest` fixtures and `rstest-bdd`
scenarios, ensuring each lint crate benefits from the consistent test harness.

## Localised diagnostics

Whitaker bundles Fluent resources under `locales/` so every lint can present
messages in multiple languages. The `common::i18n::Localizer` helper resolves
message strings and attributes, reporting when the fallback `en-GB` bundle is
used. Secondary `cy` (Welsh) and `gd` (Scottish Gaelic) locales demonstrate how
to translate each lint slug and drive behaviour tests that exercise non-English
lookups, including plural handling for languages with richer category sets.

Language selection should use `common::i18n::available_locales()` to enumerate
the compiled locales. When an unsupported locale is requested, the loader falls
back to the bundled `en-GB` strings and surfaces a missing message error if a
slug is not translated.

Workspaces can pin the active locale through the `DYLINT_LOCALE` environment
variable or the `locale` entry in `dylint.toml`. The
`common::i18n::resolve_localizer` helper combines explicit overrides with the
environment and configuration, trimming whitespace and warning about
unsupported locales before falling back to the bundled English strings. This
ordering keeps CI deterministic while still allowing developers to override the
locale for ad hoc smoke tests.

Whitaker lints source their primary messages, notes, and help text directly
from Fluent bundles at emission time. Each diagnostic assembles structured
arguments—such as the offending attribute snippet or the receiver type—so
translations never depend on hand-built strings. If a lookup fails, the lint
records a delayed span bug and falls back to deterministic English text, which
keeps builds actionable while signalling that the localisation bundle needs an
update.

```rust
use common::i18n::{
    available_locales, Arguments, Localizer, FALLBACK_LOCALE, branch_phrase,
};
use common::i18n::FluentValue;
use std::borrow::Cow;
use std::collections::HashMap;

let preferred = "gd";
assert!(available_locales().contains(&preferred.to_string()));

let localizer = Localizer::new(Some(preferred));

let mut args: Arguments<'static> = HashMap::new();
let branch_count = 3;
let branch_limit = 2;
args.insert(Cow::Borrowed("name"), FluentValue::from("match on Foo"));
args.insert(Cow::Borrowed("branches"), FluentValue::from(branch_count));
args.insert(Cow::Borrowed("limit"), FluentValue::from(branch_limit));
let branch_phrase_text = branch_phrase(localizer.locale(), branch_count as usize);
args.insert(
    Cow::Borrowed("branch_phrase"),
    FluentValue::String(Cow::Owned(branch_phrase_text)),
);
let limit_phrase_text = branch_phrase(localizer.locale(), branch_limit as usize);
args.insert(
    Cow::Borrowed("limit_phrase"),
    FluentValue::String(Cow::Owned(limit_phrase_text)),
);

let message = localizer
    .message_with_args("conditional_max_n_branches", &args)?;
let note = localizer
    .attribute_with_args("conditional_max_n_branches", "note", &args)?;

if localizer.used_fallback() {
    eprintln!("Fell back to {FALLBACK_LOCALE}");
}
```

## `no_unwrap_or_else_panic`

Purpose: deny panicking `unwrap_or_else` fallbacks on `Option` / `Result`
outside tests and doctests, closing the loophole where panics are hidden inside
the fallback closure. Plain `.unwrap()` / `.expect(...)` remain unaffected by
this lint.

Scope and behaviour:

- Triggers only on `unwrap_or_else` when the receiver is `Option` or `Result`.
- Detects panics via a shared path list (`core::panicking::panic`, `panic_fmt`,
  `panic_any`, `begin_panic`, and their `std::panicking` counterparts) and via
  inner `unwrap` / `expect` inside the closure body.
- Skips doctests (`UNSTABLE_RUSTDOC_TEST_PATH` set) and test-like contexts.
- Config knob `no_unwrap_or_else_panic.allow_in_main = true` (default false)
  permits panicking fallbacks inside `main`.

Configuration (in `dylint.toml`):

```toml
[tool.dylint.libraries.no_unwrap_or_else_panic]
allow_in_main = true
```

What is allowed:

- Plain `.unwrap()` / `.expect(...)` (covered by other policies).
- `unwrap_or_else` with a non-panicking fallback (e.g. error propagation,
  returning defaults).
- Test and doctest code.

What is denied:

- `unwrap_or_else(|| panic!(..))`, `panic_any`, or a fallback that panics via
  an inner `unwrap` / `expect`, when used in production code and not exempted
  by configuration.

How to fix:

- Propagate the error (`?`, `map_err`, custom error types) or use `expect` with
  a clear message if a panic is truly intended.
- For `main`-only panics, set `no_unwrap_or_else_panic.allow_in_main = true` in
  `dylint.toml`.

Tests:

- UI fixtures cover: direct panics, `panic_any`, inner-unwrap panics,
  allow-in-main, test/doctest skips, safe fallbacks, non-Option/Result
  receivers, and the explicit allowance of plain `unwrap`/`expect`.
- Unit tests exercise the pure policy (`should_flag`) and panic detector path
  matching.

______________________________________________________________________

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

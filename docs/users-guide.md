# Whitaker User's Guide

Whitaker is a collection of opinionated Dylint lints for Rust. This guide
explains how to install and configure the lints, use the installer command-line
interface (CLI), and customise behaviour for a given project.

For contributors who want to add new lints, see the section on generating lint
crates from the template.

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

## Using whitaker-installer

The `whitaker-installer` CLI builds, links, and stages Dylint lint libraries
for local use. This approach is useful when pre-built libraries are preferred
rather than building from source on each `cargo dylint` invocation.

### Prerequisites

Before using `whitaker-installer`, ensure the following are available:

- `cargo-dylint` and `dylint-link` installed (`cargo install cargo-dylint
  dylint-link`)
- A compatible Rust nightly toolchain (the installer detects this from
  `rust-toolchain.toml`)

### Basic usage

Clone the Whitaker repository and run the installer from the workspace root:

```sh
git clone https://github.com/leynos/whitaker.git
cd whitaker
cargo run --release -p whitaker-installer
```

By default, this builds only the aggregated suite (a single library containing
all lints) and stages it to a platform-specific directory under
`<toolchain>/release`:

- Linux: `~/.local/share/dylint/lib/<toolchain>/release`
- macOS: `~/Library/Application Support/dylint/lib/<toolchain>/release`
- Windows: `%LOCALAPPDATA%\dylint\lib\<toolchain>\release`

For example, with toolchain `nightly-2025-01-15`, the Linux path would be
`~/.local/share/dylint/lib/nightly-2025-01-15/release`.

### Installation modes

Build-specific lints by name (can be repeated):

```sh
whitaker-installer -l module_max_lines -l no_expect_outside_tests
```

Build all individual lint crates instead of the suite:

```sh
whitaker-installer --individual-lints
```

### Configuration options

| Option                  | Short | Description                                  |
| ----------------------- | ----- | -------------------------------------------- |
| `--target-dir DIR`      | `-t`  | Staging directory for built libraries        |
| `--lint NAME`           | `-l`  | Build-specific lint (repeatable)             |
| `--individual-lints`    | `—`   | Build individual crates instead of the suite |
| `--toolchain TOOLCHAIN` | `—`   | Override the detected toolchain              |
| `--jobs N`              | `-j`  | Number of parallel build jobs                |
| `--dry-run`             | `—`   | Show what would be done without running      |
| `--verbose`             | `-v`  | Increase output verbosity (repeatable)       |
| `--quiet`               | `-q`  | Suppress output except errors                |

### Shell configuration

After installation, configure the shell to find the staged libraries. The
installer prints the exact path including the toolchain subdirectory. Example
snippets for common shells (replace `<toolchain>` with the actual toolchain,
e.g., `nightly-2025-01-15`):

```sh
# bash/zsh (~/.bashrc, ~/.zshrc)
export DYLINT_LIBRARY_PATH="$HOME/.local/share/dylint/lib/<toolchain>/release"

# fish (~/.config/fish/config.fish)
set -gx DYLINT_LIBRARY_PATH "$HOME/.local/share/dylint/lib/<toolchain>/release"

# PowerShell ($PROFILE)
$env:DYLINT_LIBRARY_PATH = "$HOME/.local/share/dylint/lib/<toolchain>/release"
```

### Running installed lints

With `DYLINT_LIBRARY_PATH` set, run the lints without workspace metadata:

```sh
cargo dylint --all
```

Or combine with workspace metadata for hybrid setups where some lints are
pre-built and others are fetched from Git.

## Quick Setup

To integrate Whitaker lints into a project, add the following to the workspace
`Cargo.toml`:

```toml
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "suite" }
]
```

Then run the lints across the workspace:

```sh
cargo dylint --all
```

### Suite vs individual crates

Whitaker offers two ways to load lints:

- **Aggregated suite** (`pattern = "suite"`): All lints in a single library.
  This is the recommended approach for most projects.
- **Individual crates**: Specify each lint explicitly by name. This allows
  selective loading and independent version pinning.

To load specific individual lints instead of the full suite:

```toml
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "crates/module_max_lines" },
  { git = "https://github.com/leynos/whitaker", pattern = "crates/no_expect_outside_tests" }
]
```

Use individual crates when only a subset of lints is required or when specific
lints must be pinned to different versions.

### Experimental lints

Whitaker includes an experimental "Bumpy Road" detector
(`bumpy_road_function`). It is excluded from the aggregated suite by default.
To opt in, load the lint crate explicitly alongside the suite:

```toml
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "suite" },
  { git = "https://github.com/leynos/whitaker", pattern = "crates/bumpy_road_function" }
]
```

### Version pinning

For reproducible builds, pin to a specific release tag or commit:

```toml
# Release tag
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "suite", tag = "v0.1.0" }
]
```

```toml
# Commit hash
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "suite", rev = "abc123def456" }
]
```

### Using pre-built libraries

If lints have been installed via `whitaker-installer`, configure Dylint to use
the staged libraries. The path must include the toolchain and release
subdirectories:

```toml
[workspace.metadata.dylint]
libraries = [
  { path = "/home/user/.local/share/dylint/lib/nightly-2025-01-15/release" }
]
```

This skips the build step entirely, providing faster lint runs at the cost of
managing library updates manually.

### Lint configuration

Configure individual lint behaviour in `dylint.toml` at the workspace root:

```toml
# Diagnostic language (default: en-GB)
locale = "cy"

# Module size threshold (default: 400)
[module_max_lines]
max_lines = 500

# Conditional branch limit (default: 2)
[conditional_max_n_branches]
max_branches = 3

# Custom test attributes
[no_expect_outside_tests]
additional_test_attributes = ["my_framework::test", "async_std::test"]

# Allow panics in main
[no_unwrap_or_else_panic]
allow_in_main = true
```

See individual lint sections below for available configuration options.

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

______________________________________________________________________

## `module_must_have_inner_docs`

Purpose: enforce that every module begins with an inner documentation comment.
This lint helps maintain consistent documentation practices by requiring all
modules to explain their purpose at the beginning of their definition.

Scope and behaviour:

- Inspects all non-macro modules (both inline `mod foo { .. }` and file-backed
  modules).
- Emits a warning when a module's body does not start with a `//!` style
  comment or `#![doc = "..."]` attribute.
- The doc comment must appear before other inner attributes.
- Skips macro-generated modules automatically.

What is allowed:

- Modules beginning with `//!` comments.
- Modules beginning with `#![doc = "..."]` attributes.
- Modules with doc comments wrapped in `#![cfg_attr(...)]` (if the doc is
  present).
- Macro-generated modules (automatically ignored).

What is denied:

- Modules without leading documentation.
- Modules with other attributes appearing before the doc comment.

How to fix:

Add an inner doc comment at the very beginning of the module body:

```rust
mod undocumented {
    //! Explain the module's purpose here.
    pub fn value() {}
}
```

Or use the attribute form:

```rust
mod documented {
    #![doc = "Module documentation using attributes"]
    pub fn value() {}
}
```

______________________________________________________________________

## `module_max_lines`

Purpose: measure module size and warn when modules exceed a configurable line
count threshold. This promotes code maintainability by encouraging developers
to keep modules reasonably sized and focused.

Scope and behaviour:

- Measures the number of source lines occupied by each module.
- Counts from the first to the last line of the module body.
- Emits a warning when module line count exceeds the configured limit.
- Ignores macro-generated modules.

Configuration (in `dylint.toml`):

```toml
[module_max_lines]
max_lines = 400
```

The default threshold is 400 lines. Adjust this to match project conventions.

What is allowed:

- Modules with line count at or below the configured limit.
- Macro-generated modules (automatically ignored).

What is denied:

- Modules exceeding the configured line limit.

How to fix:

Split the module into smaller submodules:

```rust
// Before: Single 500-line module
mod large_module {
    // 500 lines of code...
}

// After: Split into focused modules
mod domain_a {
    //! Handles feature A.
}

mod domain_b {
    //! Handles feature B.
}
```

______________________________________________________________________

## `conditional_max_n_branches`

Purpose: limit the complexity of conditional predicates by enforcing a maximum
number of boolean branches. This improves code readability and testability by
preventing overly complex conditions in if/while statements and match guards.

Scope and behaviour:

- Counts boolean branches (AND/OR operations) in predicates.
- Inspects `if` conditions, `while` loop conditions, and `match` guard
  expressions.
- Counts branches recursively through binary operations (`&&` and `||`).
- Emits a warning when branch count exceeds the configured limit.

Configuration (in `dylint.toml`):

```toml
[conditional_max_n_branches]
max_branches = 2
```

The default threshold is 2 branches. A predicate like `a && b && c` has three
branches and would trigger the lint at the default setting.

What is allowed:

- Predicates with branch count at or below the configured limit.
- Single conditions and simple boolean combinations within the limit.

What is denied:

- Predicates exceeding the configured branch limit.

How to fix:

Extract complex conditions into helper functions:

```rust
// Before: Too many branches
if condition_a && condition_b && condition_c {
    // action
}

// After: Extract to helper function
fn should_proceed() -> bool {
    condition_a && condition_b && condition_c
}

if should_proceed() {
    // action
}
```

______________________________________________________________________

## `no_std_fs_operations`

Purpose: enforce capability-based filesystem access by forbidding direct use of
`std::fs` operations. This lint promotes a security model where filesystem
access is mediated through capability-bearing handles (`cap_std`) rather than
relying on the ambient working directory.

Scope and behaviour:

- Detects all imports of `std::fs` items (`use std::fs::...`).
- Detects all calls to `std::fs` functions.
- Detects type references to `std::fs` types (structs, aliases).
- Detects struct literals using `std::fs` types.

What is allowed:

- Using `cap_std::fs::Dir` handles for filesystem operations.
- Using `camino::Utf8Path` and `camino::Utf8PathBuf` for path handling.
- Capability-based approaches to filesystem access.

What is denied:

- `use std::fs::{...}` imports.
- Direct calls to any `std::fs` operation.
- Creating instances of `std::fs` types.

How to fix:

Replace `std::fs` with capability-based alternatives using `cap_std`:

```rust
// Before: Direct std::fs usage
use std::fs;

fn read_config() -> std::io::Result<String> {
    fs::read_to_string("config.toml")
}

// After: Capability-based with cap_std
use cap_std::fs::Dir;
use camino::Utf8Path;

fn read_config(config_dir: &Dir, path: &Utf8Path) -> std::io::Result<String> {
    config_dir.read_to_string(path)
}
```

Key principles:

- Pass `cap_std::fs::Dir` handles as parameters.
- Use `camino::Utf8Path` / `camino::Utf8PathBuf` for paths.
- Avoid ambient working directory assumptions.

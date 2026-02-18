# Whitaker Developer's Guide

This guide is for contributors who want to develop new lints or work on
Whitaker itself. For using Whitaker lints in a project, see the
[User's Guide](users-guide.md).

## Prerequisites

- Rust nightly toolchain (version specified in `rust-toolchain.toml`)
- `cargo-dylint` and `dylint-link` installed:

  ```sh
  cargo install cargo-dylint dylint-link
  ```

## Running Tests

Run the full test suite from the workspace root:

```sh
make test
```

This executes unit, behaviour, and UI harness tests. The shared target enables
`rstest` fixtures and `rstest-bdd` scenarios.

Other useful commands:

```sh
make lint       # Run Clippy
make check-fmt  # Verify formatting
make fmt        # Apply formatting
```

## Using whitaker-installer

The `whitaker-installer` command-line interface (CLI) builds, links, and stages
Dylint lint libraries for local development. This avoids rebuilding from source
on each `cargo dylint` invocation.

### Basic usage

From the workspace root:

```sh
cargo run --release -p whitaker-installer
```

Or install it globally:

```sh
cargo install --path installer
whitaker-installer
```

By default, this builds the aggregated suite and stages it to a
platform-specific directory:

- Linux: `~/.local/share/dylint/lib/<toolchain>/release`
- macOS: `~/Library/Application Support/dylint/lib/<toolchain>/release`
- Windows: `%LOCALAPPDATA%\dylint\lib\<toolchain>\release`

When a prebuilt artefact is available, the installer extracts it to the
Whitaker data directory keyed by toolchain and target:

- Linux:
  `~/.local/share/whitaker/lints/<toolchain>/<target>/lib`
- macOS:
  `~/Library/Application Support/whitaker/lints/<toolchain>/<target>/lib`
- Windows:
  `%LOCALAPPDATA%\whitaker\lints\<toolchain>\<target>\lib`

### Configuration options

- `-t, --target-dir DIR` — Staging directory for built libraries
- `-l, --lint NAME` — Build specific lint (repeatable)
- `--individual-lints` — Build individual crates instead of the suite
- `--experimental` — Include experimental lints in the build
- `--toolchain TOOLCHAIN` — Override the detected toolchain
- `-j, --jobs N` — Number of parallel build jobs
- `--dry-run` — Show what would be done without running
- `-v, --verbose` — Increase output verbosity (repeatable)
- `-q, --quiet` — Suppress output except errors
- `--skip-deps` — Skip `cargo-dylint`/`dylint-link` installation check
- `--skip-wrapper` — Skip wrapper script generation
- `--no-update` — Don't update existing repository clone

### Using installed lints

After installation, set `DYLINT_LIBRARY_PATH` to the staged directory:

```sh
export DYLINT_LIBRARY_PATH="$HOME/.local/share/dylint/lib/nightly-2025-01-15/release"
cargo dylint --all
```

For prebuilt installs, use the toolchain-and-target-specific directory:

```sh
export DYLINT_LIBRARY_PATH="$HOME/.local/share/whitaker/lints/nightly-2025-01-15/x86_64-unknown-linux-gnu/lib"
cargo dylint --all
```

Alternatively, configure workspace metadata to use the pre-built libraries
directly:

```toml
[workspace.metadata.dylint]
libraries = [
  { path = "/home/user/.local/share/whitaker/lints/nightly-2025-01-15/x86_64-unknown-linux-gnu/lib" }
]
```

This skips building entirely, providing faster lint runs during development.

## Standard vs Experimental Lints

Whitaker categorizes lints into two tiers:

- **Standard lints** are stable, well-tested, and included in the default suite.
  They have predictable behaviour with minimal false positives.
- **Experimental lints** are newer or more aggressive checks that may produce
  false positives or undergo breaking changes. They require explicit opt-in via
  the `--experimental` flag.

### Adding a new lint

New lints should typically start as experimental. To add a lint:

1. Create the lint crate under `crates/` (see
   [Creating a New Lint](#creating-a-new-lint))
2. Add the crate name to `EXPERIMENTAL_LINT_CRATES` in
   `installer/src/lint_crates.rs`
3. Add a feature flag for the lint in `suite/Cargo.toml` under `[features]`

### Promoting to standard

Once an experimental lint has been:

- Tested across multiple real-world codebases
- Refined to minimize false positives
- Stabilized with no breaking changes planned

It can be promoted to standard by:

1. Moving the crate name from `EXPERIMENTAL_LINT_CRATES` to `LINT_CRATES`
2. Adding the lint to the suite's default features in `suite/Cargo.toml`
3. Updating documentation to reflect the change

## Creating a New Lint

### Generating from the template

The `whitaker::LintCrateTemplate` helper generates both a `Cargo.toml` manifest
and a baseline `src/lib.rs`:

1. Create a directory for the lint under `crates/`.
2. Use the template to generate files:

   ```rust
   use cap_std::{ambient_authority, fs::Dir};
   use whitaker::LintCrateTemplate;

   fn main() -> Result<(), Box<dyn std::error::Error>> {
       let template = LintCrateTemplate::new("my_new_lint")?;
       let files = template.render();

       let root = Dir::open_ambient_dir(".", ambient_authority())?;
       root.create_dir_all("crates/my_new_lint/src")?;
       root.write("crates/my_new_lint/Cargo.toml", files.manifest())?;
       root.write("crates/my_new_lint/src/lib.rs", files.lib_rs())?;
       Ok(())
   }
   ```

3. Populate `ui/` fixtures for the lint. The generated `lib.rs` already declares
   the canonical `whitaker::declare_ui_tests!("ui")` test.

### Template options

`LintCrateTemplate::with_ui_tests_directory` targets alternative directories.
The helper:

- Normalizes Windows-style separators to forward slashes
- Rejects traversal via `..`
- Fails fast on empty names, uppercase characters, trailing separators, or
  absolute paths

### UI test fixtures

Create test fixtures under `crates/my_new_lint/ui/`:

- `pass_*.rs` - Code that should pass the lint
- `fail_*.rs` - Code that should trigger the lint
- `fail_*.stderr` - Expected diagnostic output

## Testing Lints from Git

To test lints directly from a Git repository without installing:

```sh
cargo dylint list --git https://github.com/leynos/whitaker --rev v0.1.0 --all
```

Swap `v0.1.0` for the tag to test, or omit `--rev` to use the current branch
tip.

## Localized Diagnostics

Whitaker supports multiple locales for diagnostic messages. Fluent resources
are bundled under `locales/`.

### Available locales

- `en-GB` (default) - English
- `cy` - Welsh (Cymraeg)
- `gd` - Scottish Gaelic (Gàidhlig)

### Using the Localizer API

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

### Locale resolution

Language selection uses `common::i18n::available_locales()` to enumerate
compiled locales. When an unsupported locale is requested, the loader falls
back to `en-GB` and surfaces a missing message error if a slug is not
translated.

Locale can be set via:

1. `DYLINT_LOCALE` environment variable
2. `locale` entry in `dylint.toml`

The `common::i18n::resolve_localizer` helper combines explicit overrides with
environment and configuration, trimming whitespace and warning about
unsupported locales.

### Adding translations

Lints source messages directly from Fluent bundles at emission time. Each
diagnostic assembles structured arguments, so translations never depend on
hand-built strings. If a lookup fails, the lint records a delayed span bug and
falls back to deterministic English text.

To add a new locale:

1. Create a new directory under `locales/` (e.g., `locales/fr/`)
2. Add `.ftl` files with translated messages
3. Update `common::i18n::available_locales()` to include the new locale

## Publishing

Before publishing, run the full validation suite:

```sh
make publish-check
```

This builds, tests, and validates packages in a production-like environment
without the `prefer-dynamic` flag used during development.

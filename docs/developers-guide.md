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

Run the test suite from the workspace root:

```sh
make test
```

This executes unit, behaviour, and UI harness tests. The shared target enables
`rstest` fixtures and `rstest-bdd` scenarios.

### Test profiles

By default, `make test` excludes slow installer integration tests
(`behaviour_toolchain` and `behaviour_cli`) via a nextest default-filter
defined in `.config/nextest.toml`. These tests perform real `rustup` installs
and `cargo` builds, so they can take upwards of fifteen minutes. Note that the
exclusion relies on hardcoded binary names in `.config/nextest.toml`; renaming
or splitting these test binaries requires updating the filter to match (see
[#180][issue-180]).

To run the full suite including installer tests, pass the `ci` profile:

```sh
make test NEXTEST_PROFILE=ci
```

Continuous Integration (CI) always uses the `ci` profile, so installer tests
are never silently skipped in the pipeline.

Table: Test profiles and typical usage.

| Profile   | What runs                                  | Typical use        |
| --------- | ------------------------------------------ | ------------------ |
| (default) | All tests **except** installer integration | Local development  |
| `ci`      | All tests                                  | CI and pre-release |

When working on `whitaker-installer` code, run the full suite locally before
pushing to catch installer regressions early.

### Other useful commands

```sh
make lint       # Run Clippy
make check-fmt  # Verify formatting
make fmt        # Apply formatting
```

## Regression infrastructure

Two recent regression families rely on infrastructure that is easy to miss when
adding coverage or refactoring helpers.

### Async test harness detection

`no_expect_outside_tests` prefers source-level test attributes such as
`#[test]`, `#[rstest]`, and `#[tokio::test]`. In real `rustc --test`
compilations, async wrappers can lose that source-level marker and instead be
represented by a sibling `#[rustc_test_marker = "..."] const ...` descriptor.
The driver therefore falls back to matching that harness descriptor by symbol
name plus source range when direct attribute detection fails.

Keep the regression split aligned with that compiler boundary:

- Source-level attribute-shape coverage belongs in `ui/` fixtures.
- Regressions that need `--test`, example targets, or extra compiler flags
  belong in `crates/no_expect_outside_tests/src/lib_ui_tests.rs`.
- Real async framework regressions should use `examples/` targets when the bug
  depends on the same lowering path external consumers hit.

This separation exists so a proc-macro stub test cannot accidentally mask a
failure in the real harness-descriptor path.

### Staged-suite installer shortcut

Installer behavioural tests occasionally need the suite staging path without
recursively rebuilding the workspace from inside `nextest`. The debug-only
helper in `installer/src/staged_suite.rs` provides that shortcut.

- Behavioural tests opt in with `WHITAKER_INSTALLER_TEST_STAGE_SUITE=1`.
- The helper only activates for an exact suite-only request
  (`whitaker_suite` and nothing else).
- The helper returns `Ok(None)` in release binaries before reading the
  environment variable, so production installers never stage the placeholder
  artefact.

Use this hook only for installer orchestration tests. Release validation,
prebuilt-download coverage, and user-facing installation flows must continue to
exercise the real build or download paths.

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
- `--experimental` — Include experimental lints in the build (none currently)
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

At present, all shipped Whitaker lints are standard.

### Adding a new lint

New lints should typically start as experimental. To add a lint:

1. Create the lint crate under `crates/` (see
   [Creating a New Lint](#creating-a-new-lint))
2. Add the crate name to `EXPERIMENTAL_LINT_CRATES` in
   `installer/src/resolution.rs`
3. Add a feature flag for the lint in `suite/Cargo.toml` under `[features]`

### Promoting to standard

Once an experimental lint has been:

- Tested across multiple real-world codebases
- Refined to minimize false positives
- Stabilized with no breaking changes planned

It can be promoted to standard by:

1. Moving the crate name from `EXPERIMENTAL_LINT_CRATES` to `LINT_CRATES`
2. Adding the lint dependency to the suite `dylint-driver` feature in
   `suite/Cargo.toml`
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

## Release tooling

Whitaker includes tooling for automating release-related tasks.

### `scripts/generate_checksums.py`

This script generates SHA-256 checksum files for release archives. It is
integrated into the rolling-release workflow to produce `.sha256` files for
all published archives.

#### Usage

Generate checksums for archives in the default `dist/` directory:

```sh
python scripts/generate_checksums.py
```

Generate checksums for archives in a custom directory:

```sh
python scripts/generate_checksums.py /path/to/archives
```

#### Public API

The script exposes the following functions for programmatic use:

- **`compute_sha256(path: Path) -> str`** — Computes the SHA-256 hex digest for
  a file using streaming reads to handle large files without memory pressure.

  ```python
  from pathlib import Path
  from scripts.generate_checksums import compute_sha256

  digest = compute_sha256(Path("archive.tgz"))
  print(f"SHA-256: {digest}")
  ```

- **`find_archives(directory: Path) -> list[Path]`** — Discovers archive files
  matching the configured patterns (`*.tgz`, `*.zip`). Returns a sorted list of
  paths. Raises `NoArchivesFoundError` if no matching archives are found.

  ```python
  from pathlib import Path
  from scripts.generate_checksums import find_archives

  archives = find_archives(Path("dist"))
  for archive in archives:
      print(f"Found: {archive.name}")
  ```

- **`generate_checksums(directory: Path) -> None`** — Generates `.sha256` files
  for all archives in the specified directory. Checksum files are written in
  the format `HASH  FILENAME\n` for compatibility with standard verification
  tools.

  ```python
  from pathlib import Path
  from scripts.generate_checksums import generate_checksums

  generate_checksums(Path("dist"))  # Creates dist/*.sha256 files
  ```

#### Exceptions

- **`NoArchivesFoundError`** — Raised when `find_archives()` or
  `generate_checksums()` cannot locate any archive files matching the configured
  patterns. This exception indicates either an empty directory or a path
  mismatch.

#### Integration with release workflow

The script is invoked by the rolling-release workflow after archives are
packaged. Checksum files are uploaded alongside archives as workflow artefacts,
allowing users to verify download integrity using standard tools (see the
[README](../README.md) for platform-specific verification instructions).

## Publishing

Before publishing, run the full validation suite:

```sh
make publish-check
```

This builds, tests, and validates packages in a production-like environment
without the `prefer-dynamic` flag used during development.

[issue-180]: https://github.com/leynos/whitaker/issues/180

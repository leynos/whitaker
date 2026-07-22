# whitaker-installer

Installer CLI for [Whitaker](https://github.com/leynos/whitaker) Dylint lint
libraries.

Whitaker is a collection of opinionated Dylint lints for Rust. This installer
builds, links, and stages the lint libraries for local use, avoiding the need
to rebuild from source on each `cargo dylint` invocation. It also ensures the
pinned Rust toolchain and required components are installed via rustup.

Pass `--cranelift` when your project, CI, or build configuration requires the
Cranelift back-end (`rustc-codegen-cranelift`). `rustc-codegen-cranelift` is an
alternative compiler back-end that uses the Cranelift code generator instead of
LLVM, which can produce faster debug builds. Some Whitaker configurations (or
CI environments that pre-install it) require it to be available alongside the
standard toolchain components. Without `--cranelift`, the installer only
provisions the standard required toolchain components; the flag adds
`rustc-codegen-cranelift` to that set so `rustup component add` includes it in
a single step.

## Installation

```bash
cargo install whitaker-installer
```

## Usage

### Install the default lint suite

```bash
whitaker-installer
```

This builds and stages the aggregated suite containing all standard lints.

### Install with experimental lints

```bash
whitaker-installer --experimental
```

At present, there are no experimental lints, so this flag is reserved for
future lint previews.

### Install specific lints

```bash
whitaker-installer -l module_max_lines -l no_expect_outside_tests
```

### Install all individual lint crates

```bash
whitaker-installer --individual-lints
```

### List installed lints

```bash
whitaker-installer list
```

Output as JSON for scripting:

```bash
whitaker-installer list --json
```

### Preview without building

```bash
whitaker-installer --dry-run
```

## Available Lints

Whitaker lints are divided into two categories:

- **Standard lints** are stable, well-tested, and included in the default suite.
  They are recommended for general use.
- **Experimental lints** are newer or more aggressive checks that may produce
  false positives or undergo breaking changes. They require the
  `--experimental` flag to install.

### Standard Lints

These lints are included when running `whitaker-installer` without flags:

| Lint                          | Description                                      |
| ----------------------------- | ------------------------------------------------ |
| `bumpy_road_function`         | Detect multiple complexity clusters in functions |
| `conditional_max_n_branches`  | Limit boolean branches in conditionals           |
| `function_attrs_follow_docs`  | Doc comments must precede other attributes       |
| `module_max_lines`            | Warn when modules exceed line threshold          |
| `module_must_have_inner_docs` | Require inner doc comments on modules            |
| `no_expect_outside_tests`     | Forbid `.expect()` outside test contexts         |
| `test_must_not_have_example`  | Forbid examples in test documentation            |
| `no_std_fs_operations`        | Enforce capability-based filesystem access       |
| `no_unwrap_or_else_panic`     | Deny panicking `unwrap_or_else` fallbacks        |

### Experimental Lints

There are currently no experimental lints. The `--experimental` flag remains
available for future releases that add preview lints.

## Using the Installed Lints

After installation, set `DYLINT_LIBRARY_PATH` to the staged directory and run
`cargo dylint`:

```bash
cargo dylint --all
```

The installer generates wrapper scripts and provides shell configuration
snippets to simplify this setup.

Dependency-tool verification is asymmetric by design:

- `cargo-dylint` is checked by running `cargo dylint --version`.
- `dylint-link` is never executed. It is a linker wrapper that forwards its
  entire argument list to the underlying linker, so it has no reliable
  self-reporting subcommand: `--version` exits early, and `--help` depends on
  a usable linker and toolchain in the ambient environment. Executing it as a
  health check rejects valid artefacts. A Cargo-managed `dylint-link` is
  instead checked by two independent conditions: an executable file resolves on
  `PATH`, and Cargo's recorded installed version for the `dylint-link` package
  matches the expected version. A repository-release `dylint-link` is
  trusted after its install pipeline succeeds (see below).

For repository-release installs, the trust boundary is the pipeline itself: the
release asset name pins the package and version, the `.sha256` sidecar
establishes integrity, extraction confirms the expected archive member, and the
permission step establishes launch eligibility. Genuine failures in any of
those steps — a missing asset, checksum mismatch, failed download or
extraction, or an unwritable executable — still fall back to Cargo.

On Windows, the installer honours `PATHEXT` while scanning `PATH`, so the
normal Cargo-installed `dylint-link.exe` and other shell-resolved executable
suffixes are recognized.

The wrappers are:

- `whitaker` — runs `cargo dylint` with the staged library path.
- `whitaker-ls` — lists installed Whitaker suite libraries for the staged
  path.

## Licence

ISC

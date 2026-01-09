# whitaker-installer

Installer CLI for [Whitaker](https://github.com/leynos/whitaker) Dylint lint
libraries.

Whitaker is a collection of opinionated Dylint lints for Rust. This installer
builds, links, and stages the lint libraries for local use, avoiding the need
to rebuild from source on each `cargo dylint` invocation.

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

| Lint                          | Description                                |
| ----------------------------- | ------------------------------------------ |
| `conditional_max_n_branches`  | Limit boolean branches in conditionals     |
| `function_attrs_follow_docs`  | Doc comments must precede other attributes |
| `module_max_lines`            | Warn when modules exceed line threshold    |
| `module_must_have_inner_docs` | Require inner doc comments on modules      |
| `no_expect_outside_tests`     | Forbid `.expect()` outside test contexts   |
| `no_std_fs_operations`        | Enforce capability-based filesystem access |
| `no_unwrap_or_else_panic`     | Deny panicking `unwrap_or_else` fallbacks  |

### Experimental Lints

These lints require `whitaker-installer --experimental` to install:

| Lint                  | Description                            |
| --------------------- | -------------------------------------- |
| `bumpy_road_function` | Detect high nesting depth in functions |

Experimental lints may be promoted to standard in future releases once they
have been sufficiently tested in real-world codebases.

## Using the Installed Lints

After installation, set `DYLINT_LIBRARY_PATH` to the staged directory and run
`cargo dylint`:

```bash
cargo dylint --all
```

The installer generates wrapper scripts and provides shell configuration
snippets to simplify this setup.

## Licence

ISC

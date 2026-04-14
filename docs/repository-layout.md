# Repository layout

This document is for contributors who need a quick map of the Whitaker
repository before changing code, tests, packaging, or documentation.

## Workspace overview

Whitaker is a Cargo workspace with shared infrastructure at the repository
root, reusable support code in `common/`, production lint crates under
`crates/`, installer code in `installer/`, and supporting test and
documentation material alongside them. The root `Cargo.toml` and `Cargo.lock`
define the workspace, while the root `Makefile` provides the canonical
developer entry points for formatting, linting, testing, publishing checks, and
formal verification helpers.

## Top-level directories

Table: Top-level repository layout.

| Path         | Purpose                                                                                                                               |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| `.cargo/`    | Cargo configuration shared across the workspace.                                                                                      |
| `.config/`   | Project configuration such as the nextest profile and test filters.                                                                   |
| `.github/`   | Continuous Integration (CI), release, and automation workflows.                                                                       |
| `common/`    | Shared analysis, diagnostics, localization, decomposition, and test-support code used across multiple lint crates.                    |
| `crates/`    | Individual lint crates, selected vendored `rustc_*` compatibility crates, and support crates such as clone-analysis and SARIF output. |
| `docs/`      | User, developer, design, roadmap, decision-record, and planning documentation.                                                        |
| `installer/` | The `whitaker-installer` crate, packaging logic, and installer-specific behaviour tests.                                              |
| `scripts/`   | Repository automation for checksums, Kani, and Verus setup and execution.                                                             |
| `src/`       | Root library code that assembles shared lint registration and workspace-facing APIs.                                                  |
| `suite/`     | The suite crate used to package and expose the lint set coherently.                                                                   |
| `tests/`     | Workspace-level integration, behaviour, UI harness, and workflow tests.                                                               |
| `verus/`     | Verus proof sidecars and formal verification experiments.                                                                             |

## Shared repository files

- `AGENTS.md` defines repository-wide working rules for automated contributors.
- `README.md` provides the public-facing project overview and quick start.
- `Makefile` is the canonical entry point for local validation commands such as
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie`.
- `rust-toolchain.toml` pins the Rust toolchain used across development and CI.
- `.markdownlint-cli2.jsonc`, `clippy.toml`, and `dylint.toml` capture
  repository linting and formatting policy.

## Rust workspace layout

### `common/`

The `common/` crate holds reusable building blocks that would otherwise be
duplicated across lint crates. Its `src/` tree contains shared diagnostics,
complexity metrics, decomposition advice, internationalization helpers, and
test-support utilities. Its `tests/` tree contains behaviour-driven coverage
for those shared building blocks.

### `crates/`

The `crates/` directory contains the individual lint implementations and a
small set of support crates:

- Lint crates such as `bumpy_road_function/`,
  `conditional_max_n_branches/`, `function_attrs_follow_docs/`,
  `module_max_lines/`, `module_must_have_inner_docs/`,
  `no_expect_outside_tests/`, `no_std_fs_operations/`,
  `no_unwrap_or_else_panic/`, and `test_must_not_have_example/`.
- Support crates such as `whitaker_clones_core/` and `whitaker_sarif/`.
- Vendored compatibility crates such as `rustc_ast/`, `rustc_hir/`, and other
  `rustc_*` crates used to align with the Rust compiler interfaces Whitaker
  targets.

Most lint crates follow a repeatable shape with `src/` for implementation,
`tests/` for crate-specific tests, and `ui/` fixtures for compiler-diagnostic
testing. Some crates also include locale-specific UI directories or `examples/`
where the lint needs extra coverage material.

### `installer/`

The `installer/` directory contains the `whitaker-installer` crate. Its `src/`
tree covers command-line parsing, staging, dependency resolution, artefact
handling, and install workflows. Its `tests/` tree contains larger behaviour
suites, feature files, and workflow-oriented checks that exercise the installer
as a product rather than only as a library.

### `src/` and `suite/`

The repository root `src/` directory holds the top-level library entry points
and shared lint-registration glue. The `suite/` crate packages the lint set as
an integrated suite and carries its own source and tests.

## Testing and verification assets

- `tests/` holds workspace-level integration tests, behaviour-driven tests,
  feature files, UI harness coverage, and workflow validation support.
- `common/tests/` and the per-crate `tests/` directories hold crate-scoped
  behavioural and integration coverage.
- Per-crate `ui/`, `ui-cy/`, `ui-gd/`, and similar directories store compiler
  fixture inputs and expected diagnostics.
- `scripts/run-kani.sh` and `scripts/run-verus.sh` are the entry points for the
  formal verification tooling described in the developer documentation.
- `verus/` stores proof-focused material that sits alongside, rather than
  inside, the production crates.

## Documentation and planning assets

- `docs/users-guide.md` is the primary user-facing guide.
- `docs/developers-guide.md` is the primary maintainer-facing guide.
- `docs/contents.md` is the documentation index and should be updated whenever
  documents are added, removed, or renamed.
- `docs/roadmap.md` tracks longer-running product and implementation work.
- `docs/execplans/` stores execution plans for branch-specific tasks and larger
  follow-up efforts.

## How to orient quickly

When starting unfamiliar work, use this sequence:

1. Read `AGENTS.md` for repository-wide contribution rules.
2. Read `docs/contents.md` to find the user, developer, design, and roadmap
   documents relevant to the task.
3. Use this repository layout document to locate the code, tests, fixtures, and
   scripts that implement the area you need to change.
4. Use the `Makefile` targets rather than ad hoc command sequences when
   validating changes.

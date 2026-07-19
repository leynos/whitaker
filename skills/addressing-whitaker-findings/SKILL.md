---
name: addressing-whitaker-findings
description: Best practices for fixing Whitaker Dylint suite findings — per-lint remediation playbook, exclusion policy, and the interaction traps discovered during the leynos estate rollout
---

# Addressing Whitaker findings

Use this skill when a repository fails the Whitaker Dylint suite and the
findings need fixing. It distils the remediation patterns from adopting
the suite across forty-plus repositories: what each lint wants, which
fixes hold up under review, when an exclusion is legitimate, and the
traps that cost earlier adopters rework.

## Working stance

- Prefer a real fix to a suppression, and a suppression to a lie. The
  suite encodes house policy; most findings point at genuine debt.
- Findings arrive in waves. `cargo dylint` stops at the first failing
  crate, so expect build script → lib → lib tests → each integration
  test crate, and budget three to four fix/re-run cycles on a repository
  of any size. Re-run after every round; do not assume the first report
  is complete.
- The suite runs with `--all-targets`, which compiles benches and
  examples that ordinary CI may never build. Expect to find (and fix)
  bit-rotted benches as a side effect.
- After refactoring, re-check module sizes: extracting helpers to fix
  `bumpy_road_function` can push a module over `module_max_lines`.

## Per-lint playbook

### `no_expect_outside_tests` and `no_unwrap_or_else_panic`

The policy behind these lints: **fixture and helper functions are not
tests.** A fixture arranges state; arrangement can fail, so it must
return `Result` and propagate errors. Only the test body may unwrap: a
failure there is the test verdict.

- Convert fixtures and helpers to return `anyhow::Result`,
  `io::Result`, or the domain error; tests consume with `?` or unwrap
  in the body.
- A poisoned mutex means a panic occurred while its lock was held. In
  test doubles, use `PoisonError::into_inner` only when the double's
  invariants remain valid; otherwise propagate the poison error or fail
  the fixture.
- `#[tokio::main]` expands to `.expect()` on runtime construction:
  build the runtime explicitly in binaries and BDD harnesses and
  propagate the error.
- Shared `LazyLock` statics that panic on construction should store
  `Result` and surface the error at each use site.
- Non-panicking `unwrap_or_else` fallbacks whose closure contains a
  macro (`tracing::debug!` and similar) still trip the lint, which
  cannot see through the expansion; rewrite as `if let`/`else` (a
  plain `match` may then trip clippy's `single_match_else`).

**Proc-macro erasure — functions the lint cannot recognize as tests.**
Attributes are gone by the time lints see HIR, so these are treated as
production code even inside `cfg(test)`:

- rstest `#[fixture]` functions,
- cucumber step functions (and `additional_test_attributes` cannot
  help, because the macros consume their attributes),
- `#[serial]`-wrapped `#[test]` functions.

In each case, make the function fallible instead of suppressing.
Plain `#[test]` and `#[rstest]` bodies ARE recognized.

**Structural no-propagation contexts** get a documented escape hatch,
not a scattering of expects: an extension trait (e.g. `ExpectValid`)
for proptest strategy pipelines, or a single documented helper (e.g.
`expect_bench`) for Criterion closures. One named, explained panic
boundary beats twenty anonymous ones.

### Assertions (house style, enforced by review if not by lint)

Assertions follow command–query separation: run the fallible operation
as its own statement and handle or unwrap its error there, then assert
on the result of a pure query. Never bury a fallible call inside
`assert!`/`assert_eq!`. When several tests share the same
query-plus-assertion shape, extract a custom assertion **macro** (not
a helper function) so panic line numbers keep pointing at the calling
test — and note that macros are the only option where the assertion
appears in `#[case(...)]` attributes.

### `no_std_fs_operations`

Prefer real fixes first:

- Route file access through `cap_std` `Dir` handles opened at a
  well-defined root (workspace, fixture directory).
- Tempfile writes can go through `NamedTempFile`'s own `Write` impl
  rather than `as_file_mut()`, avoiding the `std::fs::File` receiver
  the lint keys on.
- Test fixture staging can use cap-std throughout (create_dir_all,
  write, set_permissions, read_to_string all exist on `Dir`).

Know cap-std's limits before forcing it: `Dir::metadata` refuses
symlinks that leave the directory, so PATH-probing code (`which`-style
resolution of `/usr/bin/cc -> /etc/alternatives/cc`) cannot be
capability-scoped. Irreducibly ambient operations belong in a small
**boundary crate** (netsuke's `ambient_fs` is the reference) with a
documented scope-and-reuse policy, excluded in `dylint.toml`.

**In-source suppression does not work for this lint** (issue #270):
`allow` leaves the diagnostic firing and `expect` adds an unfulfilled-
expectation error on top. The only working escape hatch is crate-level
`[no_std_fs_operations] excluded_crates` in the root `dylint.toml`.
Sanctioned exclusion categories, each with a rationale comment:

- `build_script_build` — build scripts get ambient paths from Cargo.
- Test-support crates and integration-test crates that stage fixtures
  ambiently (the crate name of `tests/foo.rs` is `foo`).
- Boundary crates holding deliberately ambient probes.
- Crates whose purpose is ambient filesystem or process management
  (a daemon preparing its socket directory; an embedded-database
  bootstrapper; a pyo3 crate mirroring CPython file-handler APIs).
  Say so in the comment, and name the migration path if one exists.

New test crates that use `std::fs` must either migrate to the
capability-scoped API or receive a documented exclusion. CI must fail
until one path is chosen — that is the point: the policy decision stays
visible.

### `module_max_lines`

Split by extraction, following the repository's existing convention
(`#[path]` sibling modules are common). Test modules extract cleanly
to `foo_tests.rs`; keep behaviour identical. When splitting files under
`tests/`, place helpers in a subdirectory so cargo does not
auto-discover them as new test crates.

### `module_must_have_inner_docs`

Add a `//!` first line stating what the module is for. If the module
already opens with an outer `///` on its first item, converting may be
needed to avoid `clippy::mixed_attributes_style`.

### `bumpy_road_function` and `conditional_max_n_branches`

Extract intention-revealing helpers for each complexity cluster or
branch bundle. Re-run the suite afterwards: the extraction changes
module sizes and can surface follow-on findings.

## Clippy interactions to budget for

Converting tests to return `Result` interacts with strict clippy
configurations:

- `panic_in_result_fn` denies `assert!`/`assert_eq!` in
  Result-returning tests — use `anyhow::ensure!`/`bail!` (or the
  `eyre` equivalents).
- `shadow_reuse` fires on `let x = x?;` rebinding — take the fixture
  as a differently named parameter (rstest `#[from(fixture)]
  fixture_res: Result<…>`) or destructure to fresh names.
- Assertion macros expand inline: a macro used inside a test can push
  it over `cognitive_complexity`.

Always finish with the repository's full gate suite, not just the
Whitaker run.

## Toolchain and dependency wrinkles

- The suite lints under its own pinned nightly regardless of the
  repository toolchain, so in matrix CI run it on one leg only.
- If a dependency's `rust-version` exceeds the suite's pinned nightly
  rustc, pass `--ignore-rust-version` on the dylint invocation and
  document why; the repository toolchain still enforces the real MSRV.
- Mutually exclusive feature sets cannot be linted with
  `--all-features`; run the suite once per feature leg with that leg's
  flags.

## What not to do

- Do not add `#[allow]`/`#[expect]` for `no_std_fs_operations` — they
  do not work (issue #270) and reviewers will flag them.
- Do not soft-skip the suite ("run whitaker if installed"): a gate
  that cannot fail is not a gate. The Makefile invocation should be
  unconditional, with the binary overridable via a variable
  (`WHITAKER ?= whitaker`).
- Do not silence a fixture by renaming it to look like a test; make it
  fallible instead.
- Do not cache the staged lint libraries keyed by installer version in
  CI while the suite floats on the rolling release — it pins stale
  lints arbitrarily. Cache only the installer binary; revisit once the
  suite is pinned by ref.

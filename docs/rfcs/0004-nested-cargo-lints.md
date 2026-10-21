# RFC 0004: nested `cargo` lints for tests

## 1. Preamble

- **RFC number:** 0004
- **Status:** Proposed
- **Created:** 2026-10-22

## 2. Summary

This RFC proposes three Whitaker late-pass lints for Rust tests that spawn
`cargo` or `rustc` as child processes:

1. `nested_cargo_private_target`: a test spawns `cargo` and gives it a private
   target directory derived from a temporary directory, or leaves it with one
   by default.
2. `test_spawns_rustc`: a test spawns `rustc` directly.
3. `unbounded_child_cargo`: a spawned child `cargo` has no time bound.

All three target one class of defect: compiler work performed inside a test
body against a cache that is cold, private, or unbounded. In CI that costs
runner minutes, produces timeouts that look like flakiness, and risks running
out of disk. The lints diagnose the shape at the source, so it is caught when
the test is written rather than after a cold run exhausts its budget.

This RFC contains no lint code. It records the detection approach, the false
positives the design accepts, the escape hatches, and the four real instances
that motivate it.

## 3. Problem

Nested builds are tests that run `cargo build`, `cargo check`, `cargo test` or
`rustc` against a fixture or against the workspace itself. Trybuild-style and
fixture-based suites do this by design. The defect is not the nesting; it is
the cache and the bound the nested build runs with. Three patterns recur.

- **Lockfile drift.** A fixture `Cargo.lock` resolves separately from the
  workspace lock, so a nested build compiles different units from the outer
  build. This is a repository-level property and is handled by Concordat, not
  by these lints.
- **Private target directory.** A random or temporary `CARGO_TARGET_DIR` means
  nothing is shared between builds, and the compiler cache keys miss because
  the paths differ.
- **Compiler work inside a timed test.** `cargo` or `rustc` is spawned inside a
  timed test body with no bound, so a cold build is indistinguishable from a
  hang and consumes the test's whole timeout.

Four instances drive the design:

- [rstest-bdd#895](https://github.com/leynos/rstest-bdd/pull/895): nested
  fixtures rebuilt crates the workspace had already compiled, spending about 19
  seconds per cold invocation across about ten invocations against only 120
  seconds of slack under a 75-minute global timeout. The `cargo-bdd` CLI test
  also fell back to a private directory under the crate:

  ```rust
  let fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/cargo-bdd-cli");
  ```

- [netsuke#913](https://github.com/leynos/netsuke/pull/913): the
  nested-Cargo test harness timed out at 900 seconds because a fixture resolved
  its own unlocked dependency graph.
- [netsuke#939](https://github.com/leynos/netsuke/pull/939): a timed test ran a
  package-scoped `cargo check`, resolved compiler artefacts from Cargo's JSON
  stream, then spawned `rustc` for a positive and a negative fixture, all in
  one test body. Raising its allowance twice did not help; the test still timed
  out at 900 seconds while its nested requests waited on healthy admission.
- [ortho-config#611](https://github.com/leynos/ortho-config/issues/611): each
  `cargo-orthohelp` scenario builds into a target directory named after a
  random temporary directory:

  ```rust
  let scenario_id = out_dir
      .file_name()
      .ok_or("temporary output directory should have a final path component")?;
  Ok(resolve_target_dir(&workspace_root)
      .join("orthohelp-bdd")
      .join(scenario_id))
  ```

  The issue measured 15 such trees totalling 9 GB, eleven of them performing
  the identical cold build of the identical fixture, and nothing bounded the
  child `cargo` the scenarios spawn.

## 4. Current state

Whitaker already ships lints that reason about test context.
`no_expect_outside_tests` classifies a node as test code from its attributes
and its path, and `whitaker_common::context` exposes `is_test_fn` and
`in_test_like_context` for lints that need the same answer. These lints reuse
that classification instead of defining a second one.

Nothing in the suite examines how a test spawns a process. Concordat will check
the repository-level half of the problem (fixture lockfile alignment, manifest
coverage, nextest configuration and CI wiring). The source-level half is where
a compiler-aware tool is needed, because the relevant facts (what a `Command`
was built from, which value reaches `CARGO_TARGET_DIR`) live in typed
expressions rather than in text.

## 5. Goals and non-goals

- Goals:
  - Diagnose, at the authored call site, a test that spawns `cargo` with a
    private or temporary target directory.
  - Diagnose a test that spawns `rustc` directly.
  - Diagnose a child `cargo` that nothing in the test bounds in time.
  - Keep false positives low enough that an escape hatch is uncommon, and make
    every use of it carry a reason.
- Non-goals:
  - Cross-function or cross-process dataflow. A value built several calls away,
    or in another crate, is not followed.
  - Judging the build cache itself, lockfile content, or CI configuration.
  - Diagnosing production code that legitimately spawns `cargo`, such as a
    `cargo` subcommand that shells out to the build.
  - Reading nextest configuration. A lint cannot see it, so the escape hatch for
    the bound lint is explicit.

## 6. Proposed design

All three lints are `LateLintPass` implementations. They share one recognizer
for "this expression is a child process for `cargo` or `rustc`", one for "this
value derives from a temporary directory", and the existing test-context
helpers. They fire only in test-like context: a function under `#[test]`,
`#[rstest]`, `#[tokio::test]`, a BDD step or scenario attribute, or a function
in a test-harness compilation of an integration-test crate. Production items in
the same crate are not examined.

### Shared recognizers

*Spawned program.* A call resolving to `std::process::Command::new` (and
`tokio::process::Command::new`) whose first argument is one of:

- the string literal `"cargo"` or `"rustc"`;
- the expansion of `env!("CARGO")` or `env!("RUSTC")`;
- a call to `std::env::var("CARGO")` or `var_os("CARGO")`, directly or through
  a `let` binding in the same function body;
- a local that is bound in the same function to one of the forms above, to a
  `PathBuf` joined from them, or to an argument of the enclosing function whose
  name is `cargo` or `rustc` (a conservative naming fallback, see the risks).

*Builder chain.* Method calls on the resulting `Command`, whether chained or
issued on a `let mut` local in the same body. The lint collects `.env`, `.envs`,
`.arg`, `.args`, `.current_dir`, `.env_clear`, `.env_remove`, `.output`,
`.status` and `.spawn` calls whose receiver resolves to the same binding.

*Temporary-derived value.* An expression is temporary-derived when its type or
its initializer is, within depth 2 of `let` bindings in the same body:

- `tempfile::tempdir()`, `TempDir::new`, `TempDir::path`, `Builder::tempdir`;
- `std::env::temp_dir()`;
- a `join` or `to_path_buf` of a temporary-derived value.

### Lint 1: `nested_cargo_private_target`

A test spawns `cargo` and one of the following holds on its builder chain:

1. `.env("CARGO_TARGET_DIR", v)` or `.envs([...])` carrying that key, or
   `.arg("--target-dir")` followed by a value `v`, where `v` is
   temporary-derived;
2. the chain calls `.current_dir(d)` with a temporary-derived `d` and neither
   sets a target directory nor calls `.env_remove("CARGO_TARGET_DIR")` away
   from an inherited shared one; Cargo then builds into `d/target`, which is
   private by construction;
3. the chain calls `.env_clear()` and does not set `CARGO_TARGET_DIR` again,
   because the child can no longer inherit the shared directory.

A bare child `cargo` in the workspace directory with no `CARGO_TARGET_DIR`
override inherits the parent's variable and is deliberately not flagged. A
directory derived from `CARGO_MANIFEST_DIR` and joined with `"target/..."` is
also a private tree (rstest-bdd#895's `target/cargo-bdd-cli`); the lint treats a
`join` whose argument starts with `target` on a `CARGO_MANIFEST_DIR` expansion
as private, and the diagnostic suggests reading `CARGO_TARGET_DIR` from the
parent environment instead.

*Feasible dataflow.* Local `let` chains of depth 2 and method chains within one
function body, using typed HIR. This catches the packaging-style
`command.env("CARGO_TARGET_DIR", target_dir.path())` when `target_dir` is a
`TempDir` parameter or local.

*Out of scope.* ortho-config#611 builds the value in `scenario_target_dir`, a
function in another module whose result depends on a field of a context value.
Following that needs inter-procedural summaries and is not attempted. The lint
would still diagnose the call site in `steps_cmd.rs` only if the local it
passes is initialized in the same function from a temporary-derived call. An
open question below asks whether a crate-local `#[whitaker::target_dir_source]`
marker is worth adding.

*False-positive risks.*

- A deliberately cold build, such as a test whose purpose is to verify a fresh
  extracted crate (netsuke's packaging smoke test keeps one under `target/`).
  This is the intended use of the escape hatch.
- A test of a tool that itself manages `CARGO_TARGET_DIR`, where the test must
  set the variable to inspect the tool's behaviour without building anything.
- A `tempdir` that is created inside the workspace target directory and shared
  across the whole test binary (a `OnceLock`). The lint does not follow
  `OnceLock` initializers and so would flag the use; the diagnostic points to
  the shared-directory idiom.

### Lint 2: `test_spawns_rustc`

A test, or a helper reached from a test through same-crate calls of depth at
most 3, calls `Command::new` with a `rustc` program (the literal,
`env!("RUSTC")` or `var("RUSTC")`) and runs it. This is the netsuke#939 shape:
compiler work performed in the test body is charged to that test's timeout, and
it bypasses Cargo's package-cache lock and the build queue's admission.

*Feasible dataflow.* The same recognizer as above plus a bounded call graph
within the crate. Cross-crate helpers (a `test_support` crate) are followed
only when their MIR is available through `tcx.optimized_mir`; otherwise the
call is not followed.

*Out of scope.* A `rustc` spawned by a proc-macro or by a library the test
calls (trybuild, `ui_test`). Those are covered by the nextest checks in
Concordat.

*False-positive risks.*

- Compiler-behaviour suites (UI tests for a lint or macro crate) legitimately
  run `rustc`. They are expected to carry an `expect` with a reason, and the
  diagnostic says so.
- `rustc --version` or `rustc --print sysroot` queries do no compilation. The
  lint ignores a chain whose arguments are all `--version`, `-V`, `--print` or
  `--help` forms.

### Lint 3: `unbounded_child_cargo`

A child `cargo` is run (`.output()`, `.status()` or `.spawn()` followed by a
blocking `.wait()` or `.wait_with_output()`) with a compiling subcommand
(`build`, `check`, `test`, `run`, `package`, `clippy`, `doc`, `install`,
`publish`, `bench`, `rustc`), and no bound is visible. A bound is visible when
the same function:

- uses a timeout wrapper from a configurable list (default `wait_timeout`,
  `wait-timeout`, `recv_timeout`, `tokio::time::timeout`, `Child::kill` on a
  deadline), or
- passes the child to a helper whose name or path is on the configured bounded
  list.

*Feasible dataflow.* Presence of a call to a configured wrapper on the same
`Child`, or on a channel whose sender is moved into a thread that runs the
child, within one function body.

*Out of scope.* Bounds applied outside the test: nextest's `slow-timeout` and
`global-timeout`, or a CI step timeout. The lint cannot see them, so a test
that relies on them takes an `expect` that names the bound.

*False-positive risks.*

- Many repositories bound every test through nextest; the lint then reports
  every nested build. The `expect` reason documents the outer bound, which is a
  reviewable statement rather than noise.
- `cargo metadata`, `cargo config` and `cargo locate-project` are fast and do
  no compilation. They are excluded by subcommand.

### Configuration

Each lint reads a `dylint.toml` table of its own name. Keys follow Whitaker's
typed-configuration convention; the initial set is:

```toml
[unbounded_child_cargo]
bounded_wrappers = ["wait_timeout", "recv_timeout"]
compiling_subcommands = ["build", "check", "test", "run", "package"]

[test_spawns_rustc]
allowed_arguments = ["--version", "--print"]
```

### Escape hatch

The suite's standard mechanism applies: `#[allow(...)]` or, preferably,
`#[expect(<lint>, reason = "...")]` on the smallest item. The reason is
required; a lint run in `deny` mode treats an `allow` without one as a finding.
Typical reasons: "packaging verification must build a fresh crate", "compiler
behaviour suite", "bounded by nextest slow-timeout 20m in the cargo-spawning
group".

## 7. Requirements

### Functional requirements

- The lints MUST be named `nested_cargo_private_target`, `test_spawns_rustc` and
  `unbounded_child_cargo`, and be part of the suite distribution.
- They MUST diagnose only items in test-like context.
- `nested_cargo_private_target` MUST NOT flag a child `cargo` that inherits the
  parent's `CARGO_TARGET_DIR`.
- `test_spawns_rustc` MUST ignore invocations whose arguments only query the
  compiler.
- `unbounded_child_cargo` MUST ignore non-compiling subcommands.
- Each diagnostic MUST name the escape hatch and require a reason.

### Technical requirements

- All three MUST be late passes over typed HIR; none may scan source text.
- The recognizers MUST be shared in `whitaker_common`, with their scope and
  re-use policy recorded in the developers' guide as the repository's helper
  policy requires.
- Dataflow depth and call-graph depth MUST be constants with unit tests that pin
  them.
- Diagnostics, help text and UI snapshots MUST use the localization
  infrastructure.

## 8. Compatibility and migration

All three are opt-in through the suite's usual lint selection, so adoption is
per repository. Expected first findings on the current estate are the instances
above and the packaging-style deliberate cold builds. Adoption order: run the
lints at `warn`, resolve genuine findings by sharing the target directory or
moving the compiler work into the ordinary build graph (the netsuke#939 fix),
annotate deliberate cases with `expect` and a reason, and then raise to `deny`.

Concordat covers the repository-level counterparts: fixture lockfile alignment,
fixture manifest coverage, nextest group and timeout configuration, and CI
wiring. The two sets of checks do not overlap: Concordat reads files, these
lints read typed code.

## 9. Test plan

UI fixtures cover, for each lint, a failing case drawn from the instances and a
passing case drawn from their fixes:

- Lint 1: `.env("CARGO_TARGET_DIR", tmp.path())` fails; the same with a value
  read from `std::env::var("CARGO_TARGET_DIR")` passes; `current_dir(tmp)`
  without a target fails; `join("target/cargo-bdd-cli")` on
  `CARGO_MANIFEST_DIR` fails; the `OnceLock` shared directory idiom is
  documented as a known false positive; `#[expect]` with a reason suppresses.
- Lint 2: a `rustc` spawn in a `#[test]` fails; in a helper called from a test
  fails; `rustc --version` passes; a spawn in non-test code passes.
- Lint 3: `cargo build` with `.output()` and no wrapper fails; the same with
  `wait_timeout` passes; `cargo metadata` passes.

A `proptest` suite covers the pure classifiers: the temporary-derived predicate
over generated `let` chains up to and beyond the depth limit, the subcommand
classifier, and the query-argument filter. The implementation PR for each lint
runs the repository's formatting, lint, test, Markdown and Mermaid gates.

## 10. Alternatives considered

| Option                          | Advantages                                           | Rejected because                                                                  |
| ------------------------------- | ---------------------------------------------------- | --------------------------------------------------------------------------------- |
| Late-pass Whitaker lints        | Typed expressions, compiler spans, suite conventions | Selected                                                                          |
| Source-text scanner (Concordat) | No compiler dependency, runs on any checkout         | Cannot tell a temporary value from a shared one, so false positives dominate      |
| Runtime guard in a test helper  | Catches the real build                               | Only protects tests that adopt the helper; the defect is in tests that do not     |
| Rely on nextest limits alone    | Already configured in several repositories           | Bounds the symptom, not the cold build, and cannot see a private target directory |

*Table 1: Alternatives for diagnosing nested cargo builds in tests.*

## 11. Open questions

- Should a crate-local attribute mark a function as a source of private target
  directories, so inter-procedural cases such as ortho-config#611 are caught?
- Where should the shared recognizers live, and should Lint 2 follow
  `test_support` crates through MIR or stop at the crate boundary?
- Should Lint 3 recognize nextest configuration by reading
  `.config/nextest.toml` at lint time, accepting the coupling, instead of
  requiring an `expect`?
- Is the naming fallback for a `cargo` or `rustc` function argument acceptable,
  or does it widen the lints too far?

## 12. Recommendation

Accept the three lints as late passes with test-context gating, bounded
dataflow and an `expect`-with-reason escape hatch. They cover the part of the
nested-build problem that only typed code can see, leave the repository-level
half to Concordat, and turn the four known instances into permanent regression
fixtures.

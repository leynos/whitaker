# Issue #572: `string_continuation_style` test plan and rollout record

- **Status:** Complete (2026-11-26)
- **Issue:** [#572](https://github.com/leynos/whitaker/issues/572)
- **Design:** [RFC 0002](../rfcs/rfc-0002-string-continuation-style.md)
- **Lint lifecycle:** Experimental; promotion criteria in RFC 0002 §Rollout

## Purpose

Issue #572 tracked the RFC 0002 test-plan and rollout items that were still
open when `string_continuation_style` became experimental on 2026-11-08. This
record states what was already present at `9fc35c0`, what this change adds, the
dogfood and estate-run evidence, and the resulting promotion assessment. The
lint remains experimental: this record does not promote it.

## Test-plan completion

### Acceptance fixture for an explicit capture

`crates/string_continuation_style/ui/fail_rfc_header_explicit_argument.rs`
reproduces RFC 0002's second acceptance fixture: the PR #296 header with
`status_line = status_line` added. The fixture carries `// run-rustfix` and a
`.fixed` file whose replacement wraps both fragments in `concat!(...)` and
keeps both arguments. `pass_implicit_format_capture.rs` keeps the same header
without the explicit argument and uses the new fixture as its silence control.

### Continuation-scanner unit tests

`crates/whitaker_string_literals/src/continuation/tests.rs` now covers:

- odd and even backslash runs before LF, and the same rule after normalizing
  CRLF source to LF;
- the raw-CRLF boundary, which is not a continuation because `rustc_lexer`
  triggers only on a backslash immediately followed by LF;
- leading, interior, and trailing continuations in one body;
- Unicode text immediately before and after a join, including byte offsets;
- escaped quotes, escaped braces, and ordinary escapes, which the rewrite
  copies verbatim in `crates/whitaker_string_literals/src/rewrite/tests.rs`.

### Property tests

`crates/whitaker_string_literals/src/rewrite/props.rs` already generated one to
five joins and asserted that the copied fragments equal the source with every
selected continuation range deleted. This change adds
`crates/whitaker_string_literals/src/classification/props.rs`, which asserts
that any positive join count yields exactly one `PreferConcat` disposition and
that its join and fragment counts match. That is the policy-level result, not
an emission test: it never runs the driver.

`crates/string_continuation_style/tests/emission_count.rs` closes that gap. It
generates a fixture with one literal for each join count from one to ten,
drawing safe fragments from a fixed alphabet, runs the real lint through
`cargo dylint`, and records each diagnostic's primary source line. It asserts
exactly one diagnostic for every generated literal line. A per-literal unit
test feeds the same check a double on one line and a suppression on another
with the same aggregate total and requires rejection. A proptest also generates
one to three literals with one to eight joins from the same safe alphabet and
applies the same per-literal assertion; the deterministic one-to-ten matrix
remains boundary coverage. The `fail_multiple_joins` and `fail_many_joins` UI
fixtures retain the checked-in two-join and ten-join expectations.

### Behaviour scenarios

`crates/whitaker_string_literals/tests/features/string_continuation.feature`
and its `rstest-bdd` binding already contained the three RFC 0002 scenarios: an
ordinary join prefers `concat!()`, an implicit capture requires the
continuation, and a leading continuation requires it for layout. The full test
gate runs all three.

## Rollout evidence

### Dogfood on Whitaker

The current `string_continuation_style` library was loaded with
`cargo dylint --lib string_continuation_style` over the support-crate gate
scope and then over the whole workspace:

```sh
cargo dylint --keep-going --lib string_continuation_style -- \
  --workspace --all-targets --all-features
```

The support-crate pass found one join in
`installer/tests/behaviour_cli/support_assertions.rs`. The full-workspace pass
found four more:

- `crates/bumpy_road_function/src/driver/segment_builder/mod.rs`
- `crates/string_continuation_style/src/diagnostics.rs`
- `crates/macro_aware_cognitive_complexity/tests/support/cargo.rs`
- `suite/tests/support/macro_aware_smoke.rs`

All five were rewritten to `concat!()` and the full-workspace rerun reported
zero findings. The PR #296 fixture, `pass_implicit_format_capture.rs`, remains
clean; the UI suite exercises it on every run. The suite-level
`[whitaker] experimental = ["string_continuation_style"]` opt-in was also
exercised with `make lint-whitaker RUST_FLAGS=""` over the same support-crate
scope; it reported zero joins.

The installer's `--experimental` path was exercised from the candidate head:

```sh
whitaker-installer --experimental \
  --suite-version e893bd96df10c639d662d01a49b5d96f16a8c278 \
  --target-dir ~/.cache/whitaker-estate-scan/experimental-suite \
  --skip-wrapper --skip-deps
```

The built suite was loaded with `cargo dylint --all`. A positive-control crate
containing one explicit join and the PR #296 implicit-capture header produced
exactly one `string_continuation_style` diagnostic, for the explicit join, and
none for the header. The suite run over the support-crate gate scope reported
zero `string_continuation_style` findings. The full-workspace command was:

```sh
DYLINT_LIBRARY_PATH=~/.cache/whitaker-estate-scan/experimental-suite/nightly-2026-05-28/release
DYLINT_CLIPPY_REPO="$(make -s dylint-env | sed -n 's/^DYLINT_CLIPPY_REPO=//p')"
DYLINT_DRIVER_PATH="$(make -s dylint-env | sed -n 's/^DYLINT_DRIVER_PATH=//p')"
DYLINT_RUSTFLAGS='--cap-lints warn'
RUSTUP_TOOLCHAIN=nightly-2026-05-28
export DYLINT_LIBRARY_PATH DYLINT_CLIPPY_REPO DYLINT_DRIVER_PATH
export DYLINT_RUSTFLAGS RUSTUP_TOOLCHAIN
cargo dylint --all --keep-going -- \
  --workspace --all-targets --all-features
```

`--cap-lints warn` caps unrelated deny-level standard lints; it does not
suppress `string_continuation_style` diagnostics, which remain warnings. The
run reported zero `string_continuation_style` diagnostics and no compilation
errors. The build used the code head; later record-only commits do not change
the lint source.

### Estate run

The lint was run against every `leynos` Rust repository whose current `main`
had at least one line ending in a backslash: 34 repositories and 2,536 textual
candidate lines. Each repository was extracted from its bare clone at the
recorded commit and checked with the individually built library:

```sh
cargo dylint --keep-going --lib string_continuation_style -- --workspace
```

The run uses default Cargo targets (libraries and binaries), so test-only joins
are outside its scope; the Whitaker dogfood run above did include
`--all-targets`. Of the 34 repositories, 33 were scanned: 30 at the repository
root, two through a subdirectory workspace, and `zamburak` through a worktree
with its required submodule checked out. `rust-skill` has no `Cargo.toml` on
`main` and is not a Rust workspace.

The production-target run produced 17 findings in 7 repositories; 26 scanned
repositories were clean. The largest textual candidates, `axinite` (494
candidate lines), `wireframe` (186), and `netsuke` (71), produced no findings,
which is the context-sensitive filter working as designed rather than a failure
to scan. These 17 are an estate-wide **production-target** total, not an
estate-wide all-targets total.

| Repository                 | Commit         | Findings | Notes                            |
| -------------------------- | -------------- | -------- | -------------------------------- |
| `whitaker`                 | `5bf23eab03fa` | 4        |                                  |
| `rstest-bdd`               | `0bf12ee79a95` | 3        |                                  |
| `weaver`                   | `5fa96aee1666` | 3        |                                  |
| `wildside-engine`          | `21f58b16c2d0` | 3        |                                  |
| `thysalion`                | `cbcbbc58a5e1` | 2        |                                  |
| `theoremc`                 | `8f48abbecbd0` | 1        |                                  |
| `vtcode`                   | `fd1b77a973da` | 1        |                                  |
| `actix-v2a`                | `cabd7c858ea1` | 0        |                                  |
| `axinite`                  | `8045e7543f3a` | 0        |                                  |
| `chutoro`                  | `670b8ff518ff` | 0        |                                  |
| `cuprum`                   | `9bdfdc448c71` | 0        | scanned `rust/`                  |
| `ddlint`                   | `0caf102ada0c` | 0        |                                  |
| `femtologging`             | `c3bd1637885d` | 0        | scanned `rust_extension/`        |
| `frankie`                  | `15603977767e` | 0        |                                  |
| `gauss`                    | `ee0b3920a1c5` | 0        |                                  |
| `lille`                    | `83fa8ca3186e` | 0        |                                  |
| `mdtablefix`               | `f900447c7a73` | 0        |                                  |
| `memoryd`                  | `8639da6fefaf` | 0        |                                  |
| `mxd`                      | `68fc1d240c75` | 0        |                                  |
| `netsuke`                  | `1e60fb18104e` | 0        |                                  |
| `ortho-config`             | `e3305cc25780` | 0        |                                  |
| `pg-embed-setup-unpriv`    | `2ec8244bd382` | 0        |                                  |
| `pg-embedded-setup-unpriv` | `0e289523c1d6` | 0        |                                  |
| `podbot`                   | `30a81df36504` | 0        |                                  |
| `repovec-appliance`        | `784e4dde248c` | 0        |                                  |
| `splitters`                | `91ea1b4f2548` | 0        |                                  |
| `spycatcher-harness`       | `54ec0c276ada` | 0        |                                  |
| `tei-rapporteur`           | `e2dd5276eecb` | 0        |                                  |
| `trasic`                   | `5c3c1951149d` | 0        |                                  |
| `vk`                       | `26495c34c438` | 0        |                                  |
| `wildside`                 | `9f4baf0b192c` | 0        |                                  |
| `wireframe`                | `048b5a76255b` | 0        |                                  |
| `rust-skill`               | `3dfcb239aa48` | n/a      | no `Cargo.toml` on `main`        |
| `zamburak`                 | `daf18f2b9da0` | 0        | scanned via a submodule worktree |

The 17 findings are:

| Repository        | Finding locations                                                                                                                                                               |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rstest-bdd`      | `crates/rstest-bdd-harness/src/nested_cargo.rs:124`, `:360`; `crates/rstest-bdd-harness/src/trybuild_staging/mod.rs:145`                                                        |
| `theoremc`        | `crates/theoremc-core/src/schema/value.rs:52`                                                                                                                                   |
| `thysalion`       | `crates/world/src/scene/palette.rs:243`; `crates/world/src/scene/validation/rules/voxels.rs:159`                                                                                |
| `vtcode`          | `crates/codegen/vtcode-config/build.rs:56`                                                                                                                                      |
| `weaver`          | `crates/sempai-yaml/src/parser/builders.rs:134`; `crates/weaverd/src/dispatch/act/refactor/arguments.rs:134`; `crates/weaver-docs-gate/src/lib.rs:21`                           |
| `whitaker`        | `common/src/brain_trait_metrics/diagnostic.rs:172`; `common/src/brain_type_metrics/diagnostic.rs:301`; `crates/whitaker_clones_core/src/index/types.rs:155`; `src/config.rs:59` |
| `wildside-engine` | `wildside-data/src/wikidata/dump/log.rs:62`, `:72`; `wildside-data/src/wikidata/etl/mod.rs:208`                                                                                 |

### All-targets follow-up

The production-target scan is the estate-wide measure. To measure the test and
benchmark targets that it omits, an all-targets pass covered the seven
repositories with production findings:

| Repository        | Production findings | All-target findings | Additional test/benchmark findings |
| ----------------- | ------------------- | ------------------- | ---------------------------------- |
| `rstest-bdd`      | 3                   | 23                  | 20                                 |
| `thysalion`       | 2                   | 6                   | 4                                  |
| `vtcode`          | 1                   | 3                   | 2                                  |
| `whitaker`        | 4                   | 5                   | 1                                  |
| `weaver`          | 3                   | 3                   | 0                                  |
| `wildside-engine` | 3                   | 3                   | 0                                  |
| `theoremc`        | 1                   | 1                   | 0                                  |
| **Total**         | **17**              | **44**              | **27**                             |

`whitaker`'s production row was measured at `5bf23eab03fa`; its all-target row
at the then-current `52f6a52da`. The other rows use the same commit for both
passes. The all-targets pass used default features, except for `whitaker` and
`wildside-engine`, which need `--all-features` for feature-gated test-support
modules.

The additional test and benchmark locations are:

- `vtcode`: `crates/codegen/vtcode-ui/benches/markdown_render.rs:17`;
  `tests/check_script_ast_grep.rs:15`.
- `whitaker`: `installer/tests/behaviour_cli/support_assertions.rs:195`.
- `thysalion`: `crates/world/tests/golden_bytes.rs:52`, `:63`;
  `tests/workflow_shape.rs:32`; `tests/demo_guard.rs:52`.
- `rstest-bdd`:
  `crates/rstest-bdd-server/tests/implementation_navigation.rs:178`, `:202`,
  `:207`; `examples/todo-cli/tests/todo.rs:90`;
  `crates/rstest-bdd/tests/runner_cancel/cases.rs:91`, `:98`, `:115`, `:158`;
  `crates/rstest-bdd/tests/runner_instrumentation/nested_span.rs:73`;
  `crates/rstest-bdd/tests/runner_instrumentation.rs:191`;
  `crates/rstest-bdd/tests/parser_neutral_runner.rs:94`, `:222`, `:238`, `:265`,
  `:275`, `:294`, `:304`; `crates/rstest-bdd/tests/nextest_config.rs:86`;
  `crates/rstest-bdd/tests/runner_wire.rs:99`, `:160`.

This follow-up covers only the seven repositories with production findings. The
remaining 26 production-clean repositories' test and benchmark targets were not
scanned; that is a deliberate scope boundary for the rollout decision, which
prioritizes production style. The 17 production findings remain the estate-wide
production total; the 44 all-targets findings are the measured all-targets
sample total, not an estate-wide all-targets total.

### Review guidance

`AGENTS.md` now states the context-sensitive rule instead of the unconditional
`concat!()` rule:

> Use `concat!()` for interior joins in cooked string expressions unless
> direct-literal status is required, including implicit format capture or a
> literal-only macro contract.

CodeRabbit reads `AGENTS.md` as its code-guideline file, so its review comments
and the lint state the same policy.

## Promotion assessment

| Criterion                                                                           | Status        | Evidence                                                                                                                        |
| ----------------------------------------------------------------------------------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Test-plan items 1-5 complete                                                        | Done          | This record                                                                                                                     |
| Issue #564 review findings resolved                                                 | Open          | #564 remains open; the non-UTF-8 `CARGO_TARGET_DIR` path still needs its fail-closed fix                                        |
| Rolling-release pre-publish install check passes with the lint in the default suite | Not attempted | The lint is still experimental                                                                                                  |
| Estate run manageable, or machine fixes ready                                       | Done          | 17 production-target findings across 33 scanned repositories; the all-targets sample of the seven finding repositories found 44 |

The lint therefore remains in `EXPERIMENTAL_LINTS`. Promotion is a follow-up
release decision (RFC 0002 §Rollout step 5).

## Review reconciliation (2026-11-27)

CodeRabbit's original-issue assessment identified three gaps against the first
head. This change:

- Adds the generated `emission_count` integration test, which drives a
  deterministic one-to-ten join matrix and a proptest of varied safe fragments
  through the real lint, associates diagnostics with each literal's primary
  line, and rejects a double plus a suppression that preserves the aggregate
  total. It also retains the `fail_many_joins` UI fixture.
- Extends the dogfood to the whole workspace, runs the installer's
  `--experimental` path with a positive control, and records the all-targets
  estate follow-up, including `zamburak` through a submodule worktree.
- Corrects the implicit-capture control to pass `"200 OK"` rather than a second
  `HTTP/1.1` prefix, and adds a diagnostic to its assertion.

The original assessment's third point also noted that the UI harness compiles
the fixtures without running their `main` assertions. The correction makes the
control correct if it is run manually; the compile-time value assertions in the
`.fixed` fixtures remain the compiler-adjudicated checks.

## Reproduction

The focused commands are:

```sh
cargo nextest run -p whitaker_string_literals --all-features
cargo nextest run -p string_continuation_style --all-features --lib \
  -E 'test(tests::ui::ui)'
```

The full commit gates are `make check-fmt`, `make lint`, and `make test`, with
the CI-only installer behaviour binaries run under `NEXTEST_PROFILE=ci`.

The estate pass extracted each repository's `main` with `git archive` and ran
the command shown in the estate section with `DYLINT_LIBRARY_PATH` pointing at
the individually built library in `~/.cache/whitaker-estate-scan/lib`. The
per-repository commits and findings are in the tables above.

## Decision log

- **Keep the raw-CRLF scanner boundary explicit.** rustc normalizes CRLF before
  lexing, so the scanner's input contract is LF-only. The tests normalize CRLF
  source before applying the odd-run rule and separately pin the raw boundary,
  rather than teaching the scanner to guess whether normalization happened.
- **Scan production targets estate-wide and all targets for the finding
  repositories.** The estate-wide pass uses default Cargo targets so it can
  cover every repository. A follow-up all-targets pass covers the seven
  repositories with production findings; the remaining 26 test-target scopes
  are a recorded boundary, not an unmeasured claim.
- **Scan `zamburak` through a submodule worktree.** `git archive` omits
  `third_party/full-monty`; a detached worktree with
  `submodule update --init --recursive` scanned it and found no joins.
- **Do not promote the lint in this change.** Issue #564 is still open, and the
  rolling release's pre-publish check has not run with the lint in the default
  suite. RFC 0002 §Rollout now records the promotion criteria instead of the
  original unconditional promotion step.

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
that its join and fragment counts match. Because the driver emits one
diagnostic per disposition, that is the "one diagnostic per literal" property
at the policy boundary; the `fail_multiple_joins` UI fixture pins the
end-to-end warning count.

### Behaviour scenarios

`crates/whitaker_string_literals/tests/features/string_continuation.feature`
and its `rstest-bdd` binding already contained the three RFC 0002 scenarios: an
ordinary join prefers `concat!()`, an implicit capture requires the
continuation, and a leading continuation requires it for layout. The full test
gate runs all three.

## Rollout evidence

### Dogfood on Whitaker

The individually built `string_continuation_style` library was loaded with
`cargo dylint --lib string_continuation_style` over Whitaker's support-crate
gate scope:

```sh
cargo dylint --keep-going --lib string_continuation_style -- \
  -p whitaker-common -p whitaker-installer -p whitaker_clones_core \
  -p whitaker_sarif --all-targets --all-features
```

The first run found one join, in
`installer/tests/behaviour_cli/support_assertions.rs`. It was rewritten to
`concat!()` and the rerun reported zero findings. The PR #296 fixture,
`pass_implicit_format_capture.rs`, remains clean; the UI suite exercises it on
every run.

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
`--all-targets`. Of the 34 repositories, 32 were scanned (two through a
subdirectory workspace), one could not be checked because its `git archive`
omitted a required submodule, and one has no `Cargo.toml` on `main`.

The lint produced 17 findings in 7 repositories; 25 scanned repositories were
clean. The largest textual candidates, `axinite` (494 candidate lines),
`wireframe` (186), and `netsuke` (71), produced no findings, which is the
context-sensitive filter working as designed rather than a failure to scan.

| Repository                 | Commit         | Findings | Notes                                |
| -------------------------- | -------------- | -------- | ------------------------------------ |
| `whitaker`                 | `5bf23eab03fa` | 4        |                                      |
| `rstest-bdd`               | `0bf12ee79a95` | 3        |                                      |
| `weaver`                   | `5fa96aee1666` | 3        |                                      |
| `wildside-engine`          | `21f58b16c2d0` | 3        |                                      |
| `thysalion`                | `cbcbbc58a5e1` | 2        |                                      |
| `theoremc`                 | `8f48abbecbd0` | 1        |                                      |
| `vtcode`                   | `fd1b77a973da` | 1        |                                      |
| `actix-v2a`                | `cabd7c858ea1` | 0        |                                      |
| `axinite`                  | `8045e7543f3a` | 0        |                                      |
| `chutoro`                  | `670b8ff518ff` | 0        |                                      |
| `cuprum`                   | `9bdfdc448c71` | 0        | scanned `rust/`                      |
| `ddlint`                   | `0caf102ada0c` | 0        |                                      |
| `femtologging`             | `c3bd1637885d` | 0        | scanned `rust_extension/`            |
| `frankie`                  | `15603977767e` | 0        |                                      |
| `gauss`                    | `ee0b3920a1c5` | 0        |                                      |
| `lille`                    | `83fa8ca3186e` | 0        |                                      |
| `mdtablefix`               | `f900447c7a73` | 0        |                                      |
| `memoryd`                  | `8639da6fefaf` | 0        |                                      |
| `mxd`                      | `68fc1d240c75` | 0        |                                      |
| `netsuke`                  | `1e60fb18104e` | 0        |                                      |
| `ortho-config`             | `e3305cc25780` | 0        |                                      |
| `pg-embed-setup-unpriv`    | `2ec8244bd382` | 0        |                                      |
| `pg-embedded-setup-unpriv` | `0e289523c1d6` | 0        |                                      |
| `podbot`                   | `30a81df36504` | 0        |                                      |
| `repovec-appliance`        | `784e4dde248c` | 0        |                                      |
| `splitters`                | `91ea1b4f2548` | 0        |                                      |
| `spycatcher-harness`       | `54ec0c276ada` | 0        |                                      |
| `tei-rapporteur`           | `e2dd5276eecb` | 0        |                                      |
| `trasic`                   | `5c3c1951149d` | 0        |                                      |
| `vk`                       | `26495c34c438` | 0        |                                      |
| `wildside`                 | `9f4baf0b192c` | 0        |                                      |
| `wireframe`                | `048b5a76255b` | 0        |                                      |
| `rust-skill`               | `3dfcb239aa48` | n/a      | no `Cargo.toml` on `main`            |
| `zamburak`                 | `daf18f2b9da0` | n/a      | archive omitted a required submodule |

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

### Review guidance

`AGENTS.md` now states the context-sensitive rule instead of the unconditional
`concat!()` rule:

> Use `concat!()` for interior joins in cooked string expressions unless
> direct-literal status is required, including implicit format capture or a
> literal-only macro contract.

CodeRabbit reads `AGENTS.md` as its code-guideline file, so its review comments
and the lint state the same policy.

## Promotion assessment

| Criterion                                                                           | Status        | Evidence                                                                                 |
| ----------------------------------------------------------------------------------- | ------------- | ---------------------------------------------------------------------------------------- |
| Test-plan items 1-5 complete                                                        | Done          | This record                                                                              |
| Issue #564 review findings resolved                                                 | Open          | #564 remains open; the non-UTF-8 `CARGO_TARGET_DIR` path still needs its fail-closed fix |
| Rolling-release pre-publish install check passes with the lint in the default suite | Not attempted | The lint is still experimental                                                           |
| Estate run manageable, or machine fixes ready                                       | Done          | 17 findings in 7 of 32 scanned repositories; 25 repositories were clean                  |

The lint therefore remains in `EXPERIMENTAL_LINTS`. Promotion is a follow-up
release decision (RFC 0002 §Rollout step 5).

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
- **Scan default targets in the estate run.** Scanning every test target in
  every repository would multiply the build cost without changing the rollout
  decision, so the estate pass covers libraries and binaries. The Whitaker
  dogfood run covers all targets, and the UI suite covers the fixture level.
- **Record the archive limitation rather than cloning submodules.** `zamburak`
  needs `third_party/full-monty`, which `git archive` does not carry. A future
  estate pass should use a worktree with submodules for that repository.
- **Do not promote the lint in this change.** Issue #564 is still open, and the
  rolling release's pre-publish check has not run with the lint in the default
  suite. RFC 0002 §Rollout now records the promotion criteria instead of the
  original unconditional promotion step.

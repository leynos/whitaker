# Implement `string_continuation_style` as a post-expansion early lint

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT — awaiting approval of the plan as a whole. The three decisions
that were open at first review have been answered and are now settled:
`DEC-009` (scope) is approved, `DEC-013` (manual applicability for the first
release) is approved, and `DEC-014` resolves to a single lint name.

## Purpose / big picture

Whitaker's house style says a long string literal should be spelled with
`concat!()` rather than an escaped newline continuation. Reviewers keep
applying that rule by hand, and they keep getting it wrong: some
continuations cannot be replaced without changing what the program does.

After this change, a Rust author who writes

```rust
let text = "alpha \
            beta";
```

and runs `cargo whitaker` (or `cargo dylint --lib string_continuation_style`)
sees a warning offering a fix that rewrites the expression to
`concat!("alpha ", "beta")`, byte-for-byte equivalent at runtime. The same
author who writes

```rust
let header = format!(
    "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n\
     Content-Type: application/octet-stream\r\n\r\n",
    body.len(),
);
```

sees **nothing**, because `format!(concat!(...))` cannot implicitly capture
`status_line` and the rewrite would not compile. The lint is deliberately
asymmetric: it fires only when the replacement is proven safe.

Observable success:

1. `cargo nextest run -p string_continuation_style` passes, including a UI
   matrix that pins both the firing and the non-firing cases, and in which
   every machine-applicable suggestion is actually applied by `rustfix`,
   recompiled, and asserted to preserve the evaluated string.
2. The suite library reports the new lint:
   `cargo dylint list` includes `string_continuation_style`.
3. `docs/users-guide.md` documents the rule, its exemptions, its
   configuration, and how to ask it why it did or did not fire.
4. Running the lint over Whitaker itself produces no findings on the
   pull-request-296 fixture reproduced in the crate's `ui/` directory.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- **Evaluated-value preservation.** A suggested rewrite must produce a string
  whose evaluated bytes are identical to the original literal's. There is no
  acceptable margin here.
- **Never emit a suggestion that has not been checked at run time.** Before
  attaching any applicability above `MaybeIncorrect`, the lint must itself
  compare the original literal's evaluated bytes against the replacement's.
  On mismatch it emits nothing. See `LEM-EMIT-1`.
- **Near-zero false positives.** When the lint cannot prove safety it must be
  silent. Prefer `Ignore`/`RequireContinuation` over a speculative diagnostic.
- **The adapter never panics.** A panicking `EarlyLintPass` aborts the user's
  build with an internal-compiler-error-shaped message that looks like a
  compiler bug. Every fallible recovery maps to a `RequireContinuation` or
  `Ignore` disposition. No `#[expect]` of a panic-class lint
  (`unwrap_used`, `expect_used`, `indexing_slicing`, `string_slice`,
  `panic_in_result_fn`, `unreachable`) is permitted anywhere in either new
  crate. All offset arithmetic uses `checked_*`; `None` means
  `UnrecoverableSourceSpan`.
- **No macro-argument surgery.** The lint must not add explicit format
  arguments (`status_line = status_line`) to unlock a rewrite. See RFC 0002
  §Non-goals.
- **Module size.** No source file added by this work may exceed 400 lines
  (`AGENTS.md` §Code style).
- **British English.** Comments and documentation use en-GB-oxendict spelling,
  except where quoting an external API.
- **Domain purity.** The pure scanning, classification, and
  rewrite-construction logic lives in a crate that has no `rustc_*`
  dependency at all. See `DEC-010`.
- **No changes to existing lints' behaviour.** This plan adds a lint; it must
  not alter the diagnostics of the nine existing standard lints. The one
  exception is `bumpy_road_function`, which migrates onto the shared label
  helper of `DEC-012` with identical output, pinned by its existing fixtures.
- **Suite structure.** Do not force an early pass into
  `rustc_lint::late_lint_methods!`/`declare_combined_late_lint_pass!`.
  Register it separately (RFC 0002 §Suite integration).
- **Toolchain.** `rust-toolchain.toml` pins `nightly-2026-05-28`. Do not bump
  it.

## Tolerances (exception triggers)

- **Scope.** If implementation requires touching more than 30 files, or more
  than 1800 net lines outside the two new crates, stop and escalate. The new
  crates themselves are bounded by their milestone contents; if
  `crates/string_continuation_style/src/` exceeds 900 lines in total, stop and
  escalate, because the adapter was meant to be thin.
- **Dependencies.** Four changes are pre-authorized (`DEC-006`): add
  `googletest`, add `pretty_assertions`, promote `serial_test`, and add
  `log`/`tracing` dev-dependency edges to workspace pins that already exist.
  Any other dependency: stop and escalate. Note that `rustc_lexer` is already
  a workspace dependency and needs no authorization (`SURP-011`).
- **Public interface.** `whitaker-common` gains two additive functions
  (`DEC-012`). Any non-additive change to its public API: stop and escalate.
- **Time.** If any milestone exceeds two working days, stop and escalate with
  the current state. `EP-M0a` is boxed at two hours: if the transcript is not
  producible in that time, escalate with partial evidence and proceed on the
  `DEC-002` default.
- **Verification.** Kani: if the harness does not discharge within 20 minutes
  at its starting bound, shrink the bound rather than waiting, and record the
  reduced bound as a stated limit. If it does not discharge at the smallest
  meaningful bound after one working day, stop and escalate. Verus: if
  `LEM-REWRITE-1` is not discharged after one working day, stop and escalate
  with the open goals — do not weaken the lemma. (Both figures are calibrated
  against `docs/execplans/7-2-7-kani-verification-of-bounded-min-hasher-sketch-invariants.md`,
  which records two days for a comparable obligation.)
- **Test wall-clock.** `make test` wall-clock is measured before Step 1 and
  recorded as `EV-BASELINE`. If it grows by more than 40 per cent, stop and
  escalate with the measurement; the likely response is to move this crate
  into its own CI job, as the Makefile already does for three other lint
  crates via `TEST_EXCLUDES`.
- **Iterations.** If a UI fixture's `.stderr` cannot be made stable after 4
  attempts, stop and escalate; unstable diagnostic output usually means the
  span recovery is wrong, not that the fixture needs another nudge.
- **Ambiguity.** If a decision materially changes which source shapes the lint
  fires on, stop and present options with trade-offs.

## Risks

- **Risk: a literal captured by a `$l:literal` macro matcher looks
  root-context and is not.**
  Severity: high. Likelihood: high.
  This is the soundness hole `SURP-012` documents. It is the reason
  `DEC-002` adopts a wrapper-depth gate rather than RFC 0002's
  root-syntax-context test.
  Mitigation: the depth counter, plus `NV-DEPTH-1`, plus a
  `pass_literal_macro_capture` fixture that must be silent and whose
  gate-inversion control must make it fire.

- **Risk: wrapper-depth accounting misses an abstract-syntax-tree node kind.**
  Severity: high. Likelihood: medium.
  The counter must be maintained across every node kind that can contain an
  expression. Miss one and an expanded literal slips through with a
  suggestion. This is the one part of the design that is not pure-domain, so
  `proptest`, Kani, and Verus cannot reach it.
  Mitigation: `LEM-DEPTH-1` enumerates the enclosing node kinds and pairs each
  with a fixture; the run-time value gate of `LEM-EMIT-1` catches the
  consequence even when the cause slips through; the release applicability
  default of `DEC-013` means the first release cannot write to source.

- **Risk: the suggestion applies incorrectly under `cargo fix`.**
  Severity: high. Likelihood: low after mitigation.
  Mitigation: three independent layers. First, `LEM-EMIT-1`'s run-time value
  comparison inside the lint. Second, `// run-rustfix` fixtures that apply the
  suggestion, diff it against a checked-in `.fixed` file, recompile the result,
  and require it to produce no diagnostics (`SURP-014`). Third,
  `DEC-013`'s configuration default, which withholds `MachineApplicable`
  entirely in the first release.

- **Risk: release builds have overflow checks off; tests have them on.**
  Severity: high. Likelihood: medium.
  `installer/src/builder.rs:87` builds lint libraries with `--release`, and
  the workspace declares no `[profile.release]`, so `overflow-checks = false`.
  UI tests build with a plain `cargo build`, so checks are on. For a lint whose
  correctness is byte-offset arithmetic, a subtraction underflow panics in
  tests and wraps silently in the shipped artefact.
  Mitigation: `checked_*` arithmetic everywhere (a Constraint),
  `arithmetic_side_effects = "deny"` for both new crates, and
  `overflow-checks = true` added to `[profile.release]`.

- **Risk: the Kani harness is intractable.**
  Severity: medium. Likelihood: medium.
  A symbolic-length heap container driven through a nested loop is the exact
  shape that stalled the clone-detector proofs.
  Mitigation: start at a body length of 5 over a three-symbol alphabet, use a
  fixed-capacity `cfg(kani)` sink instead of a `Vec`, derive the unwind bound
  from that capacity with a `const` assertion, and keep `INV-SCAN-2` out of the
  Kani harness (proptest covers it). Escalation budget is in `Tolerances`.

- **Risk: Verus proof drifts from the production scanner.**
  Severity: medium. Likelihood: medium.
  `LEM-REWRITE-1` reasons about `Seq<u8>`, which cannot be `#[path]`-included
  from a `&str`-based production module, so it is a spec mirror.
  Mitigation: state the trust boundary in the proof's module doc comment, as
  `docs/developers-guide.md` §Verus scope and trust boundary requires; add the
  proof to a CI job (`DEC-011`) so it cannot rot silently; and note that
  `LEM-REWRITE-2` and `LEM-EMIT-1`, which are what actually protect users, are
  discharged against real compiler behaviour rather than against the mirror.

- **Risk: the user cannot silence a correct-but-unwanted finding.**
  Severity: medium. Likelihood: medium.
  A build script that writes a generated file containing wrapped literals
  produces warnings on a file the user does not own and cannot annotate.
  Mitigation: `DEC-013`'s configuration table ships in the first release, with
  `excluded_crates`, `excluded_paths`, and an unconditional skip for literals
  whose source file lies outside the workspace root or under the target
  directory.

- **Risk: the fixture matrix doubles the serialized test critical path.**
  Severity: medium. Likelihood: high without mitigation.
  Every dylint UI test runs in the `serial-dylint-ui` nextest group at
  `max-threads = 1` with a 10-minute `terminate-after`.
  Mitigation: `DEC-009` removes 14 fixtures from this plan; `EV-BASELINE` makes
  the growth measurable; a crate-specific `slow-timeout` override is in the
  Step 5 edit list; and the `Tolerances` entry names moving the crate into its
  own CI job as the prepared response.

## Progress

- [x] (2026-08-21) Branch created from `harden-lint-config` and pushed with
      upstream tracking.
- [x] (2026-08-21) Reconnaissance of crate template, suite integration, i18n,
      and testing conventions completed.
- [x] (2026-08-21) RFC 0002's compiler-API assumptions verified against
      `nightly-2026-05-28`.
- [x] (2026-08-21) First draft written.
- [x] (2026-08-21) Six-lens design review completed; fourteen further findings
      recorded in §Surprises & discoveries and eight decisions revised.
- [x] (2026-08-21) `DEC-009`, `DEC-013`, and `DEC-014` answered by the
      repository owner; the feature moved from roadmap item 2.2.10 to its own
      step, §2.4 String continuation style, and the branch, this file, and the
      pull request renamed to match.
- [ ] Plan approved as a whole.
- [ ] EP-M0a: expansion-shape probe.
- [ ] EP-M1: pure domain crate.
- [ ] EP-M2: plain cooked string literals.
- [ ] EP-M3: source-authored format strings from compiler built-ins.
- [ ] EP-M5: suite, installer, tooling, and documentation integration.
- [ ] Roadmap item 2.4.1 marked done.

## Surprises & discoveries

`SURP-001` to `SURP-010` were found while validating RFC 0002 against the
pinned toolchain. `SURP-011` to `SURP-016` came out of the design review.
Compiler citations are to
`~/.rustup/toolchains/nightly-2026-05-28-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/`.
Note that this is the `rustc-src` component, not the `rust-src` component at
`lib/rustlib/src/rust/`, which contains only `library/`.

- **SURP-001: rustc already proves "the format string was written in source".**
  `rustc_ast::FormatArgs::is_source_literal` is computed by
  `rustc_parse_format::Parser::new`, which compares the recovered source
  snippet against the parsed input and returns `false` when a proc macro has
  respanned the literal (the fix for rust-lang/rust#114865).
  Evidence: `rustc_parse_format/src/lib.rs:302-345`;
  `rustc_builtin_macros/src/format.rs:290`.
  Impact: RFC 0002 §Format strings steps 3 and 4 are largely discharged by
  checking this one flag.

- **SURP-002: `FormatArgs::span` is the format-string literal token's span.**
  `make_format_args` constructs `FormatArgs { span: fmt_span, .. }` where
  `fmt_span` comes from `expr_to_spanned_string`, which returns `expr.span` of
  the fully-expanded format-string expression — the literal token including
  its quotes when `is_source_literal` holds. `Span::from_inner(InnerSpan)` maps
  a byte offset within that token to a source span.
  Evidence: `rustc_builtin_macros/src/format.rs:170-181, 675-681`;
  `rustc_builtin_macros/src/util.rs:61-101`; `rustc_span/src/lib.rs:1256`.
  Impact: no bespoke span arithmetic over the enclosing macro call is needed.
  See also `SURP-013`, which draws a further consequence.

- **SURP-003: `uncooked_fmt_str` carries the raw source body and is not
  newline-appended.**
  The `Symbol` in `FormatArgs::uncooked_fmt_str: (token::LitKind, Symbol)` is
  `token_lit.symbol` — the raw, unescaped source text between the quotes.
  `println!`-family macros append `\n` to the cooked symbol (`fmt.symbol`) but
  leave `uncooked_symbol` untouched.
  Evidence: `rustc_ast/src/format.rs:44-59`;
  `rustc_builtin_macros/src/util.rs:61-101`;
  `rustc_builtin_macros/src/format.rs:190-192`.
  Impact: the scanner's input is `uncooked_fmt_str.1.as_str()` for format
  strings and `token::Lit::symbol` for plain literals. The `token::LitKind`
  discriminant is the cooked-versus-raw and type discriminator RFC 0002
  §Decision matrix needs.

- **SURP-004: `EarlyContext` implements `LintContext`; `Diag::span_suggestion`
  exists.**
  Evidence: `rustc_lint/src/context.rs:619`;
  `rustc_errors/src/diagnostic.rs:918`.
  Impact: the diagnostic path mirrors `bumpy_road_function`'s
  `cx.emit_span_lint(LINT, span, DiagDecorator(|lint| { ... }))`, adding
  `lint.span_suggestion(...)`.

- **SURP-005: `register_early_pass` exists, and the factory runs once per
  crate.**
  `register_early_pass` takes `impl Fn() -> EarlyLintPassObject`
  (`rustc_lint/src/context.rs:168`). The only site that instantiates
  `early_passes` is `rustc_interface/src/passes.rs:469`, inside the
  `early_lint_checks` query, called once per crate; the pre-expansion site at
  `passes.rs:97` uses the separate `pre_expansion_passes` list.
  Impact: per-crate state in the pass is sound *today*. `SURP-015` records why
  the plan does not rely on that.

- **SURP-006: `EarlyContext` has no `TyCtxt`.**
  `ExpnData` gives `kind: ExpnKind::Macro(MacroKind, Symbol)`,
  `macro_def_id: Option<DefId>`, `def_site: Span`, `allow_internal_unstable`,
  and `parent_module` (`rustc_span/src/hygiene.rs:969-1015`), but turning a
  `DefId` into a crate name or definition path needs `TyCtxt`, which
  `EarlyContext` does not hold. Early lints do run inside a query
  (`rustc_interface/src/passes.rs:400`), so a `TyCtxt` exists on the thread,
  reachable through `rustc_middle::ty::tls`.
  Impact: this drove the first draft's open architecture question. `DEC-002`
  now closes it without needing `TyCtxt` at all.

- **SURP-007: rustc normalizes CRLF out of source before lexing.**
  `normalize_src` calls `normalize_newlines` (`rustc_span/src/lib.rs:2510-2528`),
  so every literal `Symbol` and every `span_to_snippet` result contains LF
  only.
  Impact: RFC 0002 §Continuation scanner's CRLF branch is unreachable from the
  rustc adapter. See `DEC-004` and `SURP-016`.

- **SURP-008: `googletest` and `pretty_assertions` are not yet dependencies,
  and `serial_test` is pinned per-crate.**
  Neither appears in any manifest. `serial_test = "4.0.1"` is pinned
  identically in four crates: `conditional_max_n_branches`,
  `function_attrs_follow_docs`, `module_must_have_inner_docs`, and
  `no_std_fs_operations`.
  Impact: see `DEC-006`.

- **SURP-009: `rustc-literal-escaper` is already in `Cargo.lock`, and
  `ra_ap_syntax` re-exports it.**
  Version 0.0.4 is present transitively;
  `ra_ap_syntax-0.0.334/src/lib.rs:65` has
  `pub use rustc_literal_escaper as unescape;`.
  Impact: `DEC-007` originally proposed adding it as a direct oracle
  dependency. That decision is now withdrawn — see `SURP-014`.

- **SURP-010: `log 0.4.33`'s facade macros capture `$($arg:tt)+`, and have
  grown a `logger:` control.**
  `log::info!` matches
  `(logger: $logger:expr, target: $target:expr, $($arg:tt)+)`,
  `(target: $target:expr, $($arg:tt)+)`,
  `(logger: $logger:expr, $($arg:tt)+)`, and `($($arg:tt)+)`, forwarding to
  `log!` and then `__log!`; key-value fields are separated from the message
  by `;`. Evidence: `~/.cargo/registry/src/*/log-0.4.33/src/macros.rs:75-115,
  252-270`.
  Impact: RFC 0002's `log` allowlist is already stale, roughly one year after
  it was written, over a single new control. That is the empirical basis for
  `DEC-009`.

- **SURP-011: `rustc_lexer` is already a workspace dependency and is pure.**
  `Cargo.toml:44` pins `rustc_lexer = "0.1.0"`, the crates.io mirror, used by
  `crates/whitaker_clones_core` (see `src/token/normalize.rs`). It needs no
  `rustc_private` plumbing and it vendors an `unescape` module.
  Impact: the re-lex check that `LEM-APPLY-1` needs, and the unescaper that
  `LEM-EMIT-1` needs, are both available inside a pure crate at zero
  dependency cost. The first draft treated the lexer as an unmet need and the
  escaper as requiring approval; both were wrong.

- **SURP-012: root syntax context does not prove the literal is unwrapped.**
  In `rustc_expand/src/mbe/transcribe.rs`, `maybe_use_metavar_location`
  returns either the original token or one respanned with the call-site
  context. Either way a `$l:literal`-captured token reaches `check_expr` with
  `span.ctxt().is_root() == true`. So for

  ```rust
  macro_rules! shout { ($l:literal) => { let _x = $l; } }
  shout!("alpha \
          beta");
  ```

  every gate in RFC 0002 §Ordinary string expressions passes, and the lint
  would emit a machine-applicable suggestion that does not compile — the exact
  failure the Constraints forbid. The *container* of the substituted literal
  does carry the macro definition's marked context
  (`Marker::mark_span`, `transcribe.rs:92-103`), so tracking enclosing
  expanded nodes is the discriminator that `is_root()` is not.
  `EarlyLintPass` provides the needed hooks: `check_expr`/`check_expr_post`,
  `check_item`/`check_item_post`, `check_stmt`, and `check_mac_def`
  (`rustc_lint/src/passes.rs:143-172`).
  Impact: this is a soundness defect in RFC 0002, not in the plan's reading of
  it. It drives `DEC-002` and requires an RFC amendment.

- **SURP-013: `SURP-002` deletes half of RFC 0002's allowlist specification.**
  RFC 0002 §Detection architecture asks the lint to "parse the
  source-authored token tree and prove that the selected literal occupies the
  stated position" — argument 0 for `format!`, 1 for `write!`, 2 for
  `assert_eq!`. Since `FormatArgs::span` *is* the format-string literal token's
  span, there is nothing to count and no token tree to parse. RFC 0002
  §Non-goals separately forbids "inspecting arbitrary macro token trees whose
  grammar Whitaker cannot prove", so the RFC contradicts itself here.
  Impact: the position-proving machinery is dropped. Only the question "is the
  enclosing macro one whose matcher accepts an expression here" remains, and
  for compiler built-ins there is no macro-by-example matcher to break —
  `format_args!` parses an expression, and `write!`/`writeln!` forward through
  `$($arg:tt)*`.

- **SURP-014: `compiletest_rs` supports `// run-rustfix`, and
  `dylint_testing` does not support `TRYBUILD=overwrite`.**
  `dylint_testing 6.0.4` depends on `compiletest_rs 0.11.2`, not `trybuild`
  (`Cargo.lock:784-799`), and builds a `compiletest::Config` with
  `..Config::default()`, so `bless` is always `false`. The documented way to
  update a `.stderr` is to read the "Actual stderr saved to PATH" line from
  the report and copy that file
  (`~/.cargo/registry/src/*/dylint_testing-6.0.4/src/lib.rs:90-92`).
  Far more valuable: `compiletest_rs` in `Mode::Ui` honours a `// run-rustfix`
  header. When present it applies the machine-applicable suggestions with
  `rustfix`, diffs the result against a checked-in `.fixed` file, **compiles
  the fixed code**, and fails if it either does not compile or still produces
  diagnostics (`compiletest_rs-0.11.2/src/runtest.rs:2627-2645`).
  Impact: three things the first draft treated as unsolved come free. Applying
  the fix is verified. Recompiling it is verified. Idempotence — the fixed code
  producing no further diagnostics — is verified. And because the fixed code is
  *compiled*, a `const _: () = assert!(...)` inside the `.fixed` file makes
  **rustc itself** the value-preservation oracle, with no dependency and no
  shared lineage with our own unescaper. This is why `DEC-007` is withdrawn.

- **SURP-015: dylint UI fixtures needing dev-dependencies live in `examples/`,
  not `ui/`.**
  `Makefile:14-19` states it: "`dylint_testing::Test::example` builds them with
  each lint crate's dev-dependencies (tokio, rstest); `ui/` fixtures are
  standalone and cannot carry those." `crates/no_unwrap_or_else_panic`,
  `no_expect_outside_tests`, and `rstest_helper_should_be_fixture` all have
  `examples/` directories driven through `Test::example` and one rstest case
  per fixture, each spawning a cargo subprocess.
  Impact: any fixture needing `log` or `tracing` in scope must live in
  `examples/` and costs a cargo build, not a bare `rustc` invocation. It also
  means a `tracing/log`-feature fixture cannot coexist with a
  `tracing`-without-`log` fixture in one crate, because Cargo unifies
  dev-dependency features. This is a further argument for `DEC-009`.

- **SURP-016: the real continuation whitespace rule is not what RFC 0002
  says.**
  RFC 0002 §Continuation scanner specifies "spaces and tabs, plus complete LF
  or CRLF sequences" and says a lone carriage return makes the span
  unrecoverable. Both escapers disagree.
  `rustc-literal-escaper-0.0.4/src/lib.rs:442-472` and the vendored
  `rustc_lexer-0.1.0/src/unescape.rs:260-267` both skip every byte in
  `{b' ', b'\t', b'\n', b'\r'}`, in any order, until the first byte outside
  that set. A bare carriage return is skipped, not rejected, and CRLF has no
  special status. The current escaper additionally excludes the formfeed
  character (rust-lang/rust#136600) and warns on a skipped blank line and on
  a following non-ASCII whitespace character.
  Impact: `INV-SCAN-3` as first stated would have verified the scanner against
  a wrong specification. The corrected rule is now normative, and RFC 0002
  §Continuation scanner must be amended.

## Decision log

- **DEC-001: Implement as a post-expansion early lint, per RFC 0002.**
  Rationale (corrected after review): pre-expansion cannot see
  `FormatArgumentKind::Captured`. A late high-level intermediate
  representation pass cannot see `ExprKind::FormatArgs` at all, because format
  arguments are lowered away — that, not loss of source spelling, is the real
  disqualifier. Source spelling *is* recoverable at any phase through
  `span_to_snippet`, so RFC 0002 §Alternatives considered's stated reason for
  rejecting a late pass is imprecise and should be amended.
  Date/Author: 2026-08-21, planning agent.

- **DEC-002: Prove the *position*, not the macro's identity. RESOLVED.**
  The first draft left three options open, all of which existed only to answer
  "which macro produced this literal". The lint does not need that answer. It
  needs "is anything standing between this literal and the compiler", which is
  answerable from `EarlyContext` alone. The adopted design:
  1. **Wrapper depth.** Maintain a counter incremented in `check_item`,
     `check_stmt`, and `check_expr` when the node's span `from_expansion()`,
     and decremented in the matching `_post` hooks. Fire only at depth zero.
     This closes `SURP-012`, which `is_root()` alone does not.
  2. **Built-in name set.** For `ExprKind::FormatArgs`, require the outermost
     non-root frame of the literal span's expansion chain to be
     `ExpnKind::Macro(MacroKind::Bang, sym)` with `sym` in a fixed set:
     `format_args`, `format`, `print`, `println`, `eprint`, `eprintln`,
     `write`, `writeln`, `panic`, `todo`, `unimplemented`, `unreachable`,
     `assert`, `debug_assert`, `assert_eq`, `assert_ne`, `debug_assert_eq`,
     `debug_assert_ne`. The outermost frame is the invocation the user typed.
  3. **Crate-scoped shadow kill switch.** A bare name check is only sound if
     the name cannot be shadowed. Using `check_mac_def` and `check_item`, if
     the crate under compilation defines a `macro_rules!`/`macro` with any
     name in the set, or contains a `use` whose terminal segment is one, or a
     `#[macro_use] extern crate`, the lint disables itself for that crate.
     Blunt, and biased entirely towards silence.
  Consequences: no `TyCtxt`, no `rustc_middle`, no thread-local reach-through,
  no two-pass registration, no shared storage, and no allowlist table. `log`
  and `tracing` fail the outermost-frame check and produce silence by
  construction rather than by policy — which is what `DEC-009` wants.
  Costs accepted: the residual hole is a crate reaching a same-named macro by
  a path none of the three switches observes; the failure mode is silence, so
  it is undetectable from inside the lint. And wrapper-depth accounting is a
  new correctness surface that the pure-domain verification cannot reach; it
  gets `LEM-DEPTH-1` and its own fixture matrix instead.
  Date/Author: 2026-08-21, planning agent, on the design review's alternatives
  finding.

- **DEC-003: Hexagonal split — domain modules take no rustc types.**
  The scanner, the policy, and the rewriter are pure over `&str` and byte
  offsets. The rustc adapter converts abstract-syntax-tree nodes into a
  `LiteralFacts` value and converts a disposition back into spans. The
  diagnostic adapter converts a disposition into localized output.
  Rationale: the `hexagonal-architecture` skill's dependency rule, and the
  practical benefit that every provable invariant lives in the pure half.
  Note the review's caution, accepted: the adapter is where the byte offsets
  are, and byte offsets are the whole risk. `DEC-003` must not aim the
  verification effort away from the danger. That is what `LEM-EMIT-1`,
  `LEM-SPAN-1`, and `LEM-DEPTH-1` exist for.
  Date/Author: 2026-08-21, planning agent.

- **DEC-004: Keep the whitespace rule complete in the pure scanner; do not
  claim adapter coverage for carriage returns.**
  Rationale: `SURP-007` and `SURP-016`. The pure scanner is a reusable
  component whose contract should be complete and *correct*; the adapter never
  feeds it a carriage return. The verification plan records this as an explicit
  reachability bound rather than pretending to cover it.
  Date/Author: 2026-08-21, revised 2026-08-21 after `SURP-016`.

- **DEC-005: One diagnostic per literal, with one whole-literal replacement.**
  Rationale: RFC 0002 §Decision matrix, final row. Cascaded per-continuation
  suggestions would overlap under `rustfix`, which reports that it could not
  apply the fixes and can leave a file partially rewritten.
  Date/Author: 2026-08-21, planning agent.

- **DEC-006: Dependency changes.**
  Add `googletest` and `pretty_assertions` to `[workspace.dependencies]`; the
  task brief authorizes both and neither is present. Promote `serial_test`
  from four identical per-crate `4.0.1` pins to a workspace pin and migrate
  all four crates. `log` and `tracing` dev-dependency edges are not needed:
  `DEC-009` moved the facades to roadmap item 2.4.2.
  Date/Author: 2026-08-21, planning agent.

- **DEC-007: WITHDRAWN.** The first draft proposed adding
  `rustc-literal-escaper` as a differential oracle. `SURP-014` supplies a
  strictly better one at zero cost: the compiler under test. A
  `const _: () = assert!(...)` inside a `.fixed` file is adjudicated by rustc's
  own escaping implementation during the run-rustfix recompile, with no
  possibility of shared misunderstanding. `SURP-011` supplies the in-process
  unescaper for `LEM-EMIT-1` from an existing workspace dependency.
  Date/Author: 2026-08-21, withdrawn 2026-08-21 after review.

- **DEC-008: SUPERSEDED by `DEC-009`.** The first draft proposed rejecting
  `log`'s `logger:` control. `DEC-009` removes the `log` allowlist entirely.

- **DEC-009: Move `log` and `tracing` support out of this item. APPROVED.**
  The facades are cut by construction — `DEC-002`'s outermost-frame check
  produces silence for them — rather than by deferral, so nobody ships half an
  allowlist.
  Rationale: the roadmap text for this work never mentioned either crate. They
  were the sole driver of the whole `TyCtxt` question. The allowlist is a
  policy table over third-party macro grammars that change between versions,
  and `SURP-010` shows RFC 0002's was already stale roughly a year after it was
  written. `SURP-015` shows their fixtures cannot all live in one crate anyway,
  because Cargo unifies dev-dependency features. They accounted for 14 of
  roughly 34 fixtures on a serialized test path.
  Accepted cost: in service codebases long strings disproportionately live in
  log messages, which is exactly where the hand-application this work replaces
  actually happens. Coverage there is deferred, not abandoned.
  Consequence: the approver directed that the whole feature move out of
  §2.2 Core lint implementations into its own roadmap step rather than
  overburdening that step. `docs/roadmap.md` now carries §2.4 String
  continuation style with three tasks: 2.4.1 (this plan), 2.4.2 (the facade
  allowlist, to be designed against a shipped baseline), and 2.4.3 (`DEC-013`'s
  promotion to machine applicability). The branch, this file, and the pull
  request were renamed to match.
  Date/Author: 2026-08-21, planning agent; approved 2026-08-21 by the
  repository owner.

- **DEC-010: Put the pure domain in its own plain-library crate.**
  Create `crates/whitaker_string_literals/` as an ordinary `rlib` with no
  `crate-type` override and no rustc dependency, following
  `crates/whitaker_clones_core/` exactly.
  `crates/string_continuation_style/` becomes a thin `cdylib` adapter that
  depends on it.
  Rationale: five problems collapse into one fix. Kani has never run inside a
  `cdylib` lint crate in this repository, and `whitaker_clones_core` proves it
  works in a plain crate. Verus proofs `#[path]`-include production modules
  from plain crates. Doctests on the pure functions actually run, because the
  crate is not in `LINT_CRATES` and therefore not in `DOCTEST_EXCLUDES`.
  `make lint-whitaker` can cover it, because `WHITAKER_PACKAGES` can include a
  crate that builds as an ordinary library — which means the 400-line ceiling
  is *gate-enforced* for the domain rather than merely asserted. And domain
  purity becomes structural rather than a conformance check that no
  `--all-features` gate would ever notice.
  Date/Author: 2026-08-21, planning agent, on the review's structural findings.

- **DEC-011: Put Kani and Verus in continuous integration.**
  Neither runs today; `.github/workflows/` contains one proof-adjacent step,
  `scripts/check-verus-fragment-id-bridge.sh`, which is four `grep -F` calls.
  A proof outside a gate is a comment. Add a `proofs` job running `make kani`
  and `make verus`, scheduled nightly and on pull requests that touch
  `verus/**`, `crates/whitaker_string_literals/**`, or `scripts/run-*.sh`.
  Rationale: the plan spends most of its verification budget on artefacts
  that would otherwise be discharged once by hand and never re-run. The
  toolchain-bump scenario is the concrete danger: a bump patches the adapter to
  compile, the `.stderr` fixtures get re-blessed as "diagnostic drift", and
  nobody notices the proof now describes code that no longer exists.
  Date/Author: 2026-08-21, planning agent.

- **DEC-012: Promote a label-resolution helper into `whitaker-common`.**
  Add `safe_resolve_label(localizer, resolution, attribute, report_bug,
  fallback) -> String` and `normalize_fluent_output(text: String) -> String`,
  and make `strip_isolating_marks` delegate to the latter. Migrate
  `bumpy_road_function` onto them in the same change.
  Rationale: three divergent normalization contracts already exist in-tree —
  `strip_isolating_marks` strips `{U+2068, U+2069}`; `bumpy_road_function` and
  `no_std_fs_operations` strip different sets including `U+FFFD`;
  `conditional_max_n_branches` *replaces* them with a quote character. Copying
  the workaround would make a fourth. Stripping `U+FFFD` is itself a bug: it is
  how Fluent signals an unresolved placeable, so deleting it converts "this
  translation is broken" into "this translation reads plausibly and is wrong".
  The typed `BundleLookup::attribute(MessageKey, AttrKey, &Arguments)` already
  exists at `common/src/i18n/diagnostics.rs:73-93` and is bypassed by every
  caller. The change is purely additive, so it sits inside `Tolerances`.
  Date/Author: 2026-08-21, planning agent.

- **DEC-013: Ship a configuration table, defaulting to manual applicability.
  APPROVED.**
  RFC 0002 says "No configuration section is proposed. The proof predicate is
  the configuration." For a warn-level style lint that is defensible; for a
  lint that writes to source files it is the wrong instinct for release one.
  Adopted: a `[string_continuation_style]` table in `dylint.toml` with
  `applicability = "manual" | "machine"` defaulting to `"manual"`, plus
  `excluded_crates` and `excluded_paths` mirroring `no_std_fs_operations`'s
  shape exactly. Under `"manual"` the diagnostic still prints the full
  replacement; only the unattended write is disarmed. Flipping one line in
  `dylint.toml` arms the fixer estate-wide once the run-rustfix harness has
  proven itself over real rewrites, with no rebuild.
  Independently of the setting, skip any literal whose source file lies
  outside the workspace root or beneath the target directory, with a new
  `RequiredReason::GeneratedSourceFile` and a fixture pinning it. The
  motivating case is a build script writing `$OUT_DIR/protocol.rs`, included
  with `include!`: the user cannot annotate a file they do not write, and a
  fix would rewrite a file the next build deletes.
  Consequence: promoting the default from `"manual"` to `"machine"` is
  deliberately *not* in this plan's scope. It is roadmap item 2.4.3, gated on
  field evidence from an apply-and-recompile pass over the estate. This plan
  builds the machinery that makes that evidence collectable — the
  `// run-rustfix` fixtures of `SURP-014` and the run-time value gate of
  `LEM-EMIT-1` — and stops there.
  Date/Author: 2026-08-21, planning agent; approved 2026-08-21 by the
  repository owner.

- **DEC-014: One lint name. SETTLED.**
  Ship a single lint, `string_continuation_style`, covering both the
  plain-literal and format-string branches, as RFC 0002 and the roadmap
  describe.
  Rationale, from the approver: a user does not care about the internal split
  between the two branches unless their semantics are fundamentally different,
  and here they are not — both say "this source-line join could be a
  `concat!()`". The review's alternative of a second
  `format_string_continuation_style` name would have bought `#[allow]`
  granularity between a high-confidence and a lower-confidence branch, at the
  cost of exposing an implementation boundary in the user-facing lint list.
  With `DEC-013`'s configuration table providing a kill switch and `DEC-002`
  making the format branch no riskier than the plain one, that granularity has
  no remaining user to serve.
  Date/Author: 2026-08-21, planning agent; settled 2026-08-21 by the
  repository owner.

- **DEC-015: Order the adapter's checks cheapest-first, normatively.**
  RFC 0002 §Ordinary string expressions puts `span_to_snippet` and a re-lex at
  step 2, before anything has established the literal is a candidate. That runs
  a source-file binary search and a `String` allocation for every cooked
  literal in the crate — plausibly 30,000 to 80,000 per full-workspace build
  here, for a candidate rate of essentially zero (`git ls-files '*.rs' |
  xargs grep -c '\\$'` finds no continuation backslashes in this repository at
  all). Worse, `span_to_snippet` on a foreign `SourceFile` decodes an external
  crate's source out of crate metadata and retains it in the `SourceMap` for
  the rest of the compilation.
  Normative order: (1) `token::LitKind::Str`, touching no source map;
  (2) the uncooked `Symbol` contains `\` followed by LF — an interned lookup
  and an allocation-free scan; (3) wrapper depth is zero and the span is not
  from an expansion; (4) only then `span_to_snippet` and the re-lex.
  Date/Author: 2026-08-21, planning agent.

- **DEC-016: Construct the localizer in `check_crate`, never in `check_expr`.**
  `SharedConfig::load()` clones a `toml::Value` and runs a serde deserialize
  per call, and building a Fluent bundle is not free. Every existing lint does
  this in `check_crate`; `EarlyLintPass` offers the same hook. Stated
  explicitly because the first draft quoted the idiom without naming the hook,
  and a literal reader could reasonably have put it in `check_expr`.
  Date/Author: 2026-08-21, planning agent.

## Outcomes & retrospective

Not started. To be completed at each milestone boundary and at plan closure.

Before setting this plan to `COMPLETE`, reconcile every discovery against
`docs/rfcs/0002-string-continuation-style.md`. `SURP-012` and `SURP-016` are
defects in the RFC, not merely divergences: the first would ship a
source-corrupting suggestion, the second specifies the wrong language rule.
`SURP-001`, `SURP-002`, `SURP-003`, `SURP-007`, `SURP-010`, and `SURP-013`
describe places where the RFC's prescribed mechanics differ from the pinned
toolchain's behaviour. `DEC-001`'s rationale correction and `DEC-009`'s scope
change also require RFC edits. The RFC must be substantively revised, not
annotated, before it moves from *Proposed* to *Accepted*.

## Context and orientation

### What this repository is

Whitaker is a suite of [Dylint](https://github.com/trailofbits/dylint) lints
for Rust. Dylint loads compiled lint libraries into `rustc` at build time, so
each lint crate is a `cdylib` compiled against the pinned nightly compiler's
private crates.

Layout you need to know:

- `crates/<lint_name>/` — one directory per lint. Ten exist today.
- `crates/rustc_ast/`, `crates/rustc_lint/`, `crates/rustc_span/`,
  `crates/rustc_session/`, `crates/rustc_middle/`, `crates/clippy_utils/` —
  thin proxy crates re-exporting the nightly compiler crates. Depend on these
  by workspace alias, never on the raw `extern crate`.
- `crates/whitaker_clones_core/` — a plain `rlib` of pure logic used by lint
  crates. It carries Kani harnesses, `proptest` properties, `insta`
  snapshots, and Verus-bridged modules. **This is the template for
  `crates/whitaker_string_literals/`**, the new pure-domain crate.
- `common/` — the `whitaker-common` crate: pure helpers plus the Fluent
  localization engine.
- `suite/` — `whitaker_suite`, the aggregated `cdylib` bundling every lint.
- `installer/` — the `whitaker` command-line tool that builds, links, and
  stages lint libraries.
- `src/` — the top-level `whitaker` crate: the command-line tool plus the
  shared test harness (`src/testing/ui/`) and rustc-aware span helpers
  (`src/hir/`).
- `verus/` — Verus proof files, run out-of-band from Cargo.
- `docs/` — the knowledge base. Start at `docs/contents.md` and
  `docs/repository-layout.md`.

`rust-toolchain.toml` pins `nightly-2026-05-28` with `rustc-dev` and
`rust-src`.

### Terms used in this plan

- **Cooked string literal** — an ordinary `"..."` literal, in which backslash
  escape sequences are interpreted. Contrast a *raw* literal `r"..."`, where
  they are not.
- **Continuation escape** — a backslash immediately followed by a source
  newline inside a cooked literal, plus the whitespace the compiler then
  consumes. Its evaluated value is empty.
- **Source-line join** — a continuation escape with source content both before
  it on its physical line and after the whitespace it consumes. Its only
  purpose is to wrap one logical string across two source lines. This is what
  the lint targets.
- **Layout trim** — a continuation used to suppress a leading or trailing
  source newline and indentation rather than to divide two fragments, as in
  `"\` at the very start of a literal. The lint leaves these alone.
- **Post-expansion early lint** — an `EarlyLintPass` running on the abstract
  syntax tree after macro expansion but before lowering to the high-level
  intermediate representation. It sees `ExprKind::FormatArgs`, which records
  how `format!`-family arguments were bound.
- **Implicit format capture** — writing `format!("{name}")` so that `name` is
  captured from the enclosing scope. The compiler permits this only when the
  format string is a direct source literal, which is exactly why a `concat!()`
  rewrite would break it.
- **Wrapper depth** — this plan's counter of enclosing abstract-syntax-tree
  nodes whose span came from a macro expansion. Zero means nothing stands
  between the literal and the compiler. See `DEC-002` and `SURP-012`.
- **Applicability** — rustc's confidence label on a suggestion.
  `MachineApplicable` means `cargo fix` may apply it unattended.
- **Bless** — replacing a `.stderr` expectation with the output the code
  currently produces. In this repository that is a manual copy, not a flag;
  see `SURP-014`.

### What exists today that you will copy

**Pure-domain crate shape.** `crates/whitaker_clones_core/Cargo.toml`: no
`[lib]` section, `publish = false`, workspace-inherited metadata,
`rustc_lexer = { workspace = true }`, and dev-dependencies including `insta`,
`proptest`, `rstest`, `rstest-bdd`, and `rstest-bdd-macros`. Kani harnesses
live in `#[cfg(kani)] mod kani;` modules; the workspace already declares
`unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)'] }`
(`Cargo.toml:180`), so a crate with `[lints] workspace = true` needs no local
`check-cfg` entry.

**Lint crate shape.** `crates/no_unwrap_or_else_panic/` is the template:
`[lib] crate-type = ["cdylib", "rlib"]`, `publish = false`, and two features —
`dylint-driver` gating every compiler dependency as `optional = true`, and
`constituent = ["dylint-driver", "dylint_linting/constituent"]`. `src/lib.rs`
opens with `#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]`
and gates every module, with a `stub` module filling the crate when the
feature is off. The `crates/*` glob in the workspace `members` list picks up a
new directory automatically.

**Lint declaration.** `crates/conditional_max_n_branches/src/driver.rs:73-91`
wraps the declaration macro in a private module so the macro-generated,
source-location-free items do not trip `missing_docs`:

```rust
mod declaration {
    #![expect(
        missing_docs,
        reason = "dylint_linting macro expansion emits items with no \
                  documentable source location"
    )]
    use super::ConditionalMaxNBranches;
    dylint_linting::impl_late_lint! {
        /// ...
        pub CONDITIONAL_MAX_N_BRANCHES,
        Warn,
        "...",
        ConditionalMaxNBranches::default()
    }
}
pub use declaration::CONDITIONAL_MAX_N_BRANCHES;
```

`dylint_linting` 6 provides `impl_early_lint!` with the same shape.

**No early lint exists yet.** Every current lint is a `LateLintPass`. This will
be the repository's first `EarlyLintPass`, so write the registration from
`SURP-005`.

**Diagnostics.** `crates/bumpy_road_function/src/driver/diagnostic.rs:33-83` is
the model, being the only lint that emits per-item secondary labels:

```rust
let messages = safe_resolve_message_set(
    localizer, resolution, noop_reporter, || fallback_messages(/* ... */),
);
cx.emit_span_lint(
    BUMPY_ROAD_FUNCTION,
    input.primary_span,
    rustc_lint::errors::DiagDecorator(|lint| {
        lint.primary_message(messages.primary().to_owned());
        lint.span_note(input.primary_span, messages.note().to_owned());
        // one span_label per highlighted item
        lint.help(messages.help().to_owned());
    }),
);
```

`clippy_utils` is vendored but unused by every lint's logic; do not reach for
`span_lint_and_then`.

**Localization.** `whitaker_common::i18n` wraps
`fluent_templates::static_loader!` over `common/locales/{en-GB,cy,gd}/`. One
`.ftl` file per lint, named after the lint slug, with the message keyed by the
slug and attributes `.note`, `.help`, and `.label`. A lint obtains its
localizer once per crate in `check_crate` (`DEC-016`):

```rust
let shared_config = SharedConfig::load();
self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
```

and resolves the primary/note/help triple by passing a
`MessageResolution { lint_name, key, args }` to
`safe_resolve_message_set(&localizer, resolution, noop_reporter, fallback)`.
Labels go through the new `safe_resolve_label` of `DEC-012`.

**UI tests.** `crates/no_std_fs_operations/src/tests/ui.rs` is the
locale-aware template:

```rust
use serial_test::serial;
use whitaker_common::test_support::with_locale;

#[test] #[serial] fn ui() { run_with_locale("ui", None); }
#[test] #[serial] fn ui_runs_in_welsh() { run_with_locale("ui-cy", Some("cy")); }
#[test] #[serial] fn ui_runs_in_gaelic() { run_with_locale("ui-gd", Some("gd")); }

fn run_with_locale(directory: &str, locale: Option<&str>) {
    with_locale(locale, || {
        whitaker::run_ui_tests!(directory)
            .expect("UI tests should execute without diffs");
    });
}
```

`#[serial]` is mandatory. Never mutate the environment directly in a test; use
`whitaker_common::test_support::{with_locale, with_env_var,
with_env_var_removed}`. Note that nextest runs each test in its own process,
so the real serialization comes from the `serial-dylint-ui` group in
`.config/nextest.toml`, whose membership is an explicit filter — the layout
above matches the `test(tests::ui::)` clause, so do not rename it.

**Behavioural tests.** Feature files at `<crate>/tests/features/*.feature`;
step definitions at `<crate>/tests/<name>_behaviour.rs`. Scenarios bind by
crate-relative path and zero-based index. Fixtures carrying a "world" struct
are wrapped in `#[whitaker_test_macros::allow_fixture_expansion_lints]`.

**Suite registration.** `suite/src/lints.rs` holds two parallel, order-coupled
arrays: `SUITE_LINTS` and `SUITE_LINT_DECLS`.
`suite/tests/registration.rs::then_decls_align` asserts they agree.
`suite/src/driver.rs:65-68` is the function to extend.

**Two hard-coded lint lists.** `installer/src/resolution.rs:17`
(`LINT_CRATES`, a Rust slice) and `Makefile:58` (`LINT_CRATES`, a Make
variable that additionally contains `whitaker_suite`).

**Gates.** `make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make markdownlint`, `make nixie`. Note two limits the first draft got wrong.
`make lint-whitaker` runs the suite only over `WHITAKER_PACKAGES`
(`Makefile:81`), which contains no lint crate, because `cargo dylint`'s plain
check build cannot provide `rustc_private` plumbing — so `module_max_lines` is
*not* enforced on `cdylib` lint crates today, and files over 400 lines already
exist there. It *can* be enforced on the new plain-library crate, which is one
of `DEC-010`'s reasons. And `DOCTEST_EXCLUDES` excludes every crate in
`LINT_CRATES`, so doctests in the `cdylib` crate never run.

### The upstream specification

`docs/rfcs/0002-string-continuation-style.md` is the design of record. Read it
in full before starting — but read §Surprises & discoveries here first. Two of
its clauses are defective (`SURP-012`, `SURP-016`) and several others are
superseded.

### Skills and documents to load

Skills: `rust-router`, then `rust-unit-testing` and `rust-types-and-apis`;
`hexagonal-architecture` for `DEC-003`; `proptest`; `kani`; `verus`; `leta`
for navigation (prefer `leta show`/`leta refs` over reading whole files);
`execplans` for keeping this document current; `commit-message`;
`en-GB-oxendict`.

Documents: `AGENTS.md`; `docs/whitaker-dylint-suite-design.md` (entirely
late-pass oriented, with no early-pass guidance — `EP-M5` adds it);
`docs/developers-guide.md`, especially §Verus scope and trust boundary and
§Toolchain and parser maintenance runbooks; `docs/users-guide.md` §Available
Lints; `docs/documentation-style-guide.md`;
`docs/rust-testing-with-rstest-fixtures.md` and
`docs/rstest-bdd-users-guide.md` (both adapted upstream material citing paths
that do not exist here — trust `AGENTS.md` and crate precedent over their
examples); `docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/rust-doctest-dry-guide.md`;
`docs/execplans/7-2-7-kani-verification-of-bounded-min-hasher-sketch-invariants.md`
for what a comparable Kani obligation actually cost.

## Conformance basis

Upstream artefacts, at the revisions current on this branch (based on
`harden-lint-config` at `9b758d1`):

- **RFC-0002** — `docs/rfcs/0002-string-continuation-style.md`, status
  *Proposed*. The design of record, with the defects recorded above.
- **ROADMAP-2.4.1** — `docs/roadmap.md` §2.4 String continuation style, first
  task. Depends on ROADMAP-2.1.1 (lint crate template, done) and ROADMAP-2.3.4
  (locale selection, done). Its sibling tasks 2.4.2 (the `log` and `tracing`
  facade allowlist) and 2.4.3 (promotion to machine applicability) are
  explicitly out of scope here and depend on this one.
- **SUITE-DESIGN** — `docs/whitaker-dylint-suite-design.md`.
- **AGENTS** — `AGENTS.md`.
- **STYLE** — `docs/documentation-style-guide.md`.

There is no Terms of Reference document; ROADMAP-2.4.1 plus RFC-0002 serve
that role.

Trace links:

```plaintext
ROADMAP-2.4.1 -> RFC-0002 §Continuation scanner -> EP-M1 -> INV-SCAN-1 -> crates/whitaker_string_literals/src/continuation/tests.rs
ROADMAP-2.4.1 -> RFC-0002 §Rewrite construction -> EP-M1 -> LEM-REWRITE-1 -> verus/string_continuation_rewrite.rs
ROADMAP-2.4.1 -> SURP-012 -> EP-M2 -> LEM-DEPTH-1 -> crates/string_continuation_style/ui/pass_literal_macro_capture.rs
ROADMAP-2.4.1 -> RFC-0002 §Ordinary string expressions -> EP-M2 -> crates/string_continuation_style/ui/fail_plain_join.rs
ROADMAP-2.4.1 -> Constraints §evaluated-value -> EP-M2 -> LEM-EMIT-1 -> crates/string_continuation_style/ui/fail_value_preserved.fixed
ROADMAP-2.4.1 -> RFC-0002 §Why the motivating example passes -> EP-M3 -> crates/string_continuation_style/ui/pass_implicit_format_capture.rs
ROADMAP-2.4.1 -> RFC-0002 §Suite integration -> EP-M5 -> suite/tests/registration.rs::then_early_pass_registered
ROADMAP-2.4.1 -> AGENTS §Documentation maintenance -> EP-M5 -> docs/users-guide.md §string_continuation_style
```

## Verification plan

The structure in `DEC-003` and `DEC-010` was chosen so that every pure
obligation is reachable without a compiler session — and, learning from the
review, so that the *adapter's* obligations are named too, since that is where
the byte offsets are.

### Axioms (assumed, not verified here)

Each is discharged behaviourally at `EP-M0a` and recorded in
`EV-M0-transcript`, rather than resting on a source citation that a toolchain
bump invalidates.

- **AXIOM-1.** `is_source_literal == true` implies the format string was
  written as a direct literal whose recovered snippet matches the parsed
  input. Basis `SURP-001`. Boundary-verified by
  `pass_generated_format_string.rs`.
- **AXIOM-2.** `FormatArgs::span` is the format-string literal token's span,
  and `Span::from_inner(InnerSpan::new(a, b))` yields the sub-span at byte
  offsets `a..b` from that token's start. Basis `SURP-002`. Boundary-verified
  by the byte-exact `.stderr` caret positions and by `LEM-SPAN-1`.
- **AXIOM-3.** `uncooked_fmt_str.1` is the raw source body, unmodified by the
  `println!` newline append. Basis `SURP-003`. Boundary-verified by
  `fail_println_join`, whose `.fixed` file would gain a stray fragment.
- **AXIOM-4.** After a continuation escape's newline, Rust discards every
  subsequent byte in `{' ', '\t', '\n', '\r'}`, in any order, until the first
  byte outside that set, excluding formfeed. Basis `SURP-016`. This is the rule
  the scanner implements; `INV-SCAN-3` tests it.
- **AXIOM-5.** `concat!()` yields a `&'static str` and, when it supplies a
  format string, disables implicit named capture. Basis RFC-0002 §Source basis.
  Boundary-verified by `pass_implicit_format_capture.rs`.
- **AXIOM-6.** rustc normalizes CRLF to LF before lexing, so adapter-supplied
  bodies contain LF only, and rustc un-normalizes byte offsets back to on-disk
  positions before `rustfix` sees them. Basis `SURP-007`. The second half is
  boundary-verified by the Windows continuous-integration job running the
  run-rustfix fixtures.
- **AXIOM-7.** `register_early_pass` causes the pass to run once per crate,
  after expansion, before lowering. Basis `SURP-005`. Note `SURP-015`'s
  caution: the plan does not *depend* on the once-per-crate property for
  correctness, only for cost.

### Pure-domain obligations

**INV-SCAN-1 — continuation detection is backslash-parity-correct.**

- Statement: a byte offset `i` begins a continuation escape if and only if the
  byte at `i` is `\`, the maximal run of consecutive backslashes ending at `i`
  has odd length, and the byte at `i + 1` is a source newline.
- Method: `rstest` parameterized cases over the finite boundary partition
  (runs of length 0 to 4, before LF, before CR, before tab, before
  end-of-body), plus a Kani harness.
- Rationale: the parity rule is where a naive implementation goes wrong, and
  the interesting alphabet is tiny.
- Domain and bound: Kani over bodies of length ≤ 5 drawn from
  `{'\\', '\n', 'a'}` — 364 paths. Grow only if it discharges quickly. Use a
  `#[cfg(kani)]` fixed-capacity sink `[Option<Continuation>; 3]` in place of
  the `Vec`, mirroring `whitaker_clones_core`'s `KANI_MAX_RECORDED_PAIRS`
  pattern, and derive the unwind bound from that capacity with a
  `const _: () = assert!(...)`. **Stated limit:** the alphabet excludes `'\r'`,
  `'\t'`, and space, so the Kani result is not exhaustive over `INV-SCAN-3`'s
  whitespace set. Those bytes are covered by parameterized cases and proptest
  only.
- Artefact: `crates/whitaker_string_literals/src/continuation/tests.rs` and
  `.../continuation/kani.rs`.
- Evidence: `cargo nextest run -p whitaker_string_literals continuation::`
  and `scripts/run-kani.sh string-literals`. Both must fail before the scanner
  exists.
- Non-vacuity: `"a\\\nb"` (odd run, is a continuation) and `"a\\\\\nb"` (even
  run, is a literal backslash then a real newline) must both be exercised and
  classified differently. Negative control **NV-SCAN-1**: replace the parity
  test with `is_backslash(prev)` and require Kani to produce a counter-example
  naming the even-run input.

**INV-SCAN-2 — recorded ranges are well-formed.**

- Statement: for every emitted `Continuation`,
  `escape_range.start < escape_range.end <= skipped_whitespace_range.start <=
  skipped_whitespace_range.end <= body.len()`; continuations are strictly
  ordered; and no two combined intervals overlap.
- Method: `proptest`, asserted on every scan in every property test through a
  shared `assert_scan_well_formed` helper. Deliberately **not** in the Kani
  harness: an O(n²) pairwise assertion over a symbolic-length container buys
  nothing over proptest and is exactly the shape that stalls the solver.
- Domain: generated bodies mixing ASCII, Unicode, escaped quotes, escaped
  backslashes, blank lines, and one to five continuations.
- Artefact: `crates/whitaker_string_literals/src/continuation/props.rs`.
- Evidence: `cargo nextest run -p whitaker_string_literals continuation::props`.
- Non-vacuity: the generator is construction-based, not filter-based, and a
  `prop_assert!(scan.len() >= 1)` guard fails loudly if it stops producing
  continuations. Negative control **NV-SCAN-2**: make the scanner emit
  `skipped_whitespace_range = escape_range` and require the ordering or
  disjointness assertion to reject it.

**INV-SCAN-3 — the skipped-whitespace range matches the language rule.**

- Statement: `skipped_whitespace_range` covers exactly the bytes rustc
  discards after the continuation's newline, per `AXIOM-4`.
- Method: `rstest` cases for the boundaries (immediately-following content,
  blank lines, whitespace-only trailing run, mixed tab and carriage return, end
  of body), plus a `proptest` differential against
  `rustc_lexer::unescape::unescape_str` (`SURP-011`), plus — the strongest
  layer — the run-rustfix fixtures of `LEM-REWRITE-2`, where the real compiler
  adjudicates.
- Rationale: this is the obligation most likely to be got subtly wrong, and it
  is the one where independent oracles exist at two levels of strength.
- Domain: as `INV-SCAN-2`, with the added generator requirement that at least
  one continuation is followed by non-empty indentation.
- Artefact: `crates/whitaker_string_literals/src/continuation/props.rs`.
- Non-vacuity: the generator emits non-empty indentation after every newline,
  so an implementation deleting only `escape_range` fails. Negative control
  **NV-SCAN-3**: that exact mutation must be rejected with a diff showing the
  retained spaces.

**INV-CLASS-1 — classification is total and mutually exclusive.**

- Statement: every continuation receives exactly one of `Join`,
  `LeadingLayoutTrim`, `TrailingLayoutTrim`; `Join` holds if and only if there
  is source content before the backslash on its physical line *and* source
  content after all consumed whitespace; when neither side has content,
  `LeadingLayoutTrim` wins.
- Method: `rstest` parameterized cases over the 2×2 content matrix plus the
  precedence tie, and an `exactly_one_of` assertion inside the `INV-SCAN-1`
  Kani harness (free, since the symbolic input is already there).
- Domain: the five rows of RFC-0002 §Continuation scanner's table, plus the
  Kani bound above.
- Artefact: `crates/whitaker_string_literals/src/classification/tests.rs`.
- Non-vacuity: each variant must be produced by at least one case, and the
  Kani assertion must be `exactly_one_of`, not `at_least_one_of`. Negative
  control **NV-CLASS-1**: swap the precedence so `TrailingLayoutTrim` wins the
  tie, and require the precedence case to fail.

**LEM-REWRITE-1 — splitting and re-concatenating equals removal.**

- Statement: let `body` be the literal body and `R` a sorted,
  pairwise-disjoint sequence of sub-ranges. Let `fragments` be the `|R| + 1`
  maximal sub-sequences of `body` lying outside `R`. Then
  `concat(fragments) == body with every range in R removed`.
- Method: Verus deductive proof over `Seq<u8>`, by induction on `|R|`,
  **inducting on the last range, not the first**. With `r_{n-1}` removed
  first, `body.subrange(0, r_{n-1}.start)` is untouched and no index-rebasing
  lemma is needed; inducting from the left would require one, and that is
  where these proofs stall in Z3.
- Rationale: this is the algebraic core of the suggestion, it must hold for
  every literal length and join count, and it is a pure statement about
  sequences. Note the honest limitation: Verus cannot verify the shipped
  `render(&str) -> String`, so this is a spec mirror, and the trust boundary is
  stated in the proof's module doc comment. It is the *supporting* argument;
  `LEM-REWRITE-2` and `LEM-EMIT-1` are what actually protect users.
- Artefact: `verus/string_continuation_rewrite.rs`, following
  `verus/clone_detector_candidate_pair.rs`.
- Evidence: `make verus`, which requires three edits to
  `scripts/run-verus.sh` — a new group arm in `proof_files_for_group`, an
  addition to the `all` arm, and a new case in the
  `all|decomposition|clone-detector)` dispatch. Before the proof body is
  written the lemma must fail with an open goal, not vacuously succeed.
- Non-vacuity: exhibit a concrete witness (`body = "abc"`, `R = [1..2]`,
  fragments `["a", "c"]`) inside the proof file as an `assert` Verus must
  discharge from the lemma. The proof must contain no `assume`. The
  `decreases` clause must be present and the base case must be `|R| == 0`.
  Negative control **NV-REWRITE-2**: weaken the disjointness precondition and
  confirm Verus rejects the proof.

**LEM-REWRITE-2 — the rewrite preserves the evaluated value.**

- Statement: unescaping the original body yields the same character sequence
  as unescaping each rewrite fragment and concatenating.
- Method: two independent layers. (a) A `proptest` differential against
  `rustc_lexer::unescape::unescape_str`. (b) The authoritative layer:
  `// run-rustfix` fixtures whose `.fixed` files carry
  `const _: () = assert!(bytes_eq(ORIGINAL, REWRITTEN));`. `SURP-014` shows
  `compiletest_rs` compiles the fixed code, so **rustc's own escaping
  implementation** adjudicates, with no shared lineage with our scanner.
- Rationale: layer (a) samples broadly with a same-family oracle; layer (b)
  is narrow but uses the real thing. Together they cover both "did we get the
  general case right" and "did we understand the rule at all".
- Domain: generated bodies containing Unicode above the basic multilingual
  plane, escaped quotes, escaped backslashes, `\u{...}` escapes, doubled
  braces, and one to five joins. Fixtures cover one case per scanner boundary.
- Artefact: `crates/whitaker_string_literals/src/rewrite/props.rs` and
  `crates/string_continuation_style/ui/fail_value_preserved.{rs,fixed,stderr}`.
- Non-vacuity: classify generated cases by join count and by whether any
  scalar above `U+FFFF` appears, and assert every class is hit with an explicit
  counter at the end of the run, not by eyeball. Negative control
  **NV-REWRITE-3**: make the rewriter split at a `LeadingLayoutTrim` as well as
  at joins, and require both layers to fail.

**LEM-APPLY-1 — the applicability gate is consulted and complete.**

- Statement: `ConcatRewrite::build` returns `Some` only when the original
  snippet lexes as exactly one cooked string token, the generated replacement
  lexes as exactly one `concat!` macro-call expression, every selected split is
  an interior join, and the context accepts a general expression. Re-lexing
  uses `rustc_lexer` (`SURP-011`).
- Method: `rstest` cases, one per failure condition, each failing exactly one
  conjunct with all others satisfied.
- Artefact: `crates/whitaker_string_literals/src/rewrite/tests.rs`.
- Non-vacuity: each condition must have a case that downgrades the result to
  `None`. Negative control **NV-APPLY-1**: hard-code `build` to return `Some`
  and require every case to fail. Note that the type shape of `DEC-003` makes
  this stronger than a field check: `PreferConcat(ConcatRewrite)` means, by
  construction, that a proven rewrite exists.

**LEM-FIX-1 — the rewrite is a fixed point.**

- Statement: applying the suggestion produces source on which the lint does
  not fire again.
- Method: free, from `SURP-014` — `compiletest_rs` fails the test if the
  recompiled fixed code "is still producing diagnostics". Every `fail_*`
  fixture therefore proves idempotence at no extra cost.
- Rationale: `cargo fix` re-runs the lint up to four rounds and reports an
  error blaming the user's code if it does not converge. Worth pinning the
  awkward shapes explicitly: a literal with both a leading layout trim and an
  interior join, and a literal with two adjacent continuations.
- Non-vacuity: **NV-FIX-1** — make the rewriter emit a fragment that itself
  contains an interior join, and require the recompile step to report residual
  diagnostics.

### Adapter obligations

These are the ones the first draft omitted, and they guard the twelve lines
that turn a byte offset into a write.

**LEM-SPAN-1 — body offsets map to source spans correctly.**

- Statement: `body_span(token_span, BodyOffset(o))` equals
  `token_span.from_inner(InnerSpan::new(o + prefix_len, ...))` where
  `prefix_len` is the literal's quote-prefix length, and the conversion is the
  only place that arithmetic occurs.
- Method: `BodyOffset` and `TokenOffset` newtypes make the shift a
  compiler-checked conversion rather than a remembered `+ 1`; in-process
  `rstest` cases in `crates/string_continuation_style/src/driver/tests.rs`
  drive the conversion directly; and the byte-exact `.stderr` caret positions
  pin it end to end.
- Rationale: this is the single place an off-by-one corrupts source. The
  `+ 1` convention is only sound because raw, byte, and C literals are excluded
  by construction (`prefix_len` is 2 for `b"…"` and `c"…"`), so the two type
  fixes reinforce each other.
- Non-vacuity: negative control **NV-REWRITE-1** — seed a deliberate
  off-by-one in the conversion and require both the unit cases and at least one
  `.stderr` caret to move.
- Note on coverage: `make coverage` instruments the adapter into the test
  binary but the adapter only executes inside a separate `rustc` subprocess, so
  without in-process tests these lines report as zero per cent and the
  changed-line gate blocks the pull request. Three crates already solve this
  with `src/driver*/tests.rs`; follow them.

**LEM-DEPTH-1 — wrapper depth is maintained across every containing node.**

- Statement: for every abstract-syntax-tree node kind that can contain an
  expression and that this pass visits, entering an expansion-derived node
  increments the counter and leaving it decrements; the counter is zero exactly
  when no enclosing node came from an expansion.
- Method: in-process `rstest` cases over a synthetic visit sequence, plus one
  UI fixture per containing kind: a literal inside a `macro_rules!`-expanded
  `let`, inside an expanded item, inside an expanded statement, inside a
  derive-generated impl, and inside a nested two-level expansion. All must be
  silent.
- Rationale: `SURP-012` makes this the load-bearing soundness gate, and it is
  the one part of the design the pure-domain verification cannot reach.
- Non-vacuity: negative control **NV-DEPTH-1** — disable the counter and
  require `pass_literal_macro_capture.rs` to emit a diagnostic. This is the
  control that proves the whole class of `pass_*` fixtures is not vacuous.

**LEM-EMIT-1 — no suggestion is emitted without a run-time value check.**

- Statement: before attaching an applicability above `MaybeIncorrect`, the
  lint unescapes the recovered original snippet and the generated replacement's
  fragments with `rustc_lexer::unescape` and compares the resulting byte
  sequences. On mismatch it emits nothing.
- Method: in-process `rstest` cases feeding the emission path a deliberately
  corrupted rewrite and asserting silence.
- Rationale: this is the highest-value single item in the plan. It converts
  every span-arithmetic bug — including ones no test anticipated — from silent
  source corruption into a silent no-op. Cost is a few microseconds on a path
  that runs approximately never, because `DEC-015`'s cheap pre-filter has
  already eliminated almost every literal.
- Non-vacuity: negative control **NV-EMIT-1** — seed an off-by-one span and
  require the gate to suppress the diagnostic, with the `.stderr` showing
  nothing rather than showing a wrong suggestion.

### Silence fixtures need controls too

Roughly a dozen `pass_*` fixtures assert that the lint emits nothing. Against
an unimplemented `check_expr` every one of them passes, so as specified they
prove nothing. Each therefore gets a paired control: temporarily invert the
one gate it exercises — drop the `Captured` check for
`pass_implicit_format_capture`, accept `StrRaw` for `pass_raw_strings`, disable
the depth counter for `pass_literal_macro_capture`, and so on — and record that
the fixture then fails with a diagnostic. Run each control once, record the
transcript in §Artefacts and notes, and revert it.

### Deliberately not verified formally

Nothing in this plan's scope is a policy table over third-party macro
grammars: `DEC-002` removed the allowlist and `DEC-009` moved the facades to
roadmap item 2.4.2. When 2.4.2 is planned it will need such a table, and it
should record there — as the first draft of this plan did — why the allowlist
gets acceptance fixtures rather than a proof: its correctness is a claim about
third-party crates' macro definitions, which change between their versions,
so the safe default of "an uncertain contract produces neither a diagnostic
nor a suggestion" carries the guarantee instead.

## Plan of work

### Stage A — understand and propose (no code changes)

Read RFC-0002 in full, then §Surprises & discoveries here. `DEC-009`,
`DEC-013`, and `DEC-014` are settled, so no decision blocks the work. Do not
write code until this plan is approved as a whole.

### Stage B — red tests and the behaviour specification

Write the feature file and the failing tests before any production code. The
specification driving `EP-M1` and `EP-M2`, at
`crates/whitaker_string_literals/tests/features/string_continuation.feature`:

```gherkin
Feature: Context-sensitive string continuation style

  Scenario: An ordinary source-line join prefers concat
    Given a cooked string expression with an interior continuation
    And the literal is in a general expression context
    When the continuation is classified
    Then the disposition is PreferConcat

  Scenario: An implicit format capture requires the direct literal
    Given a source-authored format string with an interior continuation
    And rustc classified one argument as Captured
    When the continuation is classified
    Then the disposition is RequireContinuation with reason ImplicitFormatCapture

  Scenario: A leading continuation expresses source layout
    Given a cooked string whose first token after the opening quote is a continuation
    When the continuation is classified
    Then the disposition is RequireContinuation with reason LeadingOrTrailingLayoutTrim

  Scenario: A byte string keeps its continuation
    Given a cooked byte string expression with an interior continuation
    When the continuation is classified
    Then the disposition is RequireContinuation with reason NonStringLiteralType

  Scenario: A raw string is not the lint's business
    Given a raw string expression containing a backslash before a newline
    When the continuation is classified
    Then the disposition is Ignore

  Scenario: Several joins produce one rewrite
    Given a cooked string expression with three interior continuations
    When the continuation is classified
    Then the disposition is PreferConcat
    And the rewrite has four fragments

  Scenario: A rewrite that would change the value is withheld
    Given a rewrite whose fragments do not unescape to the original value
    When the emission gate runs
    Then no suggestion is produced
```

Each stage ends with `make check-fmt && make typecheck && make lint &&
make test`, run sequentially. Do not proceed while any gate fails.

### Stage C — implementation with verification developed alongside

`EP-M0a` through `EP-M3`, in order. Each writes its Kani harness or Verus
lemma in the same commit as the code it constrains.

### Stage D — integration, tooling, documentation, and wider validation

`EP-M5`.

## Milestones and plateaus

### EP-M0a — Expansion-shape probe (prototyping, boxed at two hours)

- **Outcome.** A transcript answering the questions `DEC-002` and the axioms
  depend on. No repository change except this plan's living sections.
- **Method.** Three compiler flags, no code. On a scratch file outside the
  workspace containing `format!`, `println!`, `write!`, `assert!`, a
  `macro_rules!` capturing `$l:literal`, and a local `macro_rules! info`.
  Add `log::info!` and `tracing::info!` too: they are out of scope here, but
  the probe is the cheapest place to gather the evidence roadmap item 2.4.2
  will need, and recording it now costs one extra line per flag:
  - `cargo rustc -- -Zunpretty=expanded` — does each form reach the abstract
    syntax tree as a `format_args!` node?
  - `cargo rustc -- -Zunpretty=expanded,hygiene` — what syntax context does the
    message literal carry, and does a `$l:literal`-captured literal differ
    observably from a directly written one? This is the direct probe for
    `SURP-012`.
  - `-Zmacro-backtrace` on a deliberately erroring literal — renders the
    expansion chain for free.
- **Acceptance evidence.** `EV-M0-transcript` in §Artefacts and notes, with one
  line per probe, plus a statement for each of `AXIOM-1` through `AXIOM-3` and
  `AXIOM-7` recording whether the observed behaviour matches.
- **Go/no-go.** If `SURP-012` reproduces — a `$l:literal`-captured literal
  presents with a root syntax context — `DEC-002`'s wrapper-depth gate is
  mandatory and `EP-M2` proceeds as written. If it does not reproduce, record
  that in §Surprises, simplify the gate accordingly, and note that RFC-0002
  §Ordinary string expressions step 1 needs no amendment after all.
- **Conformance check.** Does any observation falsify an axiom? If so, amend
  the RFC before `EP-M1`.
- **Recovery.** Delete the scratch file. It lives outside `crates/`
  specifically so the `crates/*` workspace glob does not sweep it into
  `make lint` and `make test` while the probe runs. Do not run
  `git checkout -- .`; that would discard the plan updates this milestone
  exists to produce.
- **Remaining gaps.** All implementation.
- **Compatibility decision.** None; nothing ships.

### EP-M1 — Pure domain crate

- **Outcome.** `crates/whitaker_string_literals/` exists as a plain library
  with no rustc dependency. The three pure modules — `continuation`,
  `classification`, `rewrite` — plus the `facts` value types are complete,
  documented with runnable doctests, and fully covered. Nothing consumes it
  yet.
- **Requirements discharged.** RFC-0002 §Continuation scanner (as corrected by
  `SURP-016`), §Rewrite construction. `INV-SCAN-1`, `INV-SCAN-2`, `INV-SCAN-3`,
  `INV-CLASS-1`, `LEM-REWRITE-1`, `LEM-REWRITE-2` layer (a), `LEM-APPLY-1`.
- **Acceptance evidence.** `EV-M1`:
  `cargo nextest run -p whitaker_string_literals` passes;
  `cargo test -p whitaker_string_literals --doc` passes;
  `scripts/run-kani.sh string-literals` discharges the harness with the bound
  recorded; `make verus` discharges `verus/string_continuation_rewrite.rs`;
  every negative control in the pure set has been run and observed to fail for
  the stated reason.
- **Conformance check.** `cargo tree -p whitaker_string_literals` shows no
  `rustc_*` proxy crate. Every module is under 400 lines, now gate-enforced via
  `WHITAKER_PACKAGES`. No `#[expect]` of a panic-class lint anywhere.
- **Recovery.** The crate is unconsumed; deleting the directory and reverting
  the workspace manifest and `WHITAKER_PACKAGES` returns the tree to its prior
  state.
- **Remaining gaps.** No abstract syntax tree is inspected; no diagnostic is
  emitted.
- **Compatibility decision.** None. New pre-1.0 crate, no consumers.

### EP-M2 — Plain cooked string literals

- **Outcome.** `crates/string_continuation_style/` exists as a `cdylib`
  adapter. `check_expr` handles `ExprKind::Lit` with a cooked
  `token::LitKind::Str` behind `DEC-015`'s cheap-first ordering and
  `DEC-002`'s wrapper-depth gate. A plain source-line join produces one
  warning with a note, a help, one secondary label per join, and a
  whole-literal `concat!()` suggestion whose applicability follows `DEC-013`.
  Byte strings, C strings, raw strings, layout trims, macro-captured literals,
  and generated source files produce silence. Diagnostics are localized in
  `en-GB`, `cy`, and `gd`.
- **Requirements discharged.** RFC-0002 §Ordinary string expressions,
  §Diagnostic, and the plain-literal rows of §Decision matrix. `LEM-SPAN-1`,
  `LEM-DEPTH-1`, `LEM-EMIT-1`, `LEM-FIX-1`, `LEM-REWRITE-2` layer (b).
- **Go/no-go before any suggestion ships.** Prove that `// run-rustfix` works
  end to end through `dylint_testing` on the first fixture. If it does not,
  that is a hard blocker on any applicability above `MaybeIncorrect`, not a
  footnote: escalate rather than shipping an unverified fixer.
- **Acceptance evidence.** `EV-M2`: fixtures `fail_plain_join`,
  `fail_multiple_joins`, `fail_join_across_blank_lines`,
  `fail_value_preserved`, `fail_trim_and_join`, `fail_adjacent_joins`,
  `pass_leading_layout_trim`, `pass_trailing_layout_trim`,
  `pass_byte_and_c_strings`, `pass_raw_strings`, `pass_real_newline`,
  `pass_literal_in_pattern`, `pass_literal_macro_capture`,
  `pass_expanded_item`, `pass_generated_source_file` all pass, each `fail_*`
  with a `.fixed` file under `// run-rustfix`; `ui-cy`, `ui-gd`, and
  `ui-fallback` locale smokes pass; every silence fixture's paired control has
  been run and observed to fail. `EV-M2-cost`: `time cargo dylint --lib
  string_continuation_style` over one medium crate, before and after.
- **Conformance check.** Applying `fail_plain_join`'s suggestion produces
  source that compiles and evaluates identically — now verified by the harness
  rather than by hand. No lint fires on any pattern, attribute, or meta-item
  literal.
- **Recovery.** Revert `check_expr` to a no-op; the crate returns to a
  coherent silent state.
- **Remaining gaps.** Format strings are not inspected.
- **Compatibility decision.** None.

### EP-M3 — Source-authored format strings from compiler built-ins

- **Outcome.** `check_expr` also handles `ExprKind::FormatArgs`. Format strings
  from the built-in set in `DEC-002` are diagnosed when the format string is a
  source literal, contains no `FormatArgumentKind::Captured` argument, wrapper
  depth is zero, the outermost expansion frame's name is in the set, and the
  crate shadows none of those names.
- **Requirements discharged.** RFC-0002 §Format strings, §Why the motivating
  example passes, and the format-string rows of §Decision matrix.
- **Acceptance evidence.** `EV-M3`: `fail_positional_arguments`,
  `fail_explicit_format_arguments`, `fail_println_join`, `fail_write_join`,
  `fail_assert_message_join`, `pass_implicit_format_capture` (the exact
  pull-request-296 example, which must remain clean),
  `pass_implicit_width_capture`, `pass_implicit_precision_capture`,
  `pass_generated_format_string`, `pass_shadowed_println` (a crate defining its
  own `println!`, which must disable the lint entirely) all pass. The RFC's
  "must fail with a machine-applicable rewrite" example produces exactly the
  RFC's expected replacement, pinned by an `insta` snapshot of the *pure*
  generated `concat!` text — a second, independently blessable artefact, so
  that drift requires two files to be blessed in agreement.
- **Conformance check.** The capture predicate is
  `any(|a| matches!(a.kind, Captured(_)))` over `arguments.all_args()`,
  catching captured width and precision. `log::info!` and `tracing::info!`
  fixtures, if present, are silent.
- **Recovery.** Remove the `FormatArgs` arm; `EP-M2` behaviour is intact.
- **Remaining gaps.** Integration.
- **Compatibility decision.** None.

### EP-M5 — Suite, installer, tooling, and documentation integration

- **Outcome.** `whitaker_suite` registers the early pass alongside its combined
  late pass. `cargo dylint list` shows the lint in both libraries. Proofs run
  in continuous integration. Documentation is complete and the roadmap item is
  ticked.
- **Requirements discharged.** RFC-0002 §Suite integration; AGENTS
  §Documentation maintenance; ROADMAP-2.4.1; `DEC-011`, `DEC-012`.
- **Acceptance evidence.** `EV-M5`: `suite/tests/registration.rs` passes,
  including a new `then_early_pass_registered` step that asserts *which* pass
  is registered, not merely how many — a count would also pass if the wrong
  pass were registered, and unlike a late pass omitted from
  `late_lint_methods!`, an omitted `register_early_pass` line is not a compile
  error. `make publish-check` succeeds. `make markdownlint` and `make nixie`
  pass. `cargo nextest show-config test-groups --profile ci` shows the new
  crate's UI test in `serial-dylint-ui`. The new `proofs` job passes.
- **Conformance check.** Both `LINT_CRATES` lists agree. `SUITE_LINTS` and
  `SUITE_LINT_DECLS` are index-aligned. Every discovery is reconciled with
  RFC-0002. `bumpy_road_function`'s fixtures are unchanged after the `DEC-012`
  migration.
- **Recovery.** Revert the suite and installer edits; the standalone lint crate
  remains usable.
- **Remaining gaps.** None. Plan closes.
- **Compatibility decision.** None. Whitaker is at `0.2.7` and the suite's lint
  list is not an external commitment.

## Concrete steps

All commands run from the repository root on branch
`2-4-1-string-continuation-style-post-expansion-early-lint.md`.

Log every gate run so truncated output can be reviewed:

```bash
make test 2>&1 | tee "/tmp/test-whitaker-$(git branch --show-current).out"
```

### Step 0 — baseline measurement

Before any change:

```bash
time make test 2>&1 | tee "/tmp/baseline-whitaker-$(git branch --show-current).out"
```

Record the wall-clock as `EV-BASELINE` in §Artefacts and notes. The
`Tolerances` wall-clock trigger is relative to this number.

### Step 1 — workspace changes

Edit `Cargo.toml`:

- `[workspace.dependencies]`: add `googletest`, `pretty_assertions`, and
  `serial_test = "4.0.1"` (matching the four existing per-crate pins exactly —
  do not write `"3"`, which would silently downgrade them). Record the resolved
  `googletest` version here once `cargo add --dry-run` reports it.
- Add `[profile.release] overflow-checks = true`.
- Migrate all **four** crates pinning `serial_test` locally —
  `conditional_max_n_branches`, `function_attrs_follow_docs`,
  `module_must_have_inner_docs`, `no_std_fs_operations` — to
  `serial_test = { workspace = true }`.

Edit `Makefile`: add `whitaker_string_literals` to `WHITAKER_PACKAGES` so the
suite lints the new pure crate and the 400-line ceiling is gate-enforced there.

Expected:

```plaintext
$ cargo metadata --format-version 1 >/dev/null && echo ok
ok
```

Commit: `Add test-assertion dependencies and enable release overflow checks`.

### Step 2 — scaffold both crates

`crates/whitaker_string_literals/` (plain library, copy
`crates/whitaker_clones_core/Cargo.toml`'s shape):

```plaintext
crates/whitaker_string_literals/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── facts.rs
│   ├── continuation/{mod.rs,kani.rs,props.rs,tests.rs}
│   ├── classification/{mod.rs,tests.rs}
│   └── rewrite/{mod.rs,props.rs,tests.rs}
└── tests/
    ├── features/string_continuation.feature
    └── string_continuation_behaviour.rs
```

`crates/string_continuation_style/` (cdylib adapter, copy
`crates/no_unwrap_or_else_panic/Cargo.toml`'s shape):

```plaintext
crates/string_continuation_style/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── diagnostics.rs
│   ├── driver/{mod.rs,depth.rs,facts.rs,span.rs,tests.rs}
│   ├── lib_ui_tests.rs
│   └── tests/{mod.rs,ui.rs}
├── ui/    ui-cy/    ui-gd/    ui-fallback/
```

`src/driver/tests.rs` is not optional: without in-process driver tests the
adapter reports zero per cent under `make coverage`, which runs on every pull
request against changed lines. Three crates already solve this the same way.

Verify:

```plaintext
$ cargo check -p whitaker_string_literals
    Finished `dev` profile
$ cargo check -p string_continuation_style --features dylint-driver
    Finished `dev` profile
```

Commit: `Scaffold the string-literal domain and lint crates`.

### Step 3 — EP-M1, red first

Write `crates/whitaker_string_literals/src/continuation/tests.rs` with the
`INV-SCAN-1` boundary cases before writing the scanner. Confirm the red:

```plaintext
$ cargo nextest run -p whitaker_string_literals continuation::
error[E0433]: failed to resolve: use of undeclared crate or module `continuation`
```

Then implement, and confirm green. Repeat for `classification` and `rewrite`.

Add the Kani harness and the Verus proof in the same commits as the code they
constrain, together with their runner edits:

- `scripts/run-kani.sh`: a new `run_string_literal_harnesses()` with an
  explicit harness-name list and
  `--manifest-path "${REPO_ROOT}/crates/whitaker_string_literals/Cargo.toml"`,
  a new `string-literals)` case arm, **and** a call in the no-argument path at
  lines 83-87 so `make kani` picks it up. Note the existing catch-all `*)`
  arm falls through to `run_decomposition_harnesses`, which would `cd common`
  and silently match nothing — an unknown group does not error.
- `Makefile`: a `kani-string-literals` target beside `kani-clone-detector`,
  and a `verus-string-literals` target beside `verus-clone-detector`.
- `scripts/run-verus.sh`: three edits — the new group arm in
  `proof_files_for_group`, the addition to the `all` arm, and the new case in
  the `all|decomposition|clone-detector)` dispatch.

Run each pure negative control once, record the observed failure in §Artefacts
and notes, then revert it.

### Step 4 — EP-M2 and EP-M3 fixtures

For each `fail_*` fixture: write the `.rs` file, add `// run-rustfix` as the
first line, write the `.fixed` file by hand with a
`const _: () = assert!(...)` value assertion, run the UI test, and create the
`.stderr` from the observed output.

**Blessing is manual.** `dylint_testing` gives `compiletest_rs` a default
config with `bless: false` and exposes no override; `TRYBUILD=overwrite` does
nothing. The report prints a line of the form `Actual stderr saved to PATH`;
copy that file over your `.stderr`. Read it first. A blessed-but-wrong
expectation is worse than a failing test, and the toolchain-bump scenario is
exactly where that discipline fails under time pressure — which is why
`EP-M3`'s `insta` snapshot exists as a second, independently blessable
artefact.

```bash
cargo nextest run -p string_continuation_style ui 2>&1 \
  | tee "/tmp/ui-whitaker-$(git branch --show-current).out"
```

For each `pass_*` fixture, run its paired gate-inversion control once, record
the failure, and revert.

### Step 5 — integration

Edit, in this order:

1. `suite/Cargo.toml` — add `dep:string_continuation_style` to the
   `dylint-driver` feature and the path dependency with
   `features = ["dylint-driver", "constituent"]`.
2. `suite/src/lints.rs` — append to `SUITE_LINTS` and `SUITE_LINT_DECLS` at the
   same index; extend the doctest's expected name list. Give `LintDescriptor`
   a `pass: PassKind { Early, Late }` field so the early-pass assertion in
   step 4 below derives from the same source rather than from a literal.
3. `suite/src/driver.rs` — insert `store.register_early_pass(...)` into
   `register_suite_lints`, between `register_lints` and `register_late_pass`.
   Update the doctest's `get_lints().len()` from 9 to 10.
4. `suite/tests/features/suite_registration.feature` and
   `suite/tests/registration.rs` — add a scenario and a
   `then_early_pass_registered` step asserting which pass is registered.
5. `installer/src/resolution.rs` — add `"string_continuation_style"` to
   `LINT_CRATES`.
6. `Makefile` — add `string_continuation_style` to the `LINT_CRATES` variable.
7. `.config/nextest.toml` — confirm the new crate's UI test matches the
   `serial-dylint-ui` filter (the `src/tests/ui.rs` layout yields
   `tests::ui::ui`, which the `test(tests::ui::)` clause catches) and add a
   crate-specific `slow-timeout` override. Record
   `cargo nextest show-config test-groups --profile ci` as evidence.
8. `.github/workflows/ci.yml` — add the `proofs` job of `DEC-011`.
9. `common/src/i18n/helpers.rs` and `common/src/i18n/diagnostics.rs` — add
   `safe_resolve_label` and `normalize_fluent_output` per `DEC-012`; migrate
   `bumpy_road_function`.
10. `common/tests/i18n_quality/suite.rs` — generalize
    `localized_help_attributes_are_complete`, which today checks only `.help`,
    to assert that every attribute present on an en-GB message is present and
    non-empty in every secondary locale. Without this, a missing `.label`
    translation renders English, gets blessed into a `ui-cy` golden, and the
    fixture then enforces the bug. This retroactively protects
    `bumpy_road_function` too.
11. `common/locales/{en-GB,cy,gd}/string_continuation_style.ftl` — the message,
    `.note`, `.help`, and `.label`. Decide the `.label` argument shape now:
    adding a `{ $ordinal }` placeable later is a breaking change to three
    Fluent files, the placeable-parity test, and every golden at once. Put a
    one-line comment at the head of each file naming the dependent fixture
    directories.
12. `README.md` — the lint table row and the "ships nine standard lints" count.
13. `docs/users-guide.md` — a `### string_continuation_style` subsection
    covering what fires, what does not, the `DEC-013` configuration table, and
    `RUST_LOG=string_continuation_style=debug` for asking the lint why.
14. `docs/developers-guide.md` — a section on writing early passes here
    (`EarlyContext` has no `TyCtxt`; the `is_source_literal` guarantee; the
    wrapper-depth pattern and why `is_root()` is insufficient; locale
    serialization), **and** an addition to the toolchain-bump runbook naming
    this crate: which `rustc_ast` items are load-bearing
    (`FormatArgs::{span, is_source_literal, uncooked_fmt_str}`,
    `FormatArgumentKind::Captured`), that the `EP-M0a` probe must be re-run and
    `EV-M0-transcript` re-recorded on every bump, and that a change in
    `is_source_literal` semantics silently converts firing cases into silence —
    the failure direction no test catches.
15. `docs/whitaker-dylint-suite-design.md` — an addendum on early-pass
    registration.
16. `docs/adr-005-early-lint-pass-position-proof.md` — a new Architectural
    Decision Record for `DEC-002`, written in the Y-Statement form of the
    existing ADRs. This is the repository's first `EarlyLintPass`, the decision
    weighs four architectures, it is hard to reverse because every later early
    pass inherits it, and `AGENTS.md` requires an ADR for substantive
    decisions. Reference it from `docs/whitaker-dylint-suite-design.md`.
17. `docs/contents.md` — index the new ADR; it lists all four existing ones.
18. `docs/repository-layout.md` — note the new pure-domain crate.
19. `docs/rfcs/0002-string-continuation-style.md` — the substantive revision
    described in §Outcomes & retrospective; move status to *Accepted*.
20. `docs/roadmap.md` — tick item 2.4.1. Items 2.4.2 and 2.4.3 already exist
    and stay open.

Then:

```plaintext
$ make publish-check 2>&1 | tee "/tmp/publish-check-whitaker-$(git branch --show-current).out"
...
$ cargo dylint list | grep string_continuation_style
string_continuation_style
```

## Validation and acceptance

Run all four gates after every milestone, sequentially — the build cache makes
sequential runs faster overall:

```bash
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-whitaker-$(git branch --show-current).out"
make lint      2>&1 | tee "/tmp/lint-whitaker-$(git branch --show-current).out"
make test      2>&1 | tee "/tmp/test-whitaker-$(git branch --show-current).out"
```

Delegate full gate runs to the `scrutineer` subagent. When it reports a
failure, read the cited log rather than re-running the gate.

Red-Green-Refactor evidence per domain module:

- **Red.** `cargo nextest run -p whitaker_string_literals <module>::` fails
  with an unresolved-item or assertion error naming the missing behaviour.
- **Green.** The same command passes after the minimal implementation.
- **Refactor.** The same command, then the four gates, all pass after cleanup.

Behaviour-driven evidence:

- **Red.** `cargo nextest run -p whitaker_string_literals behaviour` fails
  because the step definitions are unimplemented.
- **Green.** The same command passes; each scenario maps to one `#[scenario]`.

Verification evidence:

- `scripts/run-kani.sh string-literals` reports `VERIFICATION:- SUCCESSFUL`,
  with the explored bound recorded and the excluded alphabet stated.
- `make verus` reports `verification results:: N verified, 0 errors` for
  `verus/string_continuation_rewrite.rs`.
- Every `NV-*` control has been run and its failure transcript recorded.

Quality criteria — what "done" means:

- **Tests.** `make test` passes with the whole workspace green.
- **Verification.** All pure and adapter obligations discharged by their named
  artefacts, with non-vacuity controls observed.
- **Fixer.** Every `fail_*` fixture applies, recompiles, and produces no
  residual diagnostics under `// run-rustfix`.
- **Lint and typecheck.** `make check-fmt`, `make typecheck`, `make lint` exit
  zero. Whitaker lints the new pure crate clean, `module_max_lines` included.
- **Documentation.** `make markdownlint` and `make nixie` pass.
- **Performance.** `EV-M2-cost` recorded; `make test` within the `Tolerances`
  growth budget relative to `EV-BASELINE`.
- **Security.** None applicable.

Quality method: `scrutineer` runs the gates; request a CodeRabbit review
through it before marking the plan complete, and action every finding.

## Idempotence and recovery

Every step is re-runnable. UI-fixture blessing overwrites `.stderr` files in
place, so review the diff before committing; `git checkout -- crates/…/ui/`
restores the committed expectations.

Commit after each milestone and after each numbered edit in Step 5, so that
`git bisect` and time-travel review both work. Nothing writes outside the
repository except disposable log files under `/tmp`.

Each milestone's Recovery entry names the single revert that returns the tree
to the previous plateau.

## Artefacts and notes

To be filled in as work proceeds. Required entries:

- `EV-BASELINE` — `make test` wall-clock before Step 1.
- `EV-M0-transcript` — the `EP-M0a` probe output, with a per-axiom statement.
- The observed failure of each `NV-*` control, including one per silence
  fixture.
- `EV-M2-cost` — `cargo dylint` timing over one medium crate, before and after.
- The `insta` snapshot of the RFC's motivating-example rewrite.
- `cargo nextest show-config test-groups --profile ci` output.
- The `make publish-check` tail showing the lint in `cargo dylint list`.

## Interfaces and dependencies

### Domain — `crates/whitaker_string_literals` (no rustc imports)

The types make illegal states unrepresentable rather than relying on tests to
notice them. Note in particular that the offset origin is stated once, in the
`LiteralBody` doc comment, and never re-derived.

In `src/facts.rs`:

```rust
/// The raw source bytes of a cooked literal, quotes excluded.
///
/// Every offset produced by this module is relative to the start of this
/// slice, never to the enclosing literal token. The adapter converts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiteralBody<'a>(&'a str);

/// A byte offset into a [`LiteralBody`].
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BodyOffset(usize);

/// What the adapter observed about one literal.
///
/// Raw, byte, and C literals are excluded by construction: the scanner is
/// only reachable through the `Cooked` variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteralFacts<'a> {
    Cooked { body: LiteralBody<'a>, context: LiteralContext },
    /// `b"..."` or `c"..."`. Carries no body; nothing to scan.
    NonStringLiteral,
    /// `r"..."`, `br"..."`, `cr"..."`. Continuations are not interpreted.
    Raw,
}

/// The grammatical position of a cooked literal. Adapter capability is not
/// modelled here: when the adapter cannot do its job it declines by
/// returning `None` rather than encoding its own failure as a context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteralContext {
    Expression,
    SourceFormatString { capture: CaptureStatus },
    GeneratedFormatString,
}

/// Whether any format argument was implicitly captured. Not a `bool`: this
/// is the one bit separating "emit a machine-applicable fix" from "emit
/// nothing", and it is computed by the adapter from
/// `FormatArgumentKind::Captured`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureStatus { NoneObserved, Present }
```

In `src/continuation/mod.rs`:

```rust
pub struct Continuation {
    pub escape_range: core::ops::Range<BodyOffset>,
    pub skipped_whitespace_range: core::ops::Range<BodyOffset>,
    pub kind: ContinuationKind,
}

pub enum ContinuationKind { Join, LeadingLayoutTrim, TrailingLayoutTrim }

impl<'a> LiteralBody<'a> {
    /// Scans for continuation escapes. Total: never panics, never slices.
    pub fn scan(self) -> Vec<Continuation>;
}
```

In `src/classification/mod.rs`:

```rust
pub enum ContinuationDisposition<'a> {
    /// A proven rewrite. Construction of `ConcatRewrite` is the proof.
    PreferConcat(ConcatRewrite<'a>),
    RequireContinuation(RequiredReason),
    Ignore,
}

/// Only reasons the decision matrix can actually produce. RFC-0002's
/// `GeneratedFormatString` and `UnrecoverableSourceSpan` variants are absent:
/// the matrix routes both to `Ignore`, so as `RequireContinuation` variants
/// they were unreachable, and two encodings of one outcome cannot be
/// distinguished end to end because both produce silence.
pub enum RequiredReason {
    ImplicitFormatCapture,
    NonStringLiteralType,
    LeadingOrTrailingLayoutTrim,
    GeneratedSourceFile,
}

/// The single policy entry point. Pure: same facts in, same disposition out.
pub fn classify<'a>(facts: &LiteralFacts<'a>) -> ContinuationDisposition<'a>;
```

In `src/rewrite/mod.rs`:

```rust
/// A rewrite that has passed the applicability gate.
///
/// Fields are private and there is exactly one constructor, so
/// `fragments.len() == join_offsets.len() + 1` holds by construction rather
/// than by assertion, and the lifetime ties the fragments to the body they
/// came from so they cannot be rendered against a different literal.
pub struct ConcatRewrite<'a> {
    fragments: Vec<&'a str>,
    join_offsets: Vec<BodyOffset>,
}

impl<'a> ConcatRewrite<'a> {
    /// The only constructor. `None` when the applicability gate fails.
    pub fn build(body: LiteralBody<'a>, joins: &[Continuation]) -> Option<Self>;

    pub fn fragments(&self) -> &[&'a str];
    /// Byte offsets of each selected join's backslash, for secondary labels.
    pub fn join_offsets(&self) -> &[BodyOffset];

    /// Renders the replacement text at the given source indentation.
    pub fn render(&self, indent: usize, newline: Newline) -> String;
}

/// The line ending to emit in a multi-line replacement. Detected by the
/// adapter from the source file's dominant ending; emitting LF into a CRLF
/// checkout produces mixed endings inside the hunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Newline { Lf, CrLf }
```

There is deliberately no `RewriteApplicability` enum. The domain never
produces anything but a proven rewrite, so a two-variant enum with one
reachable variant would carry no information; `DEC-013`'s configuration decides
the rustc `Applicability` at the single `span_suggestion` call site.

`render` takes an indentation because a replacement that ignores the original
literal's column breaks `cargo fmt --check` after every fix run — which, for a
rollout plan whose step three is "apply machine fixes across repositories",
means every fixed repository needs a follow-up formatting commit. Prefer a
single-line `concat!("alpha ", "beta")` when it fits within the formatter's
width, and fall back to the indented multi-line form otherwise.

### Adapter — `crates/string_continuation_style` (`dylint-driver` only)

- `src/driver/mod.rs` — the `EarlyLintPass`, config and localizer lifecycle
  (`check_crate`, per `DEC-016`), and dispatch. The lint declaration lives in a
  private `declaration` module per the `conditional_max_n_branches` pattern.
- `src/driver/depth.rs` — the wrapper-depth counter of `DEC-002`, the built-in
  name set, and the crate-scoped shadow kill switch.
- `src/driver/facts.rs` — construction of `LiteralFacts` from `ExprKind::Lit`
  and `ExprKind::FormatArgs`, under `DEC-015`'s cheap-first ordering.
- `src/driver/span.rs` — the sole owner of the offset conversion:

  ```rust
  /// Byte offset relative to a literal token, quotes included.
  struct TokenOffset(u32);

  /// The only place body-relative offsets become token-relative. `prefix_len`
  /// is 1 for `"…"`; the `b"…"`/`c"…"` cases never reach here because
  /// `LiteralFacts::NonStringLiteral` excludes them.
  fn to_token(offset: BodyOffset, prefix_len: u32) -> Option<TokenOffset>;

  /// Maps a body offset range onto a source span. Returns `None` on any
  /// arithmetic failure, which the caller maps to silence.
  fn body_span(token: Span, range: Range<BodyOffset>) -> Option<Span>;
  ```

- `src/diagnostics.rs` — converts a disposition plus recovered spans into a
  localized `cx.emit_span_lint` call, and runs `LEM-EMIT-1`'s value check
  before attaching any applicability. Emits
  `debug!(target: LINT_NAME, ...)` at two points: the classification boundary,
  recording the disposition and any `RequiredReason`, and the applicability
  gate, recording which condition withheld the suggestion. Those two lines turn
  every "why didn't it fire" report into self-service through
  `RUST_LOG=string_continuation_style=debug`.

### Dependencies

`crates/whitaker_string_literals`: `rustc_lexer` (workspace). Nothing else.
Dev-dependencies: `rstest`, `rstest-bdd`, `rstest-bdd-macros`, `proptest`,
`insta`, `googletest`, `pretty_assertions`, `whitaker_test_macros`.

`crates/string_continuation_style`, all `optional = true` under
`dylint-driver`: `dylint_linting`, `rustc_ast`, `rustc_lint`, `rustc_session`,
`rustc_span`, `fluent-templates`, `log`, `serde`, `whitaker-common`,
`whitaker` (with `features = ["dylint-driver"]`), and
`whitaker_string_literals`. Do not add `clippy_utils`; no lint here uses it.
Dev-dependencies: `whitaker_test_macros`, `whitaker-common`, `whitaker`,
`rstest`, `dylint_testing`, `googletest`, `pretty_assertions`, `serial_test`,
`camino`, `tempfile`.

Both crates set `[lints] workspace = true` and additionally deny
`arithmetic_side_effects`.

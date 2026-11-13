# Architectural decision record (ADR) 010: prove the lint's position, not its macro's identity

## Status

Accepted (2026-08-21): Implement `string_continuation_style` as a post-expansion
`EarlyLintPass` whose admission test is a position test — the literal is at
wrapper depth zero, or it arrived through a known compiler built-in macro, or
the crate is silent — rather than an attempt to identify which macro produced
it.

## Date

2026-08-21.

## Context and problem statement

`string_continuation_style` diagnoses a cooked string literal that contains a
source-line join, and suggests rewriting it as `concat!()`. Identifying the
literal is easy; deciding it is *safe* to rewrite is not.

The rewrite changes the shape of the source expression. If the literal is the
argument to a macro whose matcher expects one literal token, replacing it with a
`concat!()` invocation either fails to match or matches differently. The
motivating hazard is `$l:literal`: a macro that captures a literal passes it to
the expansion with a root syntax context, so `Span::ctxt().is_root()` — the
obvious test, and the one the first draft used — reports it as unexpanded. A
lint that trusted `is_root()` would emit a rewrite that does not compile.

So the lint must answer "is anything standing between this literal and the
compiler?" A tempting answer is "which macro produced it?", which leads to
identifying macro definitions, resolving their expansions, and consulting an
allowlist of trusted names. That answer needs `TyCtxt` or a macro-expansion
pass, and it still cannot see a macro reached by a path it did not enumerate.

A second constraint pushes in the same direction. Format strings are parsed into
`FormatArgs`, whose argument kinds distinguish implicit capture
(`FormatArgumentKind::Captured`) from positional and explicitly named
arguments. Rewriting a format string that uses implicit capture into
`concat!()` turns it from a format string into macro output and silently
disables the capture — a semantic change, not a formatting one. Only
post-expansion inspection of the parsed `FormatArgs` can see this, which rules
out a pre-expansion lint. A late (HIR) pass cannot see `ExprKind::FormatArgs`
at all, because format arguments are lowered away before HIR, which is the real
reason a late pass is disqualified (source spelling is recoverable at any phase
through `span_to_snippet`, contrary to an earlier draft of the RFC).

## Decision drivers

- A rewrite offered by a lint must compile.
- Near-zero false positives matter more than hit rate; silence is an
  acceptable outcome for an ambiguous literal.
- The pure domain half of the lint must be verifiable without a compiler.
- The solution should not depend on the internals of macros the lint does not
  own.
- The choice is hard to reverse: every later early lint inherits whatever
  registration and admission shape is established here.

## Options considered

### Where the lint runs

- **Pre-expansion early pass.** Cannot see `FormatArgumentKind::Captured`, so
  it cannot distinguish a safe format-string rewrite from one that breaks
  implicit capture.
- **Post-expansion early pass (`EarlyContext`).** Sees the expanded AST
  including parsed `FormatArgs`, and has `span_to_snippet` for recovery.
  Selected.
- **Late pass on HIR.** Never sees `ExprKind::FormatArgs`; format arguments are
  lowered away.
- **Two-pass registration sharing thread-local state.** Adds a correctness
  surface, mutable global state, and a registration the suite must keep in
  sync, for information a single pass already has.

### What counts as safe

- **Macro identity resolution.** Resolve the producing macro, compare it to an
  allowlist. Needs `TyCtxt`, is fragile against macro hygiene and facade
  macros, and fails open or closed depending on how the allowlist is reached.
- **Position test (wrapper depth).** Count the enclosing AST containers whose
  span is `from_expansion()`. Depth zero means nothing that could have matched
  the literal is standing between it and the compiler. Answerable from
  `EarlyContext` alone.
- **Root syntax context alone.** Insufficient: `$l:literal` passes through
  with a root context.

The position test is the adopted one. It answers exactly the question the lint
has, and it does so with information the pass already holds.

## Decision outcome / proposed direction

The lint runs as a post-expansion `EarlyLintPass` and admits a literal by one
of two position branches, both gated on a crate-wide shadow check. The branches
are alternatives, not simultaneous conditions: a plain literal is at wrapper
depth zero and never has a macro frame, while a format string always has a
facade chain and is never at depth zero. Requiring all three would suppress
every candidate on both paths. The implementation records the two kinds in
separate safe-span sets for exactly this reason.

1. **Plain literals: wrapper depth is zero.** The literal sits in no AST
   container whose span came from an expansion. The depth set is computed by a
   `rustc_ast::visit::Visitor` run from `check_crate`, with an explicit
   push/pop for every container kind, recording safe literal spans into a
   `FxHashSet` that the emitting hooks consult. `nightly-2026-05-28` has no
   `check_stmt_post`, `check_local`, `check_block`, `check_arm` or `check_ty`
   pairing, and the paired hooks alone do not cover the `$l:literal` capture
   case, which is why the set is built by a visitor rather than maintained by
   balanced hooks.

2. **Format strings: the outermost non-root expansion frame is a known
   formatting macro.** For `ExprKind::FormatArgs`, walk the literal span's
   expansion chain to its outermost frame — the invocation the user actually
   typed — and require `ExpnKind::Macro(MacroKind::Bang, sym)` with `sym` in a
   fixed set: `format_args`, `format`, `print`, `println`, `eprint`, `eprintln`,
   `write`, `writeln`, `panic`, `todo`, `unimplemented`, `unreachable`,
   `assert`, `debug_assert`, `assert_eq`, `assert_ne`, `debug_assert_eq`,
   `debug_assert_ne`. These are language built-ins with no macro-by-example
   matcher to break.

3. **The crate does not shadow any of those names, or `concat`.** A bare name
   check is sound only if the name cannot be shadowed. If the crate under
   compilation defines a `macro_rules!` with a name in the set or one equal to
   `concat`, or contains a `use` whose bound name is one, or a
   `#[macro_use] extern crate`, the lint disables itself for that crate.
   `concat` is included even though it is not a format macro, because the
   suggested replacement itself spells a bare `concat!()`. The bound name, not
   the terminal path segment, is what counts: `use module::println as shout`
   binds `shout` and cannot shadow `println!`.

   The `macro` item kind — declarative macros 2.0, spelled `macro name { … }`
   rather than `macro_rules! name` — is not yet part of the shadow scanner,
   which observes only `ItemKind::MacroDef`. A crate that shadows a format
   macro this way, without also defining a `macro_rules!` of the same name,
   would keep the lint enabled.

**Shared entry point.** Because the usual `LateLintPass` route cannot carry an
early lint, the suite registers this pass separately with
`store.register_early_pass`. That registration has no compile-time guard, so
the registration tests assert which pass was registered rather than how many.
`suite/src/lints.rs` records the phase per lint and `suite/src/driver.rs`
performs the registration, keeping the two sources of truth in different files.
See the early-pass registration addendum in the
[suite design](../whitaker-dylint-suite-design.md#early-pass-registration-addendum).

## Consequences

- The lint needs no `TyCtxt`, no `rustc_middle`, no thread-local reach-through,
  no shared storage between passes, and no allowlist table.
- `log` and `tracing` fail the outermost-frame check and produce silence by
  construction rather than by policy. Supporting them is a separate, later
  decision that must prove macro identity, format-string position, and the
  argument grammar, and it stays out of this release.
- The residual hole is a crate reaching a same-named macro by a path that
  neither the definition check nor the `use`-tree check observes, such as a
  path imported through a re-export chain the syntactic check does not follow.
  The failure mode is silence, so it is undetectable from inside the lint. This
  is accepted because the alternative — diagnosing a literal passed to a user
  macro whose matcher the rewrite could break — fails loudly in the wrong
  direction. Glob imports were originally accepted as a second hole; they are
  now disqualifying, because a glob-imported `concat` makes the suggested bare
  call ambiguous and fails the rewrite with E0659.
- The crate-wide shadow switch is blunt: it will silence the lint across a
  crate with no real conflict. Accepted for the same reason.
- Wrapper-depth accounting is a new correctness surface that the pure-domain
  verification cannot reach. It gets a lemma and its own fixture matrix rather
  than relying on the adapter's ordinary tests.
- Every later early lint inherits the registration shape and the
  descriptor/registration split.

## References

- [RFC 0002 `string_continuation_style`](../rfcs/rfc-0002-string-continuation-style.md).
- [ADR 007 Rust rewrite materialization substrate](adr-0007-rust-rewrite-materialization-substrate.md),
  which establishes the pure-domain half this decision leaves unverifiable in
  its adapter half.

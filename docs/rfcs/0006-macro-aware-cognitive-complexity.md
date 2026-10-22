# RFC 0006: Macro-aware cognitive complexity

## 1. Preamble

- **RFC number:** 0006
- **Status:** Proposed
- **Created:** 2026-10-22
- **Proposed lint:** `macro_aware_cognitive_complexity`
- **Default level:** `warn`
- **Suite status:** experimental, behind
  `experimental-macro-aware-cognitive-complexity`
- **Tracking issue:**
  [leynos/whitaker#504](https://github.com/leynos/whitaker/issues/504)

RFC numbers 0003 to 0005 are already allocated on unmerged branches, so this
RFC takes 0006 to avoid a collision.

## 2. Summary

This RFC proposes `macro_aware_cognitive_complexity`, a Whitaker late lint that
measures a function's cognitive complexity over the code its author wrote,
rather than over the code macro expansion produced. It keeps Clippy's
`cognitive_complexity` arithmetic so that existing thresholds and suppressions
carry over. The difference is that each high-level intermediate representation
(HIR) node is charged only when a shared *authored-versus-generated* classifier
says the author wrote it.[^1]

The classifier is the substantive contribution. It is not
`span.from_expansion()`, which also flags `for` loops, `?` and other
desugarings of source syntax. It is not a check of the outermost expansion mark
either, because that misses desugarings emitted inside macro bodies. It walks a
node's complete expansion chain. Authored desugarings are treated as
transparent; macro and compiler-pass expansions are treated as opaque. Charging
is separated from traversal, so author-written branches nested inside generated
wrappers still count.

The same classifier then replaces the inconsistent expansion handling in
`conditional_max_n_branches`, `bumpy_road_function` and the planned
`brain_type` walker. A later phase adds a Sonar-faithful scoring model, which
is the variant the empirical literature actually validates.

## 3. Problem

### 3.1. Generated branching is charged to the caller

Clippy's `cognitive_complexity` walks post-expansion HIR. It adds a unit for
every `if`, every `match` with more than one arm, and every arm guard, whatever
macro body the node came from.[^2] A macro that lowers to branching therefore
raises the score of every function that calls it. That branching is neither
written nor visible at the call site.

The trigger in practice is ordinary Cargo feature unification. `rstest-bdd`
0.6.0 depends on `tracing` with `features = ["log"]`. It is a dev-dependency
while `tracing` is a normal dependency, so building the test graph unifies
`tracing/log` onto the library node. Every `tracing::*` call site in the
library then gains the `log` bridge's branching. Issue 504 measured the effect
with `cognitive-complexity-threshold = 0`:

| Probe function                     | Bridge off | Bridge on |
| ---------------------------------- | ---------- | --------- |
| Four `tracing::*` calls, no branch | 1          | **29**    |
| One source `if`                    | 2          | 2         |
| `if` inside a macro argument       | 2          | 2         |
| `matches!` only                    | 2          | 2         |
| Closure inside a macro argument    | 3          | 3         |

*Table 1: Clippy cognitive complexity with and without the `tracing/log`
bridge, as measured in issue 504.*

All of the difference is generated branching: seven units per inline `tracing`
macro. The consuming repository cannot decline the feature, and Clippy has no
per-macro suppression. The only local remedies are to restructure source around
the metric or to turn the lint off. Netsuke restructured 39 functions before
taking the second option.

This is not specific to `tracing`. rust-clippy#14417, still open, reproduces
the problem with a plain `macro_rules!` macro. It also reports `info!` adding 7
units per call on stable 1.88.[^3]

### 3.2. `from_expansion()` is the wrong predicate

The obvious guard is to skip any node whose span satisfies `from_expansion()`.
That over-excludes, because rustc marks desugarings with an expansion context
just as it marks macros. Issue 504 instrumented a visitor over a crate
containing a `for` loop, a `?`, a `matches!` and a range literal, and logged
each visited node's `ExpnKind`:

| `ExpnKind`                 | Nodes | `from_expansion()` |
| -------------------------- | ----- | ------------------ |
| `Root`                     | 26    | `false`            |
| `Desugaring(ForLoop)`      | 11    | `true`             |
| `Desugaring(QuestionMark)` | 6     | `true`             |
| `Macro(Bang, matches)`     | 3     | `true`             |
| `Desugaring(RangeExpr)`    | 1     | `true`             |

*Table 2: Expansion kinds of visited HIR nodes in a probe crate, from issue
504.*

Eighteen of the 47 nodes satisfy `from_expansion()` without being
macro-generated. A `for` loop and a `?` are written in source. They must keep
whatever they cost today.

### 3.3. Whitaker's own lints disagree with each other

Whitaker has no single expansion policy, and its two existing
branching-sensitive lints err in opposite directions:

- `bumpy_road_function` over-excludes. It calls `span.from_expansion()` in four
  places:
  - an item guard and a body guard, which return before any analysis runs;
  - a guard at the top of the recursive `SegmentBuilder::visit_expr`, which
    returns before `walk_expr`, so it prunes the node *and its whole subtree*;
  - a guard in `push_segment`, which drops the segment.

  Combined with the `Desugaring(ForLoop)` contexts in Table 2, this means a
  source-written `for` loop, and everything nested inside it, never reaches the
  bump analysis. Arguments to macros are pruned the same way. The pruning is
  read from the source and the contexts are measured; the combination is a
  deduction that the test plan in §11 must confirm.
- `conditional_max_n_branches` under-excludes. Its driver has no
  `from_expansion`, `ExpnKind` or `ctxt()` reference, so generated conditions,
  for example the one `assert!(a && b && c)` lowers to, are counted. This is
  Clippy's defect again.

The brain trust design (§6.2.3) plans for the future `brain_type` HIR walker to
pass `span.from_expansion()` as the `is_from_expansion` flag of
`CognitiveComplexityBuilder`. Implemented as written, that would repeat the
`bumpy_road_function` over-exclusion for every `for` loop and `?`.

## 4. Current state

### 4.1. Clippy's lint

Clippy's `cognitive_complexity` sits in the `restriction` group, and its
documentation now opens "We used to think it measured how hard a method is to
understand".[^2][^4] On Clippy's current `master` the algorithm is as follows;
each step must be re-verified against the pinned toolchain's Clippy:

- start at 1;
- walk the body with `for_each_expr_without_closures`, so closures and nested
  items are scored as separate bodies via `check_fn`;
- add 1 for each `ExprKind::If`;
- add 1 for each `ExprKind::Match` with more than one arm, plus 1 for each
  guarded arm;
- count `ExprKind::Ret` nodes, skipping a `return` that immediately follows
  another;
- subtract the full return count when the body's type is `Result`, otherwise
  half of it, but only when the score is at least the adjustment;
- skip the whole body when its span is `from_expansion()`, skip functions
  carrying `#[test]`, and unwrap `async fn` bodies with `get_async_fn_body`;
- report when the score exceeds `cognitive-complexity-threshold` (default 25),
  which `#[clippy::cognitive_complexity = "N"]` can override per item.

There are no nesting increments, no boolean-sequence increments and no loop
increments as such. Loops are charged only through the `if` and `match` nodes
they lower to. The Clippy metric is therefore a structural cousin of the Sonar
metric, not an implementation of it.

### 4.2. The macro question in Clippy's history

rust-clippy#3793 proposed the lint in 2019, citing the Sonar white paper.[^5]
Expansion was the first design problem raised:

- Oli Scherer: the metric is "all syntactical, so we should probably move to a
  pre-expansion-pass. IMO macro calls reduce complexity, even if macro
  definitions increase it".
- In rust-clippy#3900, the lint's author, Félix Fischer, wrote that macros "do
  introduce abstraction, they are not 'weightless'. But they are cognitively
  (we believe) very close to reading a function call."[^6] The same thread
  reported `tracing` and `log` call sites tripping the lint as early as 2019.
- Philipp Krones then identified the flaw in a pre-expansion pass: it cannot
  see expressions inside macro arguments, because those are still unparsed token
  trees.[^6]

The pre-expansion rewrite (rust-clippy#3963) was closed unmerged.[^7] Fischer
later retracted the proposal, arguing that the white paper has no empirical
base and that no structural pass can measure understanding. rust-clippy#14915
rewrote the lint's documentation along those lines and closed rust-clippy#3793
in July 2025.[^4] The macro defect itself was never fixed.

### 4.3. Whitaker

| Component                                                           | Relevant behaviour                                                                                                               |
| ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `crates/bumpy_road_function/src/driver/{mod.rs,segment_builder.rs}` | Four `from_expansion()` guards; the `visit_expr` guard prunes subtrees; `DesugaringKind::WhileLoop` is special-cased.            |
| `crates/conditional_max_n_branches/src/driver.rs`                   | Inspects `If`, `while` loops and match guards; no expansion filtering; `max_branches` defaults to 2.                             |
| `common/src/brain_type_metrics/cognitive_complexity.rs`             | Sonar-style `CognitiveComplexityBuilder` with structural, nesting and fundamental increments and a per-call `is_from_expansion`. |
| `src/hir/mod.rs`                                                    | `walk_span_chain` and `recover_user_editable_hir_span` follow `source_callsite()` to recover editable spans for diagnostics.     |
| `crates/clippy_utils`                                               | A minimal shim; Clippy's real `clippy_utils` and its visitors are not available.                                                 |
| `rust-toolchain.toml`                                               | Pins `nightly-2026-05-28`; issue 504 measured on `nightly-2026-08-23`.                                                           |

*Table 3: Whitaker components that this RFC builds on or changes.*

`CognitiveComplexityBuilder` already embodies the right charging rule:
increments flagged as expanded are dropped, and an expanded nesting level is
pushed without raising the effective depth. What is missing is a correct
predicate to feed it, and a walker that never prunes.

## 5. Evidence base

### 5.1. The Sonar specification

G. Ann Campbell's white paper defines cognitive complexity by three rules:
ignore structures that let several statements be "readably shorthanded into
one"; add one for each break in linear flow; and add more when flow-breaking
structures are nested.[^8] Its first example of shorthand is the method call
itself: "Breaking code into methods allows you to condense multiple statements
into a single, evocatively named call … Thus, Cognitive Complexity does not
increment for methods." A Rust macro invocation is shorthand in exactly this
sense. Charging its expansion to the caller contradicts the specification's
first rule.

### 5.2. Empirical validation

Muñoz Barón, Wyrich and Wagner pooled 10 datasets, covering about 24,000
understandability judgements of 427 snippets, in a random-effects
meta-analysis.[^9] Cognitive complexity computed by SonarQube correlated with:

- comprehension time: r = 0.54, 95% CI [0.24, 0.75];
- subjective difficulty ratings: r = −0.29, 95% CI [−0.49, −0.06];
- a composite of time and correctness: r = 0.40, 95% CI [0.11, 0.62].

It did not correlate meaningfully with correctness (r = −0.13). The authors
call it "the first validated and solely code-based metric that is able to
reflect at least some aspects of code understandability".

Three caveats bear directly on this RFC:

- **The scores were computed on the text participants read.** The validation
  pairs scores taken from source snippets with judgements about those snippets.
  No snippet came from a language with macros. The evidence supports scoring
  code as written. It says nothing for scoring expanded HIR.
- **Only the Sonar model was validated.** Clippy's model, with no nesting
  increments, has no comparable evidence.
- **There is no validated threshold.** Only two of the studies had snippets
  above 15, SonarQube's default. The authors conclude that "a meaningful
  threshold value has yet to be identified" and recommend keeping the metric
  "as low as possible". Heterogeneity was high, with I² from 78% to 85% for
  time, correctness and the composite.

### 5.3. Rust-specific evidence

Ardito, Barbato, Coppola and Valsesia compared Rust with five other languages
on nine small benchmark programs, using a fork of `rust-code-analysis`.[^10]
Rust had the lowest average cognitive complexity per function. The study is
exploratory and does not validate the metric against human understanding. Its
definition cites Shao and Wang's cognitive weights rather than the Sonar rules,
and its sample is small. Its relevance here is methodological:
`rust-code-analysis` parses with tree-sitter, so it scored pre-expansion
source, where a macro invocation is an opaque token tree. No published Rust
measurement scores expanded HIR.

### 5.4. Industry precedent for macros

In 2022 a SonarQube user reported that a C++ logging macro containing an `if`
inflated `cpp:S3776` in "practically every method that's not trivial", the same
pathology as Table 1.[^11] Sonar's analysers already treated macros that
substitute a complete syntactic construct as opaque function calls. They
hesitated only over C macros that expand to partial constructs, such as a bare
`if` header that captures a following `else`. After internal discussion they
"decided that cognitive complexity should ignore macro invocations" (CPP-3743).

Rust's partial-construct concern cannot arise. A Rust macro invocation always
expands to a complete AST fragment of the kind its position requires, so every
Rust macro is the benign case Sonar describes.

### 5.5. What the evidence supports

| Claim                                                          | Support                                                                                                 |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Branching the reader cannot see should not be charged          | Sonar shorthand rule; Sonar's C++ decision; Clippy maintainers in rust-clippy#3793 and rust-clippy#3900 |
| Author-written branches inside macro arguments must be charged | Sonar's structural rules; Krones's objection to pre-expansion passes                                    |
| The metric is a useful ranking signal for comprehension effort | Moderate; meta-analysis over time and ratings, not correctness                                          |
| Any particular threshold is correct                            | None                                                                                                    |
| The Clippy model, specifically, tracks understanding           | None; only the Sonar model has been validated                                                           |

*Table 4: Claims this RFC relies on and the strength of their support.*

The lint must therefore be documented as a structural proxy for reading effort,
not as a measurement of understandability. Clippy's retraction was about
overclaiming, and this RFC does not repeat it.

## 6. Goals and non-goals

- Goals:
  - Score each function over the constructs its author wrote. Exclude
    macro-generated branching and keep branching that comes from desugaring
    source syntax.
  - Make Table 1's bridge-on and bridge-off columns identical.
  - Match Clippy's scores exactly on macro-free code, so that migrating
    thresholds and suppressions is mechanical.
  - Provide one classifier, with one test suite, that every Whitaker
    complexity-sensitive lint uses.
  - Report how much generated branching was excluded, so the gap between Clippy
    and Whitaker is visible rather than mysterious.
- Non-goals:
  - Measuring understandability. The lint is a proxy and its documentation
    must say so.
  - Scoring `macro_rules!` definitions or procedural macro implementations.
    Macro definitions can be complex, but that is a separate rule.
  - Recursion increments, `cfg`-dependent code, which is gone before HIR
    exists, or per-construct weightings beyond Clippy and Sonar.
  - Fixing Clippy upstream. Once validated, the findings should be offered on
    the upstream issue.[^3]

## 7. Proposed design

### 7.1. Overview

The lint is a `LateLintPass`. For each function, method and closure body it
walks the HIR expressions once. It classifies every candidate node with the
shared classifier, charges only authored nodes, and always descends into
children. Arithmetic lives in a pure, compiler-free `policy` module, and
compiler access lives in a thin `driver` behind `dylint-driver`, following the
crate layout that `conditional_max_n_branches` established.

### 7.2. The authored-versus-generated classifier

**Rule.** A node is *generated* when its expansion chain contains at least one
*opaque* step. Otherwise, it is *authored*. The chain is the sequence of
expansions between the node's syntax context and the crate root, outermost
first. A macro expansion, a compiler AST pass and a desugaring that inserts
logic the author did not write (`GeneratedDesugaring`) are all opaque. Only a
desugaring of syntax the author wrote (`AuthoredDesugaring`) is transparent.

Two consequences follow:

- A token substituted for a macro metavariable keeps the syntax context it had
  at the call site. An `if` the author passes as a macro argument therefore has
  no opaque step in its chain and stays authored.
- A `for` loop written inside a `macro_rules!` body carries a
  `Desugaring(ForLoop)` step and, beyond it, the macro's step. It is generated.
  The outermost-mark predicate proposed in issue 504 classifies it as authored,
  which is why the chain must be walked.

**Policy, in `common`.** The policy is pure Rust, with no `rustc_private`
dependency, so it runs in the ordinary test lane:

```rust
/// Whether a HIR node's construct was written by the author.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeOrigin {
    /// Written in source, directly or through a desugaring of source syntax.
    Authored,
    /// Synthesized by a macro, a compiler pass or a generating desugaring.
    Generated,
}

/// One expansion between a node and the crate root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpansionStep {
    /// Lowering of syntax the author wrote, such as `for`, `?` or `.await`.
    AuthoredDesugaring,
    /// Lowering that inserts logic the author did not write as control flow.
    GeneratedDesugaring,
    /// A bang, attribute or derive macro expansion.
    Macro,
    /// A compiler AST pass, such as test-harness generation.
    AstPass,
}

/// Classify a node from its expansion steps, outermost first.
pub fn classify(steps: impl IntoIterator<Item = ExpansionStep>) -> NodeOrigin {
    if steps.into_iter().all(|step| step == ExpansionStep::AuthoredDesugaring) {
        NodeOrigin::Authored
    } else {
        NodeOrigin::Generated
    }
}
```

**Adapter, in `src/hir/`.** The adapter maps a real `Span` onto the policy,
behind `dylint-driver`:

```rust
/// Classify the construct whose own span is `span`.
pub fn node_origin(span: Span) -> NodeOrigin { classify(expansion_steps(span)) }

/// Report whether `span` belongs to macro- or pass-generated code.
pub fn is_generated(span: Span) -> bool { node_origin(span) == NodeOrigin::Generated }
```

`ExpnKind` maps to steps as follows. The match over `DesugaringKind` must be
exhaustive, with no wildcard arm. A toolchain bump that adds a variant then
fails to compile until a reviewer has classified it.

| `ExpnKind`                                                                                 | Step                  | Rationale                                                                                                          |
| ------------------------------------------------------------------------------------------ | --------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `Root`                                                                                     | (end of chain)        | Source text.                                                                                                       |
| `Macro(Bang \| Attr \| Derive, _)`                                                         | `Macro`               | The expansion is the macro's text, not the caller's.                                                               |
| `AstPass(_)`                                                                               | `AstPass`             | Prelude injection and harness generation are not authored.                                                         |
| `Desugaring(QuestionMark \| TryBlock \| YeetExpr)`                                         | `AuthoredDesugaring`  | The author wrote `?`, `try` or `do yeet`.                                                                          |
| `Desugaring(ForLoop \| WhileLoop)`                                                         | `AuthoredDesugaring`  | The author wrote the loop.                                                                                         |
| `Desugaring(Async \| Await)`                                                               | `AuthoredDesugaring`  | The author wrote `async` or `.await`; see open question 2.                                                         |
| `Desugaring(RangeExpr \| FormatLiteral { .. } \| OpaqueTy \| BoundModifier \| PatTyRange)` | `AuthoredDesugaring`  | No control flow; classified for completeness.                                                                      |
| `Desugaring(Contract)`                                                                     | `GeneratedDesugaring` | Contract attributes inject checks the author wrote as predicates, not as branches, in the same way `assert!` does. |

*Table 5: Mapping from `rustc_span::hygiene::ExpnKind` to classifier steps.*

The variant list follows `DesugaringKind` in current nightly documentation[^12]
and must be re-verified against the pinned toolchain.

**Walking the chain.** Walk the chain one mark at a time, from the node's
`SyntaxContext` towards the root. At a macro, AST-pass or generated-desugaring
mark, stop and report `Generated`. Never follow a macro's `call_site`, because
that leads back to the caller's context and would wrongly classify the macro's
text as authored.

At a transparent desugaring mark, the walk must continue into the context that
was being lowered. Implementers must confirm on the pinned compiler whether
rustc stacks a desugaring mark on top of the enclosing context, or re-roots it
and leaves the enclosing context reachable only through `ExpnData::call_site`.
If it re-roots, the adapter follows `call_site` for desugaring marks only.
Phase 1 is blocked until this is settled, because the outcome decides whether a
`for` loop or `?` inside a macro body is classified correctly.

**Contract.** The classifier answers "who wrote this construct?" It decides
neither whether to descend nor where to report:

- Classify the node's own span, never a span recovered by `walk_span_chain` or
  `recover_user_editable_hir_span`. Those helpers stay diagnostic-only.
- Never use `from_expansion()` as a charging or pruning predicate in a
  complexity-sensitive lint.
- Guard expressions, match arms and the `if` or `match` node are each
  classified independently.

### 7.3. Charging versus traversal

Generated control flow wraps author-written expressions; it does not replace
them. `assert!(if c { x } else { y })` lowers to a generated `if` whose
condition is the author's `if`. `println!("{}", if c { a } else { b })` lowers
to generated formatting machinery whose argument contains the author's `if`.
Pruning a generated subtree loses those authored branches. The walker therefore
follows two rules:

1. **Charge by origin.** A generated node adds nothing to any tally and does
   not take part in return bookkeeping.
2. **Always descend.** After classifying a node, call `walk_expr`
   unconditionally. A generated node is skipped; its children are not.

Gating only the descent, for example by returning `Descend::No` from a
`for_each_expr` callback, is not a substitute for the first rule. The callback
has already run for that node before the descent decision applies.

### 7.4. Scoring model

Version 1 reproduces Clippy's arithmetic over authored nodes:

| Construct                                         | Clippy                                              | Proposed                                      |
| ------------------------------------------------- | --------------------------------------------------- | --------------------------------------------- |
| Baseline                                          | 1                                                   | 1                                             |
| `ExprKind::If`                                    | +1                                                  | +1 when authored                              |
| `ExprKind::Match` with more than one arm          | +1                                                  | +1 when the `match` is authored               |
| Guarded arm                                       | +1 per guard                                        | +1 per guard whose own expression is authored |
| `ExprKind::Ret`, not directly after another `Ret` | counted                                             | counted when authored                         |
| Return adjustment                                 | full count for `Result`, else half, when `cc ≥ adj` | same rule over authored returns               |
| Body boundary                                     | `for_each_expr_without_closures`                    | same; see §7.5 for generated closures         |

*Table 6: Charges in Clippy's model and in the proposed version 1.*

For the consecutive-return rule, only authored nodes update the "previous
expression" state.

Reusing Clippy's arithmetic changes only one variable, the exclusion predicate.
That keeps Clippy's scores as a test oracle (§11.4) and lets
`cognitive-complexity-threshold` move across unchanged. The Sonar model, which
§5.2 shows is the better-evidenced one, follows in phase 5 (§12) as
`model = "sonar"`. It drives `CognitiveComplexityBuilder` from the same
classifier.

### 7.5. Function and closure selection

- **Bodies.** Use `check_fn` for item functions, methods and closures, as
  Clippy does, and skip functions carrying `#[test]`. Unwrap `async fn` bodies
  as `get_async_fn_body` does, after verifying the HIR shape on the pin.
- **Anchors.** A body's anchor is the function's identifier span, or a
  closure's `fn_decl_span`. Clippy instead tests the item span with
  `from_expansion()`.
  - The identifier is what the author named. It stays authored through
    attribute macros that re-emit the function, such as
    `#[tracing::instrument]` and `#[tokio::main]`, and it is generated for
    derive output.
- **Generated closures are transparent.** A closure or `async` block whose
  anchor is generated is not a separate unit of reading, because the author
  wrote no closure.
  - `check_fn` skips it.
  - The enclosing body's walker descends into it, and its authored contents
    are charged to that body.
  - Without this rule, a macro that wraps `$body` in a closure would hide the
    author's branching completely.
  - This matters for attribute macros that wrap an `async fn` body in a
    generated `async move` block.
- **Generated functions.** A function with a generated anchor is scored like
  any other. When its score exceeds the limit, the diagnostic is emitted only
  if at least one authored charge was recorded. It is then placed at the
  user-editable span recovered by `recover_user_editable_hir_span`. Derive
  output, which contains no authored charges, never reports.

### 7.6. Worked examples

| Function body                             | Clippy        | Proposed | Source of the difference                           |
| ----------------------------------------- | ------------- | -------- | -------------------------------------------------- |
| Four `tracing::*` calls, `log` bridge on  | 29 (measured) | 1        | 28 generated units excluded                        |
| `matches!(x, Some(_))`                    | 2 (measured)  | 1        | Generated two-arm `match` excluded                 |
| One authored `if` inside a macro argument | 2 (measured)  | 2        | Authored `if` in an argument still charged         |
| `assert!(a && b)`                         | 2 (expected)  | 1        | Generated `if` excluded                            |
| `matches!(x, Some(v) if v > 0)`           | 3 (expected)  | 2        | `match` excluded; authored guard charged           |
| `for x in xs { if x { … } }`              | 3 (expected)  | 3        | Desugared `match` and source `if` both charged     |
| `let v = f()?;` in a `Result` function    | 1 (expected)  | 1        | Desugared `match` and return; adjustment as before |

*Table 7: Clippy and proposed scores for small bodies. "Measured" values come
from issue 504; "expected" values are derived from Clippy's rules and must be
pinned by the UI fixtures in §11.3.*

### 7.7. Configuration

```toml
[macro_aware_cognitive_complexity]
max_complexity = 25
```

`max_complexity` is a `u64` and defaults to 25, Clippy's default. The lint
fires when the score is strictly greater than the limit, as Clippy's does. The
configuration struct uses `serde(default, deny_unknown_fields)` and is loaded in
`check_crate`, in the same way as `conditional_max_n_branches`. Per-item
suppression uses `#[expect(macro_aware_cognitive_complexity, reason = "...")]`.
Whether to add an equivalent of `#[clippy::cognitive_complexity = "N"]` is open
question 4.

### 7.8. Diagnostics

The diagnostic is emitted at the anchor span, and its text is localized through
the shared Fluent resources in `en-GB`, `cy` and `gd`:

```plaintext
warning: `parse_manifest` has a cognitive complexity of 31 (limit 25)
  --> src/manifest.rs:42:8
   |
42 | pub fn parse_manifest(input: &str) -> Result<Manifest, Error> {
   |        ^^^^^^^^^^^^^^
   |
   = note: 14 units of macro-generated branching were not counted
   = help: split the function into smaller functions with clear names
```

The note appears only when the excluded count is non-zero. The excluded count
is the number of generated `if`, multi-arm `match` and guard charges. It lets a
user reconcile the score with Clippy's, and it is the quantity checked by the
differential test in §11.4.

### 7.9. Known blind spots

- **Procedural macros that re-span generated tokens with user spans.**
  `quote_spanned!(user_span => if … { … })` gives generated code an authored
  syntax context. The classifier then charges it, because hygiene is the only
  provenance signal rustc keeps. Conversely, a procedural macro that re-emits
  user code with `Span::call_site()` hides that code. Both are defects in the
  macro's span hygiene, and the user's guide must name them.
- **Control-flow domain-specific language (DSL) macros.** `tokio::select!`,
  `anyhow::ensure!` and similar macros genuinely branch, and the reader knows
  it. Version 1 charges them like any macro: zero for the generated branching
  and full price for authored expressions inside them. This follows the
  shorthand rule, but it can under-charge a `select!` with many arms. See open
  question 3.
- **Mixed-provenance spans.** For a construct whose leading keyword is
  generated and whose blocks are authored, classification follows the context
  that `Span::to` assigns to the combined HIR span on the pinned compiler.
  Fixtures must pin the
  `macro_rules! when { ($c:expr, $b:block) => { if $c $b } }` case.

### 7.10. Placement and reuse policy

- `common/src/node_origin.rs` owns `NodeOrigin`, `ExpansionStep` and
  `classify`. It has no compiler dependency, and it is the only place the
  transparency policy is written down.
- `src/hir/node_origin.rs` owns the `ExpnKind`-to-`ExpansionStep` table and
  the chain walk. It is the only code permitted to inspect `ExpnKind` for
  provenance.
- Permitted callers are lint drivers that charge, count or segment HIR nodes
  by provenance. Diagnostic placement keeps using
  `recover_user_editable_hir_span`. The two must not be composed.
- `CognitiveComplexityBuilder`'s `is_from_expansion` parameters keep their
  names for compatibility. Callers pass `is_generated(span)`.
- On acceptance, record the classifier as an architectural decision record
  (ADR), referenced from `docs/whitaker-dylint-suite-design.md`. Document the
  three walker rules in `docs/developers-guide.md`: classify the node's own
  span, never prune generated subtrees, and recover spans only for diagnostics.

## 8. Requirements

### 8.1. Functional requirements

- The lint MUST give the same score for a function with and without the
  `tracing/log` bridge enabled.
- On macro-free code, the lint MUST give Clippy's score for every function
  Clippy scores at the pinned toolchain. Any body that Clippy skips because of
  a desugaring mark, and Whitaker scores, MUST be listed in the user's guide.
- The lint MUST charge author-written `if`, `match` and guard constructs that
  appear inside macro arguments, including closures passed as arguments.
- The lint MUST NOT charge `if`, `match`, guard or `return` constructs written
  in a macro body, including desugared `for` loops and `?` in that body.
- Wrapping any authored expression in an identity macro,
  `macro_rules! id { ($($t:tt)*) => { $($t)* } }`, MUST NOT change any score.
- The diagnostic MUST report the score, the limit and, when non-zero, the
  excluded generated units.

### 8.2. Technical requirements

- The classifier policy MUST live in `common` without `rustc_private`, and the
  adapter MUST live in `src/hir/` behind `dylint-driver`.
- The adapter's `DesugaringKind` match MUST be exhaustive, with no wildcard
  arm.
- No complexity-sensitive lint may call `from_expansion()` to decide charging
  or traversal once phase 4 has landed.
- The walker MUST call `walk_expr` for every visited node, generated or not.
- The lint MUST be registered as experimental in `suite/Cargo.toml`,
  `suite/src/lints.rs`, `suite/src/driver.rs` and
  `installer/src/resolution.rs`, following `rstest_helper_should_be_fixture`.
- Behaviour measured in issue 504 on `nightly-2026-08-23` MUST be
  re-established on the repository pin before it is used as an oracle.

## 9. Adoption by existing lints

Adoption is a separate phase with its own fixtures and changelog entries,
because it changes existing diagnostics:

- **`conditional_max_n_branches`.** Skip `inspect_condition` for generated
  `If` nodes and generated `while` loops. Classify each match guard by its own
  span. Traversal is unchanged, because `check_expr` already sees every
  expression. Generated conditions such as `assert!(a && b && c)` stop being
  flagged. An authored three-predicate `if` inside a macro argument keeps being
  flagged.
- **`bumpy_road_function`.**
  - Replace the item and body guards with `is_generated` on the anchor.
  - Remove the early return from `SegmentBuilder::visit_expr`: record no
    segment or predicate for a generated node, but walk its children.
  - Replace the `push_segment` guard with the classifier.
  - Keep the `DesugaringKind::WhileLoop` handling and the closure policy.

  Source `for` loops and macro arguments begin to contribute bumps. Whitaker
  issue 357 (the weighting model) stays out of scope.
- **`brain_type` and `brain_trait`.** Amend brain trust design §6.2.3 so the
  future HIR walker passes `is_generated(span)`, not `from_expansion()`, to
  `CognitiveComplexityBuilder`. Note that `brain_type` uses the Sonar model
  with a threshold borrowed from Clippy's scale (open question 8).

## 10. Compatibility and migration

Phases 1 to 3 add an opt-in experimental lint and change nothing else. To
migrate from Clippy, a project:

- disables `clippy::cognitive_complexity`;
- moves `cognitive-complexity-threshold` from `clippy.toml` to
  `max_complexity` in `dylint.toml`;
- replaces `#[expect(clippy::cognitive_complexity)]` attributes with the
  Whitaker lint name.

Scores are usually equal to or lower than Clippy's. They can be higher in two
cases:

- Macro-generated `return`s, from macros such as `bail!`, were offsetting
  authored branches through Clippy's return adjustment.
- A generated closure or `async` block now charges its authored contents to
  the enclosing function, where Clippy previously skipped them.

Both are corrections, and the user's guide must explain them. Phase 4 changes
two stable lints in opposite directions: `conditional_max_n_branches` reports
less and `bumpy_road_function` may report more. Each change needs its own
changelog entry and a migration note.

## 11. Test plan

### 11.1. Classifier policy

- `rstest` cases: an empty chain; each single step; `AuthoredDesugaring`
  before `Macro`; `Macro` before `Macro`; two `AuthoredDesugaring` steps; and
  `GeneratedDesugaring` alone.
- Exhaustive enumeration of every chain of length 0 to 4 over the four steps
  (341 chains), checking three properties:
  - inserting `AuthoredDesugaring` anywhere never changes the result;
  - any opaque step yields `Generated`;
  - the empty chain yields `Authored`.

  At this size enumeration is strictly stronger than sampling. Add a `proptest`
  property over unbounded lengths for the first two.

### 11.2. Adapter

Extend the synthetic hygiene tests in `src/hir/tests.rs`. Parameterize the
expansion constructor by `ExpnKind` and parent context, then assert the
classification of root, `Macro(Bang)`, `Desugaring(ForLoop)` over root,
`Desugaring(QuestionMark)` over a macro mark, and a macro over a macro. The
module documentation must say that synthetic contexts do not prove real
lowering; that is what §11.3 is for.

### 11.3. User-interface fixtures

Driver-gated `tests/ui.rs` with `fn ui()` and `max_complexity = 0`, using only
`std` and local `macro_rules!`:

- a `matches!`-only function scores 1;
- an authored `if` inside `println!` and `assert!` arguments is charged;
- an authored `if` wrapping `matches!` charges the `if` but not the `match`;
- nested macros, where a local macro calls `matches!` or emits an `if`, charge
  neither layer;
- a closure passed to a macro is scored separately, its authored branch is
  charged, and the enclosing function is unaffected;
- a macro that wraps `$body` in a closure charges the author's branches to the
  enclosing function (§7.5);
- source `for`, `while` and `?` are charged, and the same constructs in a
  `macro_rules!` body are not;
- an authored guard inside `matches!` is charged;
- the §7.9 `when!` mixed-provenance case is pinned;
- `Result` and non-`Result` functions with returns, including macro-generated
  returns, pin the adjustment;
- `async fn` bodies, `async` blocks and `.await` are pinned;
- each Table 7 row is pinned, and so is the identity-macro invariance of §8.1.

### 11.4. Differential parity with Clippy

A test target builds a macro-free fixture crate twice, once under
`cargo clippy` with `-W clippy::cognitive_complexity` and threshold 0, and once
under Whitaker with `max_complexity = 0`. It parses both sets of scores and
asserts per-function equality. On a second fixture containing macros but no
generated `return`, it asserts that Clippy's score minus Whitaker's equals the
excluded count in the note. Both run on the pinned toolchain's Clippy component.

### 11.5. Feature-matrix end-to-end test

A subprocess test generates two standalone fixture projects, following
`crates/no_std_fs_operations/tests/test_support`. They differ only in
`tracing`'s `log` feature. It asserts that:

- `cargo tree -e features` shows the difference;
- the four-call probe scores 1 in both projects;
- the diagnostics are byte-identical.

The test is `#[ignore]`d, because it needs registry access, and sits in its own
nextest group with a timeout.

### 11.6. Corpus evaluation

Before promotion out of experimental, run Clippy and Whitaker over Whitaker,
Netsuke and a sample of public crates, and publish the score distributions. On
Netsuke, issue 504's prototype found a workspace maximum of 6 against 39
functions above 9 under Clippy. Every per-function difference must be explained
by excluded units or by the §10 cases. Hand-inspect the 20 highest-scoring
functions under each model.

## 12. Delivery phases

| Phase | Deliverable                                                                                     | Exit gate                                          |
| ----- | ----------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| 1     | `common` policy, `src/hir/` adapter, desugaring-chain behaviour verified on the pin             | §11.1 and §11.2 pass; chain-walk decision recorded |
| 2     | `crates/macro_aware_cognitive_complexity` with the Clippy model, configuration and diagnostics  | §11.3, §11.4 and §11.5 pass                        |
| 3     | Experimental suite and installer registration; user's guide and design documentation            | Suite build with and without the feature           |
| 4     | Adoption in `conditional_max_n_branches`, `bumpy_road_function` and brain trust design          | Per-lint fixtures; changelog and migration notes   |
| 5     | `model = "sonar"` backed by `CognitiveComplexityBuilder`; corpus evaluation; promotion decision | §11.6 report reviewed                              |

*Table 8: Delivery phases and their exit gates.*

[Roadmap](../roadmap.md) phase 15 schedules phases 1, 2, 3 and 5. Roadmap phase
16 extends phase 4 into a single breaking release. That release moves
`conditional_max_n_branches`, `bumpy_road_function` and the brain trust lints
onto the shared measure selected by the corpus evaluation.

## 13. Alternatives considered

| Option                                             | Advantages                                      | Rejected because                                                                                                       |
| -------------------------------------------------- | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Skip nodes where `from_expansion()` holds          | One call; already used by `bumpy_road_function` | Excludes `for`, `?`, `.await` and ranges (Table 2); pruning variants also lose macro arguments                         |
| Check only the outermost mark (issue 504's sketch) | Simple; agrees with every measured case         | Classifies desugarings inside macro bodies as authored                                                                 |
| Pre-expansion (early) pass                         | Sees exactly what the author wrote              | Macro arguments are unparsed token trees, so authored branches in them are invisible; rust-clippy#3963 stalled on this |
| Charge +1 per macro invocation                     | Matches the function-call analogy literally     | Breaks Clippy parity; re-penalizes logging; function calls themselves cost 0 in both models                            |
| Per-macro allow or deny lists only                 | User control                                    | Endless curation; does not fix desugarings or the existing lints; may follow later for DSL macros                      |
| Ship the Sonar model first                         | Better empirical support                        | Changes two variables at once and loses Clippy as an oracle; deferred to phase 5, not rejected                         |
| Patch Clippy upstream                              | Fixes the problem for everyone                  | The lint is deprecated in spirit and in `restriction`; Whitaker needs the classifier for its own lints anyway          |

*Table 9: Alternatives to the proposed design.*

The pre-expansion option deserves emphasis because it was Clippy's own plan.
Krones's objection is decisive: an `if` inside `assert!(…)` exists only as
tokens before expansion. Post-expansion HIR with hygiene-based classification
sees the parsed argument and still knows who wrote it. That is the combination
a pre-expansion pass cannot provide.

## 14. Open questions

1. Does the pinned compiler stack desugaring marks on the enclosing context,
   or re-root them with the context reachable only through `call_site`? This
   blocks phase 1 (§7.2).
2. Should `.await` stay `AuthoredDesugaring`, keeping Clippy parity if Clippy
   charges its lowered `match`, or be treated as shorthand like `?` in the
   Sonar model?
3. Should a `charged_macros` setting let users assign a fixed increment to
   control-flow DSL macros such as `tokio::select!`?
4. Is a per-item threshold override, equivalent to
   `#[clippy::cognitive_complexity = "N"]`, worth its cost, or is
   `#[expect(…, reason = …)]` enough?
5. Should test-function skipping stay at Clippy's `#[test]` check, or use
   Whitaker's broader test detection, which covers `rstest` and `tokio::test`?
6. Should the default stay at 25 for parity, or move towards SonarQube's 15,
   given that Netsuke peaks at 6 once generated branching is excluded?
7. Is `macro_aware_cognitive_complexity` the right name? Clippy's retraction
   argues against implying a measurement of understanding. Candidates are
   `authored_cognitive_complexity` and `structural_complexity`.
8. Should `brain_type` use the same model and scale as this lint, so that its
   threshold of 25 means the same thing in both places?

## 15. Recommendation

Accept the authored-versus-generated classifier as Whitaker's single provenance
policy, and ship `macro_aware_cognitive_complexity` as an experimental lint
with Clippy-compatible arithmetic.

The evidence is consistent:

- The Sonar specification excludes shorthand.
- Sonar's C++ analyser made the same decision for macros.
- Clippy's maintainers wanted macro calls to cost what function calls cost.
- The only validation study scored code as written.

Clippy's post-expansion walk contradicts all four, and `from_expansion()`
overcorrects. Walking the full expansion chain, charging by origin and always
descending fixes both errors. It also gives Whitaker's existing lints a shared
rule in place of two opposite mistakes.

Resolve open question 1 before implementation starts. Then adopt the classifier
in the existing lints as a separately reviewed phase, and add the Sonar model
once the corpus evaluation can show what it changes.

[^1]: [leynos/whitaker#504](https://github.com/leynos/whitaker/issues/504)
  records the `tracing/log` measurements, the `ExpnKind` census and the Netsuke
  results used throughout this RFC.
[^2]: [Clippy's `cognitive_complexity.rs`](https://github.com/rust-lang/rust-clippy/blob/master/clippy_lints/src/cognitive_complexity.rs)
  contains the scoring loop, return adjustment and body selection summarized in
  §4.1.
[^3]: [rust-lang/rust-clippy#14417](https://github.com/rust-lang/rust-clippy/issues/14417)
  reports macro-expanded code being charged to callers, including `info!` at 7
  units per call.
[^4]: [rust-lang/rust-clippy#14915](https://github.com/rust-lang/rust-clippy/pull/14915)
  rewrote the lint's documentation to disclaim measuring understandability.
[^5]: [rust-lang/rust-clippy#3793](https://github.com/rust-lang/rust-clippy/issues/3793)
  is the original proposal, the draft Rust mapping and its later retraction.
[^6]: [rust-lang/rust-clippy#3900](https://github.com/rust-lang/rust-clippy/issues/3900)
  asks for macro-expanded code to be ignored and records the pre-expansion
  objection.
[^7]: [rust-lang/rust-clippy#3963](https://github.com/rust-lang/rust-clippy/pull/3963)
  is the unmerged pre-expansion rewrite.
[^8]: G. Ann Campbell, *Cognitive Complexity: a new way of measuring
  understandability*, SonarSource white paper, version 1.7, 29 August 2023.
[^9]: Marvin Muñoz Barón, Marvin Wyrich and Stefan Wagner, "An Empirical
  Validation of Cognitive Complexity as a Measure of Source Code
  Understandability", ESEM 2020,
  [doi:10.1145/3382494.3410636](https://doi.org/10.1145/3382494.3410636).
[^10]: Luca Ardito, Luca Barbato, Riccardo Coppola and Michele Valsesia,
  "Evaluation of Rust code verbosity, understandability and complexity", *PeerJ
  Computer Science* 7:e406, 2021,
  [doi:10.7717/peerj-cs.406](https://doi.org/10.7717/peerj-cs.406).
[^11]: [Sonar Community: C++ cognitive complexity rule counts macro contents](https://community.sonarsource.com/t/c-cognitive-complexity-rule-counts-macro-contents/67322)
  records Sonar's decision to ignore macro invocations.
[^12]: [`rustc_span::hygiene::DesugaringKind`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_span/hygiene/enum.DesugaringKind.html)
  lists the compiler's desugaring kinds.

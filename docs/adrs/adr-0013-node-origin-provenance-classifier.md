# Architectural decision record (ADR) 0013: classify HIR node origin from expansion marks

## Status

Accepted (2026-11-08): Decide per HIR node whether it was authored or generated
by walking its syntax context's expansion marks from the outermost inwards,
treating desugarings as transparent and the first macro or AST-pass mark as
proof of generation. Keep the policy compiler-free in `whitaker-common`, and
adapt rustc hygiene to it inside the `whitaker` crate behind the
`dylint-driver` feature.

## Date

2026-11-08.

## Context and problem statement

A complexity lint that charges branches must not charge branches a macro
expansion produced. Without that distinction, a one-line authored call such as
`assert!(a && b && c)` becomes several branches, and a macro body's control
flow is attributed to every call site. Issue #504 and
[RFC 0010](../rfcs/rfc-0010-macro-aware-cognitive-complexity.md) require a
per-node answer rather than a per-file or per-span heuristic.

Rustc hygiene already records where a token came from. Each `Span` carries a
`SyntaxContext`, and a context is a chain of expansion marks, each naming an
`ExpnId` whose `ExpnData` classifies the expansion as a macro, an AST pass, or
a desugaring. The question is which parts of that chain mean "generated", and
in which direction the chain must be read.

The implementation plan for #504 forbids classification shortcuts:
`Span::from_expansion`, `Span::source_callsite`, `walk_span_chain`, and
`recover_user_editable_hir_span` are not permitted as provenance answers. The
plan also sets a decision gate: if syntax contexts did not retain the origin of
a node's parent, M1 had to stop rather than ship a misleading classifier.

### Compiler-generation receipt

All measurements below come from one coherent generation:

| Component                                  | Recorded value                                                            |
| ------------------------------------------ | ------------------------------------------------------------------------- |
| Toolchain                                  | `nightly-2026-05-28`                                                      |
| rustc                                      | `1.98.0-nightly` (`57d06900fd7d9ee06d3a7f323bb77f17ab3cfaf8`, 2026-05-27) |
| Whitaker Clippy snapshot                   | `e4608a25c2e20116fbc1e97a51a73f094a1db7a1`                                |
| Dylint-selected Clippy revision            | `9fca3bc9fc2bc83c60bde26d18ed68f11564b228`                                |
| Dylint                                     | 6.3.0                                                                     |
| Clippy upstream `main` observed 2026-11-08 | `146674fd0b6b414a2ae3e0a0b05a1b4885567b08`                                |

*Table 1: Compiler generation used for the chain-walk decision.*

The snapshot is the tip of Clippy `main` on 2026-10-28; upstream `main`
observed on 2026-11-08 pins `nightly-2026-10-30`. The adopted generation is
therefore about 155 days behind Clippy's own nightly. Closing that gap is owned
by issue #561 and its monitoring by issue #579. This ADR records the gap and
does not move the pin.

## Decision drivers

- Desugarings must not turn authored code into generated code. A `for` loop or
  a `?` expression is authored even though rustc lowers it into a marked
  context.
- Code passed as a macro argument must remain authored. The expansion
  surrounds it without originating it.
- Code written inside a macro definition must be generated when the macro is
  invoked.
- The policy must be expressible without rustc types so it can be enumerated,
  property-tested, and proved.
- One adapter, not a trait hierarchy or compatibility facade.
- The classifier answers provenance only. It must not decide traversal,
  diagnostic spans, or suppression.

## Options considered

| Option                              | Assessment                                                                                                                                                                                            |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Span::from_expansion()`            | Reports whether any mark exists. A `for` loop or `?` would report generated, so the answer is wrong for authored lowering. Rejected.                                                                  |
| `Span::source_callsite()`           | Collapses a node to the macro call site. It cannot distinguish a macro body from a macro argument at the same call site. Rejected.                                                                    |
| `walk_span_chain`                   | Walks span parents rather than expansion marks, and the plan forbids it as a classification shortcut. Rejected.                                                                                       |
| `SyntaxContext::marks()` directly   | Documented innermost-first and allocates a `Vec`. Passing it to a policy whose contract is outermost-first would invert any future order-sensitive rule. Rejected in favour of an incremental cursor. |
| Incremental outermost-first removal | Reads exactly the chain rustc stores, allocates nothing, and makes direction explicit in the iterator. Chosen.                                                                                        |

*Table 2: Options for reading a node's expansion provenance.*

## Decision outcome

`ExpansionMark` has three variants: `Desugaring`, `Macro`, and `AstPass`.
`classify` consumes them outermost first:

- a `Desugaring` is transparent and the walk continues;
- the first `Macro` or `AstPass` returns `NodeOrigin::Generated`;
- an exhausted sequence, including the empty sequence, returns
  `NodeOrigin::Authored`.

`NodeOrigin` has exactly `Authored` and `Generated`, and an `is_generated`
predicate for call sites that do not need the enum.

The rustc adapter wraps a `SyntaxContext` in a `MarkCursor`. Each `next` call
returns `None` at the root, otherwise calls `SyntaxContext::remove_mark`, which
pops the outermost mark and mutates the context to its parent, and maps the
returned `ExpnId` through `ExpnData::kind`. The `ExpnKind` match is exhaustive
over `Root`, `Desugaring`, `Macro`, and `AstPass`; there is no wildcard arm, so
a new compiler variant becomes a compile error rather than silently falling
into a default. `Root` maps to no mark because the cursor stops before it.

### Decision-gate result

Syntax contexts do retain parent origin. Every generated node measured below
carried the macro mark outside its desugaring marks, and `remove_mark` returned
the outermost mark first. The gate did not trigger; implementation continued.

### Evidence from real lowering

A temporary Dylint probe compiled a fixture and printed each selected HIR
expression's span, context id, and mark chain. The chains below are written
outermost first, as the cursor produces them.

| Fixture site                          | Context chain                                                             | Origin    |
| ------------------------------------- | ------------------------------------------------------------------------- | --------- |
| Source `if` at 49:5                   | empty                                                                     | Authored  |
| `body_if!` body `if` at 6:9           | `Macro(Bang, "body_if")`                                                  | Generated |
| `takes!(if …)` argument at 54:35      | empty                                                                     | Authored  |
| `when!` definition body `if` at 18:9  | `Macro(Bang, "when")`                                                     | Generated |
| `when!` caller argument `if` at 56:31 | empty                                                                     | Authored  |
| Source `for` at 58:19                 | `Desugaring(ForLoop)`                                                     | Authored  |
| Source `?` at 63:13                   | `Desugaring(QuestionMark)`                                                | Authored  |
| `body_for!` body `for` at 24:9        | `Desugaring(ForLoop) > Macro(Bang, "body_for") > Macro(Bang, "body_for")` | Generated |
| `body_try!` body `?` at 30:17         | `Desugaring(QuestionMark) > Macro(…) > Macro(…)`                          | Generated |
| Nested macro body `if` at 36:9        | `Macro(Bang, "inner_body")`                                               | Generated |

*Table 3: Expansion chains observed from real rustc lowering.*

Three details matter for consumers:

1. Macro arguments carry no mark. A macro cannot launder authored code into
   generated code.
2. Desugarings do appear above macro marks inside a macro body, so the
   classifier must skip them rather than stop at the first mark.
3. A desugared macro-body span can carry the same macro `ExpnId` twice, and a
   nested macro chain may retain only the inner macro mark. Origin is a
   presence question, never a count or a depth question.

## Production/model correspondence

`verus/node_origin.rs` models the policy as sequences of `Mark`. The bridge is
a reviewed mirror, because Verus cannot import the Cargo crate directly. The
receipt is the transition table below: every production decision point has a
model transition with the same result, and each row names the production test
and model evidence that keep the two aligned.

| Production transition             | Model transition                            | Evidence                                                   |
| --------------------------------- | ------------------------------------------- | ---------------------------------------------------------- |
| Root context, no marks            | `scan` loop not entered, returns `Authored` | `case_01_empty`, `lemma_class_witnesses`                   |
| Head `Desugaring`                 | `index = index + 1` and continue            | `case_02_desugaring`, exhaustive suite                     |
| Head `Macro`                      | return `Generated`                          | `case_03_macro_mark`, `scan`                               |
| Head `AstPass`                    | return `Generated`                          | `case_04_ast_pass`, `scan`                                 |
| Only desugarings, then exhaustion | final invariant + `Authored`                | `lemma_class_witnesses`, `lemma_desugaring_is_transparent` |
| `Root` is never a chain element   | no `Mark` variant for it                    | adapter `root` case, exhaustive `ExpnKind` match           |

*Table 4: Correspondence between production transitions and the Verus model.*

The model's specification is deliberately independent of the executable
early-exit scan: `spec_origin` is an existential over the sequence, and
`scan`'s postcondition states equality with it. `lemma_class_witnesses`
supplies non-empty witnesses for both classes, `lemma_concatenation` proves
that a concatenation is generated exactly when either part is, and
`lemma_desugaring_is_transparent` composes that result to show a desugaring
never changes an origin. The verifier reports 8 verified, 0 errors.

One boundary is explicit: production `classify` is generic over any iterator,
whereas `scan` models a sequence. `MarkCursor` produces exactly one mark per
`remove_mark`, and the adapter tests pin the sequence it yields. The proof
covers the mark-sequence policy, not the iterator plumbing, the rustc hygiene
extraction, HIR visitor order, or body boundaries. Those remain covered by
compiler-backed tests and later UI fixtures.

### Mutation evidence

Each alternative rule was injected and had to fail:

| Mutation                                                  | Result                              |
| --------------------------------------------------------- | ----------------------------------- |
| Model treats `Macro` as transparent                       | 2 assertion errors, exit 1          |
| Model stops at the outermost mark when it is a desugaring | postcondition not satisfied, exit 1 |
| Policy treats `Macro` as transparent                      | 8 tests failed, exit 101            |
| Policy returns `Authored` for an outermost desugaring     | 7 tests failed, exit 101            |

*Table 5: Injected-rule failures that would have to pass under a wrong rule.*

## Known risks and limitations

- Provenance is a hygiene approximation. A procedural macro can manufacture
  spans that look authored, so the classifier cannot promise to catch every
  generated node. The limitation must be documented for users, not hidden.
- The adapter uses nightly compiler APIs. A toolchain or Clippy pin change
  requires rechecking the hygiene functions, the mark order, and the lowering
  evidence before reusing any conclusion in this ADR.
- `AstPass` marks make standard-import injection and test-harness code
  generated. That is intended: those nodes are not authored control flow.
- The Clippy snapshot lags upstream `main`. Monitoring and rollout remain owned
  by issues #561 and #579; this facility must not be used to justify moving the
  pin.
- `classify` is order-insensitive today. It still requires outermost-first
  input because the cursor's direction is part of the contract and a future
  rule may depend on which mark is outermost. `SyntaxContext::marks()` must not
  be substituted for the cursor.

## Reuse rules

- `whitaker_common::node_origin` is the single source of truth for the policy.
  Do not re-derive provenance in a lint with `from_expansion`,
  `source_callsite`, span chains, or a local mark walk.
- The policy module stays free of rustc types and I/O. The adapter lives in
  `crates/whitaker_lint_core/src/hir/node_origin/mod.rs`, depends inward on the
  policy, and is the only place that touches `SyntaxContext`, `ExpnId`, or
  `ExpnKind`.
- A consumer receives an origin and nothing else. It owns traversal and must
  still descend into every expression, including generated wrappers. Generated
  nodes must not update return-adjacency state, and a match guard's origin is
  independent of its enclosing match.
- `recover_user_editable_hir_span` remains a diagnostic facility. It is not a
  provenance classifier and must not be used as one.
- Adding a mark kind requires extending both `ExpansionMark` and the adapter's
  `ExpnKind` match in the same change. The exhaustive match enforces this.
- The permitted consumers are the complexity lints in this repository. A new
  consumer must state which decisions it withholds from generated nodes and add
  cases demonstrating both classes.

## Consequences

Authored code keeps its complexity whether or not rustc lowers it, while macro
bodies and compiler-generated plumbing stop inflating scores. Macro arguments
stay visible. The policy can be tested exhaustively and proved without linking
rustc, and the adapter has one exhaustive match to maintain.

The cost is a nightly-API dependency and a second model to keep in step. That
cost is bounded by the transition table and the mutation evidence, and by
running the proof and adapter tests together on any pin change.

## References

- [Issue #504](https://github.com/leynos/whitaker/issues/504).
- [RFC 0010](../rfcs/rfc-0010-macro-aware-cognitive-complexity.md).
- [Issue #561](https://github.com/leynos/whitaker/issues/561) and
  [issue #579](https://github.com/leynos/whitaker/issues/579).
- [ADR 0009](adr-0009-verified-clippy-snapshot-for-the-dylint-driver-bootstrap.md).
- [Developer's guide](../developers-guide.md).
- [ExecPlan](../execplans/issue-504-macro-aware-cognitive-complexity-rule.md).

# Architectural decision record (ADR) 0021: a pre-expansion pass for `missing_docs_in_test_functions`

## Status

Accepted (2026-11-23): Implement `missing_docs_in_test_functions` as a Dylint
pre-expansion pass, decide candidacy in a compiler-free policy kernel, track
the enclosing item tree with a balanced container stack, and register the lint
as experimental under [ADR 0014](adr-0014-per-lint-experimental-opt-in.md)
until the configured path gate replaces the interim one.

## Date

2026-11-23.

## Context and problem statement

Procedural attribute macros used by test frameworks rewrite the function they
decorate. `#[rstest]` and `rstest-bdd`'s `#[given(...)]` both consume the
authored item during expansion and emit wrappers, scenario descriptors, and
harness machinery. A post-expansion lint cannot reliably tell the authored
function from what the macro generated, so a documentation policy over test
functions has no compiler-native check at that phase. A text scanner would need
to recreate Rust parsing, source mapping, configuration, and diagnostics
outside the compiler.

[RFC 0003](../rfcs/rfc-0003-missing-docs-in-test-functions.md) asks for a
Whitaker lint that closes that gap by running over the parsed abstract syntax
tree before any macro expands, and by diagnosing only source-authored functions
in a test-harness compilation.

Roadmap item 2.2.11 delivers the pass and its candidate-selection predicate.
The configured include/exclude path policy (2.2.12), promotion into the default
suite (2.2.13), translations (2.2.14), the SARIF-contract UI matrix (2.2.15),
and path-configuration property tests (2.2.16) are later items.

## Decision drivers

- The pass must see the authored function with its own attributes and doc
  comments, and nothing a macro generated.
- The candidacy rule must be testable without linking the compiler, because the
  interesting failures are in the rule, not in the plumbing.
- The diagnostic must land on the function's identifier, which is where a
  reader looks, and must degrade to the item span rather than vanish if the
  identifier span is unusable.
- No repository may gain a new diagnostic merely because the lint exists:
  RFC 0003 §10 rejects requiring documentation for every `--test` function.
- The intervening roadmap items must be able to replace pieces without
  re-litigating the shape of the pass.

## Options considered

### Option A: a late (HIR) pass

Would have normal HIR infrastructure and typed node access. Rejected: expansion
has already rewritten the authored function, which is the whole problem.

### Option B: a post-expansion early pass

An `EarlyLintPass` registered with `register_early_pass` runs on the expanded
AST. Rejected for the same reason as option A, and it would additionally see
every generated wrapper as an ordinary item.

### Option C: a pre-expansion pass (chosen)

`dylint_linting::impl_pre_expansion_lint!` registers a pass that rustc runs
over the parsed AST before expansion, once for the crate root and once more for
each out-of-line module file as expansion loads it.

### Option D: a rustdoc JSON consumer or a text scanner

Both would work outside the compiler. Rejected: rustdoc JSON is unstable, needs
a second toolchain workflow, and exposes a different item model, while a
scanner would recreate parsing, mapping, and diagnostics.

## Decision outcome

### Pass shape

The lint is declared with `impl_pre_expansion_lint!`. Its hooks are the paired
`check_item`/`check_item_post`, `check_impl_item`/`check_impl_item_post`, and
`check_trait_item`/`check_trait_item_post`. Rustc calls each pair
unconditionally around the walk of that node, so a container stack that pushes
exactly one frame in every pre-hook and pops exactly one in every post-hook is
balanced by construction. A mismatch is logged, never asserted: a lint must not
abort the user's build.

Module-file runs walk only that module's items and never call `check_crate`, so
the pass keeps no per-crate state. The localizer is a process-wide `OnceLock`
rather than something initialized in `check_crate`.

### Candidate selection

The decision is a pure function over six facts — harness, provenance, context,
body, path, documentation — in a public `policy` module with no `rustc_*`
dependency. Gates are evaluated in that order and the first failing gate names
the skip reason, which makes precedence a property rather than an accident of
statement order.

Context is `Module`, `InherentImpl`, `Trait`, `TraitImpl`, or `Nested`. In
scope are free functions whose every enclosing item is a module, inherent
associated functions in the same position, and trait methods with a default
body. Methods in trait implementations are out of scope because their
documentation conventionally lives on the trait; rustc's own `missing_docs`
makes the same exception. Bodiless declarations are out of scope because there
is nothing to document.

### Documentation rule

Any attribute whose shape is a sugared doc comment, a `doc` attribute with an
`=` value, or a `cfg_attr` whose list carries a `doc = ...` entry satisfies the
gate. Classification is by shape, not by resolved value, so
`#[doc = include_str!("...")]` counts even though its value is an unexpanded
macro call. List-form `#[doc(hidden)]` does not count. Empty documentation
counts as present, following `missing_docs`.

### Interim path gate

RFC 0003 §6 makes the source path the third clause of the predicate, matched
after the source map has recovered a real file path. Recovery is permanent: a
`SourceFile` is `Real(Utf8PathBuf)` when the source map yields a local path and
`Virtual` otherwise, and a virtual source is never selected.

Until 2.2.12 adds configured include and exclude patterns, a crate-private
`PathPolicy::InterimUnrestricted` selects every real file. That deliberately
reproduces the alternative RFC 0003 §10 rejects — requiring documentation for
every `--test` function — for one roadmap step, because 2.2.13, the first
shipping item, requires 2.2.12. The debt is made visible rather than hidden:
the variant is named for its transience, and the fixture
`fail_library_source_interim.rs` records the behaviour and records that 2.2.12
inverts it. 2.2.12 must make an absent or empty `include` inert, which answers
RFC 0003 §11 open question 3.

### Experimental registration

The user approved the interim gate on condition that the lint is not selected
by default. [ADR 0014](adr-0014-per-lint-experimental-opt-in.md) is the
repository's only definition of "experimental", and its tests pin the shared
list, the installer list, and the suite's pass table to one another, so marking
the lint experimental is suite registration.

The lint is compiled into every suite, declared in `SUITE_LINT_DECLS`, and
given a `PassKind::PreExpansion` descriptor; its pass is registered only through
`experimental_passes()`, which `register_suite_lints` filters by the
repository's `[whitaker] experimental` list. A repository that does not list
the lint gets a declared, passless lint: it cannot run, and `#[allow]` naming
it stays silent. Promotion to the default suite is 2.2.13 and must follow
2.2.12.

### Verification split

The selection contract is discharged by exhaustive enumeration: six fields with
160 records, checked against an independently written conjunction and an
ordered gate scan, plus a snapshot of the full truth table. The container stack
is checked by a property test against a recursive tree oracle over generated
item trees. Diagnostic placement, the documentation shapes, and the hooks
themselves are checked by UI fixtures compiled with and without `--test`.

No Verus or Kani artefact is added. Three of four design reviewers judged that
a proof here would restate its specification: the selection domain is finite,
so running the real code over all 160 records is itself exhaustive, and the
stack proof as first sketched defined the open-container set by the same rule
as the implementation. The first substantive proof obligation in this feature
is 2.2.12's anti-widening lemma over unbounded pattern lists, where the
quantified domain genuinely outruns enumeration. This is a recorded deviation
from the `AGENTS.md` preference for Verus on introduced lemmas, approved by the
user on 2026-11-21.

### Soft deprecation and the exit plan

Rustc documents `register_pre_expansion_pass` as "softly deprecated", and
Dylint 6.3.0 already switches to a renamed registration method for compilers
from 2026-06-22. A future toolchain bump could remove the interface the RFC
mandates. The exit plan, if that happens, is to move to a post-expansion early
pass that treats a function as authored when its identifier span has the root
syntax context — attribute macros that re-emit the authored item usually
preserve the identifier's span — accepting lower precision for macros that
rename or synthesize functions. While the lint is experimental, a toolchain
bump that removes the interface may delete its suite entry and dependency
without breaking any consumer contract, because no repository selects it by
default. The developers' guide adds this crate to the toolchain-bump checklist.

## Consequences

- The crate is the suite's first pre-expansion lint, so its conventions are
  written down in the developers' guide and in the design document.
- Roadmap item 2.2.13 becomes promotion from experimental to default rather
  than first registration, and must wait for 2.2.12.
- The lint is inert for a plain `cargo dylint` run over a `cargo check` with no
  test targets, and for `harness = false` targets and doctests. The users'
  guide states that `--all-targets` or `--tests` must follow `--`, because
  `cargo dylint` accepts no such flag itself.
- The lint has no runtime observability channel. A Dylint lint is a `cdylib`
  loaded under `RTLD_LOCAL`, so its `log` and `tracing` statics are private and
  any event it emits is discarded, as ADR 0015 records for the brain-trust
  crates. The pass therefore carries no logging. The container invariant is
  held by the stack's unit tests, the independent tree-oracle property test,
  and the fixtures whose blessed diagnostics differ per enclosing container
  kind, and the users' guide describes the gates a reader should check instead
  of promising debug output.
- `#[expect(missing_docs_in_test_functions)]` on a procedural-attribute function
  is not the supported suppression: an expectation is recorded against
  attribute identifiers, which macro output does not preserve. The users' guide
  prescribes the two-attribute `allow` workaround only, and the measured
  behaviour feeds the suppression matrix in 2.2.15.

## References

- [RFC 0003](../rfcs/rfc-0003-missing-docs-in-test-functions.md).
- [ADR 0014](adr-0014-per-lint-experimental-opt-in.md).
- [ADR 0002](adr-0002-attribute-macro-for-conditional-dylint-expect.md), for the
  pre-expansion `cfg_attr(dylint_lib = ...)` limitation.
- [Developers' guide](../developers-guide.md), "Writing a pre-expansion pass".
- [Suite design](../whitaker-dylint-suite-design.md), the
  `missing_docs_in_test_functions` section and the early-pass registration
  addendum.

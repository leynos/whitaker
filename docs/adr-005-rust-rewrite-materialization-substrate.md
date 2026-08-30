# Architectural decision record (ADR) 005: Rust source rewrite substrate

## Status

Proposed. This ADR authorizes a bounded comparative spike. It does not select a
production rewrite backend until the spike evidence has been reviewed.

## Date

2026-08-30.

## Context and problem statement

Whitaker's proposed rewrite pipeline in [RFC 0003][rfc-0003],
[RFC 0004][rfc-0004], and [RFC 0005][rfc-0005] separates semantic detection,
source edit materialization, compiler validation, and transactional
application. Dylint rules identify an ownership or borrowing shape and emit a
versioned `RewriteIntent`. A materializer must then turn that intent into one or
more precise `TextEditPlan` values before the compiler-validation and
application stages can proceed.

The materializer is not responsible for deciding whether two source expressions
refer to the same Rust place, whether a value escapes, or whether Polonius makes
a proposed borrow valid. Rust compiler analysis and the rewrite checker own
those questions. The materializer is responsible for a different, deceptively
sharp problem:

- locate the syntax identified by semantic anchors;
- revalidate the expected local source shape;
- construct replacement Rust syntax;
- preserve comments, attributes, whitespace, and line endings outside the
  declared edit;
- support edits spanning several statements or files;
- return deterministic, non-overlapping text edits; and
- reject stale or structurally ambiguous input rather than guessing.

Two existing Rust libraries appear suitable as the foundation:

1. `ast-grep-core` with the Rust support from `ast-grep-language`, using
   tree-sitter patterns, metavariables, and replacement templates; and
2. `ra_ap_syntax`, using rust-analyzer's full-fidelity Rust concrete syntax tree
   (CST), typed abstract syntax tree (AST) wrappers, and `SyntaxEditor`.

The first option offers the most direct "template with holes" programming model.
The second offers a Rust-specific, full-fidelity syntax model and structured
editing operations. Neither option should be selected from API appearance
alone. Whitaker needs evidence from the rewrites it actually intends to ship.

## Decision drivers

- Preserve every untouched source byte, including comments and line endings.
- Produce small, reviewable diffs rather than regenerate whole files.
- Keep semantic analysis in Dylint and rustc-facing crates rather than infer it
  again from spelling.
- Express local template rewrites without excessive custom syntax plumbing.
- Support statement-range and multi-file transformations required by the
  ownership and borrow-workaround RFCs.
- Reject stale anchors, macro-only spans, and ambiguous syntax safely.
- Keep edit ordering and output deterministic for tests, agents, and CI.
- Minimize Whitaker-owned parsing, trivia, and source-mapping code.
- Bound dependency size, compile-time cost, and API-churn maintenance.
- Retain a backend-independent `RewriteIntent` and `TextEditPlan` model.

## Requirements

### Functional requirements

The selected substrate must allow Whitaker to:

- parse Rust 2024 source and retain all source text;
- resolve a bounded source region from a semantic anchor;
- validate that the resolved syntax has the expected local shape;
- replace an expression, statement, item fragment, or contiguous statement
  range;
- delete syntax without swallowing unrelated comments or punctuation;
- construct an edit group spanning several files;
- preserve source outside declared edit ranges byte-for-byte;
- emit edits rather than write directly to the working tree;
- detect overlapping or contradictory edits before application; and
- report unsupported source shapes without panicking.

### Technical requirements

- The production abstraction must not expose backend-specific node types through
  `RewriteIntent`, `RewriteAlternative`, or `TextEditPlan`.
- Semantic captures must carry compiler-derived identity and source ranges.
  Repeated spelling alone is not sufficient evidence that two expressions are
  equivalent.
- The backend may use structural matching to confirm a local shape, but it must
  not perform unbounded repository-wide discovery for a rewrite intent.
- The same input must produce byte-identical output and edit ordering across
  runs.
- The backend must accept source by immutable string reference and return owned
  edits. It must not retain syntax nodes beyond the materialization call.
- Dependency versions used by the spike must be pinned in `Cargo.lock` and
  recorded in the result report.
- Benchmark results must separate cold dependency compilation from steady-state
  rewrite execution.
- Compiler acceptance remains the responsibility of the rewrite checker. A
  successful syntax edit does not establish semantic equivalence.

## Options considered

### Option A: ast-grep pattern and replacement backend

Use `ast-grep-core` for parsing, traversal, structural matching, metavariable
capture, and replacement, with `ast-grep-language` providing the Rust grammar
and pattern preprocessing.

A materializer would receive a semantic anchor and recipe payload, restrict the
search to the anchored item or statement range, apply a recipe-specific pattern,
and confirm that captured source ranges agree with the compiler-derived
captures. The replacement would use ast-grep metavariables in a template.

A representative recipe might resemble:

```rust,no_run
struct TemplateRecipe {
    pattern: &'static str,
    replacement: &'static str,
}

const REMOVE_BORROW_ONLY_CLONE: TemplateRecipe = TemplateRecipe {
    pattern: "let $TMP = $VALUE.clone();",
    replacement: "",
};
```

The production code would use the Rust API rather than invoking the ast-grep
command-line interface (CLI).

Advantages:

- Metavariables provide a concise, recognizable model for template-shaped
  rewrites.
- The library already supports parsing, structural search, and node
  replacement.[^1]
- Replacement templates naturally reuse source text captured by patterns.[^2]
- The approach may keep simple recipe implementations small and data-oriented.
- Tree-sitter parsing handles incomplete syntax gracefully, although green
  Whitaker inputs should normally be valid Rust.

Disadvantages and risks:

- The matcher is syntactic. Whitaker must bind metavariables back to
  compiler-derived captures to prevent spelling-based false matches.
- A fix normally replaces one target node's text. More complex deletion or
  punctuation handling requires explicit range expansion or additional edit
  planning.[^2]
- Rust-specific typed AST operations are less direct than with a
  Rust-native CST.
- A statement-spanning or signature-changing recipe may need substantial custom
  range and trivia code, eroding the apparent template advantage.
- Whitaker would need an adapter around an API whose documentation emphasizes
  the CLI as the usual consumer.[^1]

### Option B: rust-analyzer syntax tree and `SyntaxEditor` backend

Use `ra_ap_syntax` to parse a full-fidelity Rust CST, navigate typed Rust AST
nodes, and build structured edits with `SyntaxEditor`.

Whitaker already exact-pins `ra_ap_syntax` for clone-detector AST refinement.
Option B may therefore reuse a parser dependency and version-management policy
that the repository already carries. That prior investment is relevant evidence,
not a foregone selection: the spike must still measure the incremental editing
API, adapter complexity, build footprint, and update burden.

A materializer would resolve compiler-derived byte ranges to syntax nodes or
tokens, cast them to expected typed AST nodes, construct replacement fragments,
and use editor operations to replace, insert, or remove syntax. The completed
syntax edit would then be converted into Whitaker's backend-independent text
edits.

A representative backend seam might resemble:

```rust,no_run
trait RustSyntaxBackend {
    fn materialize(
        &self,
        source: &str,
        intent: &RewriteIntent,
    ) -> Result<Vec<TextEdit>, MaterializeError>;
}
```

Advantages:

- `ra_ap_syntax` explicitly provides a full-fidelity representation in which
  every source text can be represented precisely.[^3]
- Typed Rust AST wrappers make recipe preconditions and syntax construction more
  explicit.
- `SyntaxEditor` provides structured editing and syntax mappings inspired by
  Roslyn's editor model.[^4]
- The Rust-specific parser should track current language syntax more closely
  than generic tree-sitter patterns.
- Structured node and token operations may reduce hand-written punctuation and
  trivia handling for complex recipes.

Disadvantages and risks:

- The `ra_ap_*` crates use rapidly moving, synchronized versions and require
  deliberate pinning and update management.
- The public documentation is sparse, so implementation may require reading
  rust-analyzer source and tests.
- `SyntaxEditor` is documented as temporarily built on mutable syntax-tree
  editing, which signals possible API evolution.[^4]
- Template-shaped rewrites may require more Rust code than ast-grep's
  metavariable replacement syntax.
- The dependency graph and cold build cost may be larger.

Screen reader note: The following table compares the two candidate substrates
against Whitaker's decision drivers.

| Topic | Option A: ast-grep | Option B: `ra_ap_syntax` |
| --- | --- | --- |
| Template-shaped local rewrites | Native metavariable model | Requires recipe code or a Whitaker template layer |
| Rust-specific typed syntax | Limited | Strong |
| Full-fidelity source model | Tree-sitter source ranges and captured text | Explicit design property |
| Statement-range edits | Possible with expanded ranges and custom planning | Structured node and token editing |
| Multi-file edits | Composed by Whitaker | Composed by Whitaker |
| Semantic identity | Must be supplied by Whitaker | Must be supplied by Whitaker |
| Trivia and punctuation control | Recipe-dependent | Rust-specific syntax operations available |
| API and version churn | Adapter required | Exact synchronized version pin required |
| Expected simple-recipe effort | Lower | Medium |
| Expected complex-recipe effort | Medium to high | Lower to medium |
| Dependency and cold-build cost | To be measured | To be measured |

_Table 1: Expected trade-offs between ast-grep and `ra_ap_syntax`._

## Decision outcome / proposed direction

Do not select either production backend yet. Run the comparative spike defined
below and select one primary materialization substrate from its evidence.

The spike must preserve the following architectural decisions regardless of its
winner:

- `RewriteIntent`, semantic anchors, and compiler evidence remain independent of
  the syntax backend.
- The selected backend materializes edits only. It does not decide whether a
  candidate is semantically valid or compiler-acceptable.
- Whitaker will support one primary production backend in the first release.
  Maintaining two parsers and two edit semantics would multiply testing and
  source-mapping risk.
- A later backend-neutral template layer may be built over the selected
  substrate if the spike shows that template ergonomics matter. Such a layer
  must not force Whitaker to retain both candidate libraries.
- The losing prototype will not become a production dependency merely because
  the spike code exists.

After the spike report is reviewed, this ADR should be amended and moved to
`Accepted` with one of these outcomes:

- adopt Option A;
- adopt Option B; or
- reject both and open a new RFC for a different substrate.

A combined ast-grep plus `ra_ap_syntax` production architecture is outside this
ADR. It would require separate justification showing that its benefit exceeds
the cost of duplicate parsing, source mapping, dependency maintenance, and test
coverage.

## Comparative spike

### Spike hypotheses

The spike should test the following hypotheses rather than merely demonstrate
that both libraries can replace text:

- **H1:** ast-grep materially reduces implementation complexity for bounded,
  template-shaped rewrites.
- **H2:** `ra_ap_syntax` materially reduces custom range, trivia, and syntax
  reconstruction code for structural and multi-file rewrites.
- **H3:** one candidate can cover both classes well enough that Whitaker
  does not need two production backends.
- **H4:** dependency and runtime costs are small relative to compiler
  validation, but cold build cost may still affect installation and contributor
  workflows.

### Repository placement

The spike should live in a standalone Cargo workspace so neither candidate
becomes part of Whitaker's production dependency graph before the decision.

```plaintext
spikes/rewrite-materialization/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── src/
│   ├── lib.rs
│   ├── backend.rs
│   ├── harness.rs
│   ├── ast_grep_backend.rs
│   └── ra_syntax_backend.rs
├── fixtures/
│   ├── own001-clone-only-borrow/
│   ├── own002-owned-param/
│   ├── bor001-conditional-lookup/
│   ├── bor002-remove-reinsert/
│   └── adversarial/
└── expected/
```

The root Whitaker workspace must not include this directory as a member. A
repository script or Make target may run the spike explicitly.

Generated timing data and temporary rewritten workspaces should remain under
`target/`. The reviewed result should be committed as
`docs/rewrite-materialization-spike-report.md` and linked from
`docs/contents.md` while it remains relevant.

### Shared spike interface

Both prototypes must implement the same small interface and return the same edit
model.

```rust,no_run
pub trait SpikeBackend {
    fn name(&self) -> &'static str;

    fn materialize(
        &self,
        fixture: &FixtureWorkspace,
        intent: &SpikeIntent,
    ) -> Result<TextEditPlan, SpikeError>;
}

pub struct SpikeIntent {
    pub recipe: RecipeId,
    pub files: Vec<FileIntent>,
    pub captures: Vec<SemanticCapture>,
}

pub struct SemanticCapture {
    pub name: String,
    pub path: WorkspaceRelativePath,
    pub base_file_sha256: Sha256Digest,
    pub range: ByteRange,
    pub snippet_sha256: Sha256Digest,
    pub expected_syntax: ExpectedSyntax,
}
```

The fixtures should contain hand-authored semantic captures equivalent to those
a Dylint rule would emit. This prevents the comparison from accidentally
measuring repository-wide pattern discovery. Each backend may rematch or cast
syntax inside the bounded anchor to confirm structural preconditions.

The harness, not either backend, should own:

- file digest verification;
- edit sorting;
- overlap detection;
- edit application;
- unified diff generation;
- compiler invocation;
- timing;
- metric collection; and
- JSON report serialization.

### Primary rewrite fixtures

The spike must implement the same recipes with both backends.

#### Fixture 1: borrow-only local clone removal

Exercise `clone_only_used_by_borrow` with a local clone used at least twice by
shared borrow, including a comment between the binding and first use.

The rewrite must:

- delete the clone binding;
- replace every captured use with the original place;
- preserve the comment and surrounding blank lines;
- avoid changing unrelated formatting; and
- compile under ordinary non-lexical lifetimes (NLL).

This fixture measures simple recipe ergonomics, repeated capture substitution,
and comment ownership.

#### Fixture 2: private owned parameter converted to a borrow

Exercise `owned_param_causes_clone` across at least three files. Change a
private `String` parameter to `&str`, remove `.clone()` at multiple call sites,
and
preserve documentation and attributes on the function.

The rewrite must:

- update the signature;
- update every captured call site atomically;
- avoid changing public APIs;
- preserve imports unless an import becomes genuinely unused;
- compile under NLL; and
- produce deterministic file and edit ordering.

This fixture measures typed item editing, source mapping, and multi-file plan
composition.

#### Fixture 3: conditional lookup rewritten to a direct mutable borrow

Exercise the path-sensitive collection pattern that motivates Polonius. Start
from a green NLL-compatible lookup-then-relookup implementation and rewrite it
to a direct conditional mutable borrow that returns the existing entry or
inserts and returns a default.

The rewrite must:

- replace a contiguous statement or tail-expression region;
- preserve comments inside both control-flow branches;
- produce the expected direct formulation;
- fail under the NLL checker profile where the fixture intentionally exercises
  the known limitation; and
- compile under `-Zpolonius=next` with the same nightly compiler.

This fixture measures non-trivial statement reconstruction while keeping
compiler classification outwith the backend.

#### Fixture 4: remove, mutate, and reinsert collapsed to direct mutation

Exercise the review-required remove/reinsert lint. Replace an owned temporary
and later reinsertion with direct mutable access. Include a nearby early-return
operator or error propagation and a comment attached to the mutation.

The rewrite must:

- remove the extraction and reinsertion statements;
- retain the mutation and its comment;
- preserve expression evaluation order in the emitted source shape;
- compile under the configured checker profile; and
- remain classified as review-required by the surrounding recipe metadata.

This fixture measures statement deletion, trivia retention, and the distinction
between syntactic materialization and semantic policy.

### Adversarial fixtures

The spike must also cover failure and fidelity cases:

- stale whole-file or snippet digest;
- ambiguous syntax at the recorded range;
- an edit originating only from macro expansion;
- overlapping edits from two captures;
- a comment attached to syntax proposed for deletion;
- CRLF line endings;
- Unicode identifiers;
- Rust 2024 syntax in the enclosing item; and
- a structurally valid but unsupported recipe variant.

The required behaviour is deterministic rejection or an exact expected edit.
Neither backend may panic, silently widen the edit range, or rewrite a different
same-spelled expression.

### Validation pipeline

For every backend and fixture, the harness should perform these steps:

1. Verify file and snippet digests.
2. Materialize the edit plan.
3. Validate edit ordering and non-overlap.
4. Repeat materialization 100 times and compare serialized plans byte-for-byte.
5. Apply the plan in an isolated fixture copy.
6. Compare the complete rewritten source with the checked-in expected source.
7. Assert that bytes outside declared edit ranges are unchanged.
8. Run `cargo fmt --check` on the rewritten fixture.
9. Run the expected NLL and Polonius compiler profiles.
10. Record metrics and diagnostic output in a stable JSON report.

Compiler runs verify that both backends emitted the intended program. They must
not compensate for a backend that changes unrelated source or loses comments.

### Mandatory gates

A backend remains eligible only if it:

- passes every primary fixture;
- passes every adversarial fixture with the specified result;
- preserves untouched bytes exactly;
- preserves required comments, attributes, and line endings;
- emits deterministic plans across 100 repetitions;
- rejects stale and ambiguous anchors;
- detects overlap before edit application;
- produces the expected NLL and Polonius compiler outcomes; and
- contains no fixture-specific source-string replacement in the shared harness.

A candidate that fails a mandatory gate cannot win through a better benchmark
score.

### Evidence to collect

The result report should include the following evidence for each backend.

Correctness and fidelity:

- primary and adversarial fixture results;
- exact-output differences;
- comments or trivia requiring custom handling;
- edits outside intended ranges; and
- failure-mode quality.

Implementation effort:

- backend-specific non-test lines of Rust;
- lines and branches per recipe;
- number of custom range, trivia, and punctuation helpers;
- number of backend escape hatches that bypass structured operations; and
- clarity of recipe preconditions in code review.

Maintenance:

- direct and transitive dependency counts;
- public API surface used by the adapter;
- amount of source reading required because documentation is absent;
- expected version-pinning strategy; and
- one trial update to the newest available compatible release at spike review
  time, recording any required code changes.

Performance and footprint:

- clean debug build time;
- incremental debug build time after changing one recipe;
- release binary size;
- median parse and materialization time over the fixture corpus; and
- peak resident memory where the measurement is reliable.

Performance measurements are secondary. Compiler replay will dominate the final
product workflow, and noisy microbenchmark victories must not outweigh fidelity
or maintainability.

### Decision rule

The spike decision should use hard gates first and comparative judgement second.

1. If only one backend passes every mandatory gate, select that backend.
2. If neither backend passes, reject both and return to RFC review.
3. If both pass, select the backend that requires less Whitaker-owned syntax,
   range, trivia, and punctuation machinery across all four primary recipes.
4. If that comparison is close, prefer the backend whose adapter uses a smaller,
   clearer public API surface and whose dependency update trial requires less
   repair.
5. Use build time, binary size, and steady-state speed only as later
   tie-breakers unless one candidate imposes an operationally disproportionate
   cost.

The report may score categories from one to five for readability, but an
aggregate score must not obscure failed gates or substitute false numerical
precision for review.

### Spike exit artefacts

The spike is complete only when it produces:

- both backend implementations;
- checked-in fixtures and expected outputs;
- a reproducible command that runs the full comparison;
- a machine-readable result report;
- a human-readable analysis with the selection recommendation;
- an amended ADR naming the selected option; and
- a cleanup change that removes or archives the losing production prototype.

## Goals and non-goals

Goals:

- Make the backend choice from representative Whitaker rewrites.
- Preserve a backend-independent rewrite model.
- Measure source fidelity and maintenance cost before dependency adoption.
- Select one production substrate for the first rewriter release.
- Leave enough fixtures to regression-test the selected adapter.

Non-goals:

- Implement the complete rewriter described by RFC 0005.
- Discover candidates from real Dylint lints during the spike.
- Prove behavioural equivalence of remove/reinsert or snapshot rewrites.
- Benchmark whole-repository rewrite checking.
- Expose arbitrary user-authored rewrite templates.
- Select the final formatting policy for every future recipe.
- Retain both candidates as permanent dependencies.

## Migration plan

1. Add the standalone spike workspace and shared harness.
2. Add primary and adversarial fixtures with hand-authored semantic captures.
3. Implement Option A without importing `ra_ap_syntax` into its adapter.
4. Implement Option B without importing ast-grep crates into its adapter.
5. Run the full validation and measurement pipeline.
6. Review the report against the mandatory gates and decision rule.
7. Amend this ADR with the selected outcome and rationale.
8. Move reusable fixtures and the winning adapter seam into the implementation
   plan for RFC 0005.
9. Remove the losing backend from production dependency resolution.

## Consequences

Positive consequences:

- Whitaker avoids committing to a rewrite library based on toy examples.
- Both candidates face the same semantic captures, fixtures, edit model, and
  compiler oracle.
- The spike creates reusable regression fixtures for the eventual production
  adapter.
- Selecting one backend limits duplicate parsing and source-mapping complexity.
- The backend-independent model protects the RFC architecture from library
  churn.

Negative consequences:

- The spike duplicates a small amount of prototype work.
- The final rewriter implementation starts only after the evidence is reviewed.
- Some metrics, especially cold build time and memory, will vary by machine and
  require careful interpretation.
- A four-recipe corpus cannot expose every future transformation.

## Known risks and limitations

- The shared interface may favour one backend accidentally. The review should
  identify adapter code forced by the interface rather than by the library.
- Expected-output fixtures can reward overfitting. Adversarial variants and the
  prohibition on fixture-specific harness replacements mitigate this risk.
- Tree-sitter and rust-analyzer may parse edge syntax differently. Compiler
  replay confirms emitted Rust but does not erase source-fidelity differences.
- A dependency update trial samples maintenance cost rather than predicting it.
- Multi-file edits are primarily a Whitaker planner concern. The spike measures
  each backend's ability to materialize per-file edits, not workspace
  transactionality.
- `cargo fmt --check` may reject a mechanically correct minimal edit. The report
  should distinguish malformed output from output that merely needs a later
  formatting phase.
- The Polonius fixture depends on the pinned nightly's current behaviour. The
  fixture and expected profile must be reviewed when the toolchain changes.

## Outstanding decisions

- Select the exact pinned versions of both candidate libraries at spike start.
- Decide whether the spike report should remain a standalone document after the
  ADR is accepted or be condensed into the ADR's rationale.
- Decide whether the selected production adapter belongs in
  `whitaker_rewrite_engine` or a narrower `whitaker_rust_edit` crate.
- Decide whether a backend-neutral template syntax warrants a later RFC after
  the substrate has been selected.

## Architectural rationale

The rewriter's most dangerous failure mode is not an obvious parser error.
It is a plausible diff that targets the wrong same-spelled expression, drops a
comment, widens a deletion, or quietly makes future recipes depend on ad hoc
byte surgery.
The substrate decision therefore deserves evidence from semantic anchors,
statement edits, multi-file changes, trivia, stale input, and Polonius-only
output.

A bounded spike keeps that evidence inexpensive while protecting the larger
rewrite architecture. Selecting one backend after both implementations cross the
same hard gates gives Whitaker a coherent syntax-editing foundation without
letting either a pleasant template syntax or an impressive AST API win by
brochure.

## References

[rfc-0003]: rfcs/0003-compiler-validated-rewrite-checking.md
[rfc-0004]: rfcs/0004-borrow-workaround-lint-family.md
[rfc-0005]: rfcs/0005-validated-ownership-and-borrow-rewriter.md

[^1]: `ast-grep-core` crate documentation, parsing, traversal, search, and
    replacement APIs: <https://docs.rs/ast-grep-core/latest/ast_grep_core/>.

[^2]: ast-grep rewrite documentation, metavariable substitution and expanded
    fix ranges: <https://ast-grep.github.io/guide/rewrite-code.html>.

[^3]: `ra_ap_syntax` crate documentation, full-fidelity Rust CST and typed AST:
    <https://docs.rs/ra_ap_syntax/latest/ra_ap_syntax/>.

[^4]: `ra_ap_syntax::syntax_editor` documentation:
    <https://docs.rs/ra_ap_syntax/latest/ra_ap_syntax/syntax_editor/>.

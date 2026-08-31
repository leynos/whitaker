# RFC 0003: `missing_docs_in_test_functions`

## 1. Preamble

- **RFC number:** 0003
- **Status:** Proposed
- **Created:** 2026-08-30

## 2. Summary

This RFC proposes `missing_docs_in_test_functions`, a Whitaker pre-expansion
lint that requires documentation on source-authored functions in configured
Rust test-source paths. The lint runs only for test-harness compilations, so it
does not impose a documentation policy on normal library or binary targets.

The rule deliberately complements rather than duplicates
`clippy::missing_docs_in_private_items`. Clippy continues to govern ordinary
targets. Whitaker governs the narrower repository-policy invariant that test
functions written in selected source files are documented, including functions
whose procedural attribute macros later generate test machinery.

## 3. Problem

Procedural attribute macros used by test frameworks can transform one
source-written function into a function plus generated wrappers or scenario
machinery. A post-expansion lint cannot reliably distinguish the authored
function from generated artefacts, and a source scanner would need to recreate
Rust parsing, source mapping, configuration, and diagnostic behaviour outside
the compiler.

This produces a documentation-policy gap. In a selected integration-test file,
the following source function lacks a durable, compiler-native check:

```rust
#[given("a runtime alias counter initialized to 0")]
fn runtime_alias_counter_init() {
    // Setup omitted.
}
```

The desired result is a diagnostic on `runtime_alias_counter_init`, before the
`#[given]` macro transforms the item. Documentation makes the test's semantic
role visible without requiring readers to infer it solely from an executable
step expression.

## 4. Current state

Whitaker already uses Dylint 6 and Rust compiler-private crates for its lint
suite. `function_attrs_follow_docs` is an adjacent precedent: it examines
function attributes, recovers source-oriented spans, and emits compiler
diagnostics. The existing suite design also recognizes pre-expansion lints as
the appropriate mechanism when a rule must see attribute-macro syntax before
expansion.[^1]

Clippy's `missing_docs_in_private_items` remains the policy for ordinary
targets. This RFC does not assume that Clippy reports, or consistently omits, a
function re-emitted by an attribute procedural macro. The proposed lint runs
before that question arises.

Cargo may compile ordinary library source with the test harness enabled. A
test-harness check without a source-path boundary would therefore require
documentation for unrelated helpers such as a private function inside a
`#[cfg(test)]` module. The source-path boundary is consequently part of the
policy, not merely an implementation detail.

## 5. Goals and non-goals

- Goals:
  - Enforce documentation for source-authored functions in explicitly selected
    Rust test-source paths.
  - Report diagnostics on the authored function's identifier or item span
    before procedural attribute-macro expansion.
  - Reuse Rust compiler spans, Dylint configuration, localization, and UI
    testing instead of introducing a separate source scanner.
  - Make generated wrappers and other macro-generated functions out of scope.
  - Provide a configuration model that projects can reuse without duplicating
    path-matching conventions in individual lints.
- Non-goals:
  - Reimplement all of Clippy's `missing_docs_in_private_items` semantics.
  - Require documentation for every function compiled under `--test`.
  - Inspect generated functions, wrappers, or other macro expansion output.
  - Generate rustdoc JSON, parse it externally, or convert its results back
    into diagnostics.
  - Change the level of `clippy::missing_docs_in_private_items` for ordinary
    targets.

## 6. Proposed design

### Pass boundary

Implement the rule with Dylint's pre-expansion registration support:
`declare_pre_expansion_lint!` and `impl_pre_expansion_lint!`.[^1] The pass
inspects the Abstract Syntax Tree (AST) before a procedural attribute macro can
consume or re-emit the authored item.

The following pipeline has a single owner for each responsibility:

For screen readers: Whitaker checks source-authored test functions before
procedural macro expansion, and Clippy checks ordinary high-level intermediate
representation items afterwards.

```plaintext
Source Rust
  -> Whitaker: selected, source-authored test functions have documentation
  -> Procedural attribute-macro expansion: step attributes and wrappers
  -> high-level intermediate representation (HIR): Clippy's ordinary private-item documentation policy
```

_Figure 1: Documentation-check ownership across test compilation._

The diagram shows that Whitaker operates on authored test functions before
macro expansion, while Clippy retains responsibility for ordinary HIR items.

For a documented BDD step, the pre-expansion rule observes the exact source
attributes and accepts the function:

```rust
/// Reset the compatibility-alias counter.
#[given("a runtime alias counter initialized to 0")]
fn runtime_alias_counter_init() {
    // Setup omitted.
}
```

The lint MUST skip any item whose span originates in an expansion. Because it
runs before the source item's attribute macro expands, generated wrappers do
not exist from the lint's perspective. The explicit span check remains a
defence against compiler or macro-generated AST items from earlier expansion
stages.

### Candidate selection

The lint MUST return without a diagnostic unless all of the following are true:

1. The compiler session is building a test harness (`sess.opts.test`).
2. The candidate is a source-authored function item, not an expansion-derived
   item.
3. The item source file matches the configured include patterns and does not
   match the configured exclude patterns.
4. The item has no Rust documentation attribute.

The implementation MUST recognize both line documentation comments (`///`) and
the equivalent `#[doc = "..."]` form. It MUST attach the diagnostic to the
function name where a reliable identifier span exists, falling back to the
source item span only when necessary.

The initial implementation should visit free-function AST items and any
equivalent associated-function AST shape supplied by the pinned compiler API.
Its UI matrix MUST state explicitly which source forms are included. Closures,
trait declarations without bodies, and generated items are not in scope.

### Configuration

The lint uses a dedicated `dylint.toml` table. Include and exclude patterns are
matched against workspace-relative, slash-normalized source paths after the
compiler source map has recovered a real file path. Excludes take precedence.
The implementation MUST reject invalid or non-relative patterns with a clear
configuration diagnostic rather than silently widening enforcement.

The following configuration illustrates the intended policy boundary:

```toml
[missing_docs_in_test_functions]
include = [
    "crates/*/tests/**/*.rs",
    "tests/**/*.rs",
]
exclude = [
    "tests/ui/**",
]
```

This configuration includes ordinary integration tests but deliberately leaves
UI fixtures outside the policy. If the project requires a new shared path-glob
helper, its ownership, normalization rules, permitted call sites, and reuse
policy MUST be recorded in the relevant design documentation before the helper
is introduced.

### Diagnostics and suppressions

The primary diagnostic SHOULD say that the named test function requires a
documentation comment, and its help text SHOULD show a `///` comment directly
above the function. It MUST use Whitaker's localized diagnostic conventions.

Pre-expansion lints have a Dylint-specific suppression limitation:
`cfg_attr(dylint_lib = ..., allow(...))` is not available before expansion.
The Dylint 6.0.1 documentation records this limitation and its workaround.
When an individually justified exception is unavoidable, consumers can use the
documented two-attribute workaround:[^2]

```rust
#[allow(unknown_lints)]
#[allow(missing_docs_in_test_functions)]
fn generated_fixture_adapter() {
    // Narrow, documented exception omitted.
}
```

The lint's path configuration should make these local exceptions uncommon.
Consumers MUST keep any exception attached to the smallest relevant item and
document why the function is exempt.

## 7. Requirements

### Functional requirements

- The lint MUST be named `missing_docs_in_test_functions` and be available from
  Whitaker's normal suite distribution.
- It MUST only diagnose source-authored functions in a test-harness
  compilation and a configured included path.
- It MUST honour configured exclusions before inclusions can produce a
  diagnostic.
- It MUST accept `///` and `#[doc = "..."]` documentation.
- It MUST report undocumented plain functions and functions carrying a
  procedural attribute macro at the authored source location.
- It MUST not report expansion-generated functions or functions in excluded
  paths.

### Technical requirements

- The lint MUST use a pre-expansion AST pass, not a Clippy pass, late HIR pass,
  rustdoc JSON parser, or regular-expression scanner.
- Configuration MUST be loaded through Whitaker's established Dylint
  configuration path, with typed defaults and actionable invalid-pattern errors.
- Source-path matching MUST be deterministic across platform path separators
  and must not match outside the workspace by accident.
- Diagnostics, help text, and UI snapshots MUST use Whitaker's localization
  infrastructure.
- The implementation MUST add UI coverage whose fixtures consume pre-baked
  SARIF results as their diagnostic-input contract. The matrix MUST include an
  undocumented function in an included test file that fails at its identifier,
  documented functions, `#[doc = "..."]`, attribute-macro functions, excluded
  paths, test-harness gating, and generated-code immunity.
- The test matrix MUST include the pre-expansion suppression workaround so its
  compiler behaviour remains understood and intentional.

## 8. Compatibility and migration

The lint is opt-in through `dylint.toml`; adding it does not change existing
Whitaker users until they select source paths. Projects should begin with their
integration-test directories, resolve genuine missing documentation, and keep
test fixtures or intentionally terse generated-support files in narrow exclude
patterns.

The `rstest-bdd` adoption described by issue 684 should follow these steps:

1. Upgrade its Whitaker installation or dependency pin to a release containing
   this lint.
2. Configure its root integration-test paths in `dylint.toml`.
3. Keep `clippy::missing_docs_in_private_items = "deny"` unchanged for normal
   targets.
4. Invoke Whitaker separately for the intentionally independent published-GPUI
   workspace, because root Cargo commands do not traverse that workspace.
5. Remove the proposed external source-scanner work once the compiler-native
   path supplies the required diagnostics.

This staged adoption is intentionally separate from the generic lint's first
implementation. It prevents a downstream workspace layout from becoming part of
Whitaker's reusable configuration contract.

## 9. Test plan

The lint crate's UI fixtures should exercise the full decision boundary:

- UI fixtures consume pre-baked SARIF results as their diagnostic-input
  contract. An undocumented function in an included test file fails at its
  identifier.
- Under that same contract, an undocumented compiler-supported associated
  function in an included test file fails at its function identifier.
- A `///`-documented function and a `#[doc = "..."]`-documented function pass.
- An undocumented function with a representative procedural attribute macro
  fails before expansion, while its generated wrapper is never diagnosed.
- A file matching an exclusion passes despite containing an undocumented
  function.
- A function in a normal library source file passes even when that library is
  compiled for a test harness.
- An item emitted by a macro passes, proving the expansion-span guard.
- A dedicated suppression fixture contains an undocumented function annotated
  with both `#[allow(unknown_lints)]` and
  `#[allow(missing_docs_in_test_functions)]`, plus an unsuppressed undocumented
  control function. The expected UI output shows that the two-attribute
  workaround suppresses `missing_docs_in_test_functions` only for the annotated
  item.

The implementation MUST include a substantive Rust `proptest` suite for the
pure candidate-selection and path-configuration logic. Generated combinations
MUST cover test-harness status, expansion provenance, documentation presence,
include and exclude matches, slash and backslash separators, and valid and
invalid patterns. Properties MUST assert the diagnostic predicate, exclusion
precedence, separator normalization, and rejection of invalid patterns without
widening the configured workspace scope.

The implementation PR must run the repository's formatting, lint, test,
Markdown, and Mermaid validation gates. Downstream adoption must separately
prove the root integration-test invocation and the isolated published-GPUI
workspace invocation.

## 10. Alternatives considered

| Option                                   | Advantages                                                                          | Rejected because                                                                             |
| ---------------------------------------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Pre-expansion Whitaker AST lint          | Sees authored attributes, uses compiler spans, and ignores later generated wrappers | Selected                                                                                     |
| Clone Clippy's `LateLintPass`            | Has normal HIR infrastructure                                                       | Expansion has already obscured the source boundary needed for attribute-macro test functions |
| External source scanner                  | Can scan files without compiler-private APIs                                        | Recreates parsing, path resolution, configuration, and diagnostics outside rustc             |
| Rustdoc JSON checker                     | Exposes documentation and optional spans                                            | Requires nightly orchestration, JSON processing, and a separate diagnostic channel           |
| Require docs for every `--test` function | Simple predicate                                                                    | Incorrectly includes unrelated unit-test helpers throughout normal source trees              |

_Table 1: Alternatives for enforcing documentation on selected test functions._

An external rustdoc JSON design is principled: its item representation exposes
documentation and may expose a source span.[^3] It remains an unstable rustdoc
feature, however, and would require a second toolchain workflow to generate,
parse, filter, and report results.[^4] That is disproportionate when Whitaker
already provides the compiler-semantic policy layer.

## 11. Open questions

- Which glob library and exact pattern grammar should Whitaker standardize for
  reusable workspace-relative source-path policies?
- Does the pinned compiler AST expose associated functions through a distinct
  pre-expansion callback that should be included in the first release?
- Should an empty `include` list disable the lint, or should configuration
  loading reject it to prevent a misleading enabled-but-inert policy?
- Which release channel should make the lint available to downstream projects,
  and what minimum Whitaker version should `rstest-bdd` pin?

## 12. Recommendation

Accept `missing_docs_in_test_functions` as a pre-expansion Whitaker lint with
test-harness and configured-path gates. It preserves Clippy's ordinary-target
responsibility, gives procedural-attribute test functions a direct,
source-accurate diagnostic, and avoids building a parallel parser or rustdoc
JSON pipeline for a policy Whitaker is designed to enforce.

[^1]: [Dylint linting README](https://docs.rs/crate/dylint_linting/latest/source/README.md)
  documents Dylint's pre-expansion lint support.
[^2]: [Dylint 6.0.1 documentation](https://docs.rs/crate/dylint/6.0.1) documents
  the pre-expansion lint suppression workaround.
[^3]: [Rust's `rustdoc_json_types::Item`](https://doc.rust-lang.org/nightly/nightly-rustc/rustdoc_json_types/struct.Item.html)
  documents the JSON item's documentation and optional span fields.
[^4]: [Rustdoc unstable features](https://doc.rust-lang.org/rustdoc/unstable-features.html)
  documents rustdoc JSON as an unstable feature.

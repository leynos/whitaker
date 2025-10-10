# Whitaker Rust Dylint Workspace — Lint Suite Design and Roadmap

This single document consolidates the workspace design, the seven core lints
(with the **revised conditional rule**), the separate `no_unwrap_or_else_panic`
lint crate, a feasibility study for a **Bumpy Road** detector, and a phased
**roadmap**.

## 0) Objectives

- Provide seven Dylint rules as separate crates, plus an optional **aggregated
  suite** crate.
- Keep configuration simple via `dylint.toml` and workspace metadata.
- Offer robust UI tests and CI.
- Add an extra restriction lint: **`no_unwrap_or_else_panic`**.
- Explore an advanced maintainability signal (**Bumpy Road**) as an
  experimental lint.

## 1) Workspace layout

```text
<repo-root>/
├─ Cargo.toml                     # workspace + dylint metadata
├─ rust-toolchain.toml            # nightly pinned for lints
├─ dylint.toml                    # optional defaults per lint
├─ crates/
│  ├─ function_attrs_follow_docs/
│  ├─ no_expect_outside_tests/
│  ├─ public_fn_must_have_docs/
│  ├─ module_must_have_inner_docs/
│  ├─ conditional_max_two_branches/   # revised: complex conditional detection
│  ├─ test_must_not_have_example/
│  ├─ module_max_400_lines/
│  └─ no_unwrap_or_else_panic/        # separate crate
├─ suite/                         # aggregated dylint library (optional)
├─ installer/                     # optional convenience binary
├─ common/                        # shared helpers for lints
└─ ci/                            # CI workflows
```

### Top-level `Cargo.toml`

```toml
[workspace]
members = ["crates/*", "suite", "installer", "common"]
resolver = "2"

[workspace.package]
edition = "2021"

[workspace.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = ["cfg(dylint_lib, values(any()))"]

[workspace.dependencies]
dylint_linting = "4"
rustc_hir = { package = "rustc-hir", version = "*" }
rustc_lint = { package = "rustc-lint", version = "*" }
rustc_middle = { package = "rustc-middle", version = "*" }
rustc_session = { package = "rustc-session", version = "*" }
rustc_span = { package = "rustc-span", version = "*" }
clippy_utils = { version = "*", optional = true }

[workspace.metadata.dylint]
libraries = [ { git = "https://example.com/your/repo.git", pattern = "crates/*" } ]
```

> Pin a nightly in `rust-toolchain.toml` aligned with the targeted
> `dylint_linting` version.

## 2) Common crate (`common`)

Utilities shared by lints:

- **Attribute helpers:** doc vs non-doc; inner vs outer; detect `#[test]`-like
  attributes (`test`, `tokio::test`, `rstest`).
- **Context:** `in_test_like_context`, `is_test_fn`, `is_in_main_fn`.
- **Types:** `recv_is_option_or_result`.
- **Spans:** `span_to_lines`, `span_line_count`, `def_id_of_expr_callee`,
  `is_path_to`.
- **Visibility:** effective export check via `cx.tcx`/`effective_visibilities`.
- **Diagnostics:** `span_lint`, formatting helpers, suggestion utilities.

### Implementation notes — Phase 1 delivery

- Adopted lightweight domain models rather than compiling against the unstable
  `rustc_*` crates. `Attribute`, `ContextEntry`, `SimplePath`, and `Expr`
  encode the data needed by early lints without tying the helpers to a specific
  compiler snapshot.
- Attribute helpers normalize paths into `Vec<String>` segments, allowing
  reusable matching logic for doc comments and test-like markers. This ensures
  future lints can extend the recognized attribute set without restructuring
  the API.
- Context detection operates on an explicit stack of `ContextEntry` frames. The
  helpers analyze the recorded attributes so callers can reason about ambient
  test contexts without leaking traversal state.
- Span utilities introduce `SourceLocation`/`SourceSpan` wrappers with
  validation, providing deterministic line counting and range projection for
  diagnostics while flagging inverted spans early.
- Diagnostics are constructed via a builder (`span_lint`) that gathers notes,
  help messages, and suggestions before emitting a concrete `Diagnostic`. The
  structure mirrors `rustc` concepts but keeps the surface area simple for unit
  and behaviour tests.
- UI test harness helpers live in `whitaker::testing::ui`. The helpers validate
  crate names and UI directories before invoking `dylint_testing::ui_test`, and
  expose `run_ui_tests!` plus `declare_ui_tests!` macros. The macros expand
  `env!("CARGO_PKG_NAME")` in the caller so dependent lint crates always pass
  their own names to the harness while publishing the canonical `ui` test
  without copying boilerplate. Tests inject stub runners via `run_with_runner`
  to cover happy and unhappy paths without touching the real filesystem.
- Path handling standardises on a caret requirement anchored at `camino`
  v1.1.10. Transitive constraints currently resolve this to 1.2.1. This keeps
  the workspace benefiting from the maintenance fixes delivered since 1.1.6,
  including the `unexpected_cfgs`-warning resolution needed for lint workspaces.
- Shared configuration lives in `whitaker::config::SharedConfig`. The
  `load()` helper uses the Dylint loader for Whitaker itself, while
  `load_with()` accepts the caller's crate name plus an injectable loader so
  individual lint crates can read their matching tables in `dylint.toml` or
  tests can stub the source. `serde` defaults keep fields optional so teams can
  override only the `module_max_400_lines.max_lines` threshold (default 400)
  without rewriting the table. Unknown fields are rejected via
  `deny_unknown_fields` so configuration typos fail fast during deserialization.
- Unit and behaviour coverage lean on `rstest` fixtures and `rstest-bdd`
  scenarios (v0.1.0-alpha4) to exercise happy, unhappy, and edge cases without
  duplicating setup logic.

## 3) Seven core lints (specs + sketches)

| Crate                         | Kind            | Summary                                                                                                                | Level |
| ----------------------------- | --------------- | ---------------------------------------------------------------------------------------------------------------------- | ----- |
| `function_attrs_follow_docs`  | style           | Outer doc comments on functions must precede other outer attributes.                                                   | warn  |
| `no_expect_outside_tests`     | restriction     | Ban `.expect(..)` on `Option`/`Result` outside test/doctest contexts (per effective visibility of the enclosing item). | deny  |
| `public_fn_must_have_docs`    | pedantic        | Publicly exported functions require at least one outer doc comment.                                                    | warn  |
| `module_must_have_inner_docs` | pedantic        | Every module must open with a `//!` inner doc comment.                                                                 | warn  |
| `test_must_not_have_example`  | style           | Test functions (e.g. `#[test]`, `#[tokio::test]`) must not ship example blocks or `# Examples` headings in docs.       | warn  |
| `module_max_400_lines`        | maintainability | Flag modules whose span exceeds 400 lines; encourage decomposition or submodules.                                      | warn  |

### Per-lint crate scaffolding

Each lint crate is a `cdylib` exposing a single lint. The shared structure
keeps dependencies aligned and ensures UI tests run uniformly.

```toml
[package]
name = "function_attrs_follow_docs"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
dylint_linting = { workspace = true }
rustc_hir = { workspace = true }
rustc_lint = { workspace = true }
rustc_middle = { workspace = true }
rustc_session = { workspace = true }
rustc_span = { workspace = true }
common = { path = "../../common" }

[dev-dependencies]
dylint_testing = "4"
```

> Swap the `name` per crate. Tests live under `tests/ui` with `dylint_testing`
> providing the harness.

### 3.1 `function_attrs_follow_docs` (style, warn)

Ensure function doc comments precede other **outer** attributes.

Sketch:

```rust
use dylint_linting::{declare_late_lint, impl_late_lint};
use rustc_hir as hir; use rustc_lint::{LateContext, LateLintPass};

declare_late_lint!(pub FUNCTION_ATTRS_FOLLOW_DOCS, Warn, "function attributes must follow doc comments");

pub struct Pass;
impl_late_lint! { FUNCTION_ATTRS_FOLLOW_DOCS, Pass,
  fn check_item<'tcx>(&mut self, cx: &LateContext<'tcx>, it: &'tcx hir::Item<'tcx>) {
    if let hir::ItemKind::Fn(..) = it.kind {
      let attrs = cx.tcx.hir().attrs(it.hir_id());
      common::check_doc_then_attrs(cx, it.span, attrs, "functions");
    }
  }
  fn check_impl_item<'tcx>(&mut self, cx: &LateContext<'tcx>, it: &'tcx hir::ImplItem<'tcx>) {
    if let hir::ImplItemKind::Fn(..) = it.kind {
      let attrs = cx.tcx.hir().attrs(it.hir_id());
      common::check_doc_then_attrs(cx, it.span, attrs, "methods");
    }
  }
}
```

### 3.2 `no_expect_outside_tests` (restriction, deny)

Forbid `.expect(..)` on `Option`/`Result` outside tests/doctests.

Sketch:

```rust
use rustc_hir::{Expr, ExprKind};

declare_late_lint!(pub NO_EXPECT_OUTSIDE_TESTS, Deny, ".expect() must not be used outside of test or doctest");
impl_late_lint! { NO_EXPECT_OUTSIDE_TESTS, Pass,
  fn check_expr<'tcx>(&mut self, cx: &LateContext<'tcx>, e: &'tcx Expr<'tcx>) {
    if let ExprKind::MethodCall(seg, recv, ..) = e.kind {
      if seg.ident.name.as_str() == "expect"
        && common::recv_is_option_or_result(cx, recv)
        && !common::in_test_like_context(cx, e.hir_id) {
        common::span_lint(cx, NO_EXPECT_OUTSIDE_TESTS, e.span, "`.expect(..)` outside tests");
      }
    }
  }
}
```

### 3.3 `public_fn_must_have_docs` (pedantic, warn)

Any effectively exported function must have a doc comment.

Sketch uses `effective_visibilities` and `has_outer_doc`.

### 3.4 `module_must_have_inner_docs` (pedantic, warn)

Every module must start with an inner doc `//!`.

Sketch checks inner attributes on `ItemKind::Mod`.

### 3.5 `conditional_max_two_branches` (style, warn)

Reinterpreted as a **complex conditional detector**. The crate name remains
`conditional_max_two_branches` for backwards compatibility, with a future alias
(`decompose_complex_conditional`) under consideration.

**Intent.** Discourage complex boolean predicates inside `if`, `while`, and
match guard conditions. Inline expressions such as
`if x.started() && y.running()` obscure the business rule and contribute to the
Complex Method smell. Encourage encapsulation via a well-named helper or a
local variable.

**Rationale.** Teams often accumulate guard clauses over time by bolting
additional `&&`/`||`/`!` terms into the conditional. The logic becomes
entangled with control-flow, harming readability and reuse. Extracting the
predicate makes the rule explicit, improves testability, and reduces accidental
duplication.

**How to fix.** Apply the *Decompose Conditional* refactoring. Prefer
extracting the predicate into a function with a domain-flavoured name. When a
function is overkill, bind the expression to a local variable and branch on
that name.

**Lint metadata.**

- Crate: `conditional_max_two_branches` (alias rename TBD).
- Kind: `style`.
- Default level: `warn`.
- Escape hatch: `#[allow(conditional_max_two_branches)]`.

**Detection model.** A *complex conditional* is any boolean-valued expression
in a branching position that contains two or more predicate atoms. An atom is a
boolean leaf (comparisons, boolean-returning calls, boolean identifiers, etc.).
Logical connectives (`&&`, `||`, `!`) form the internal nodes of the predicate
tree.

**Positions checked.**

- `if <cond> { … }` where `<cond>` is not an `ExprKind::Let` (i.e. exclude `if
  let`).
- `while <cond> { … }` with the same exclusion for `while let`.
- `match` guards represented in HIR as `Guard::If(<cond>)`.

**Algorithm.** Traverse the HIR expression and compute the number of atoms:

```text
atoms(e) =
  if e is Binary(And|Or, lhs, rhs): atoms(lhs) + atoms(rhs)
  if e is Unary(Not, inner):         atoms(inner)
  else:                              1
```

Emit a diagnostic when `atoms(e) >= max_atoms`, where the default `max_atoms`
is `1` (flag any conjunction/disjunction).

**Implementation sketch (`src/lib.rs`).**

```rust
use dylint_linting::{declare_late_lint, impl_late_lint};
use rustc_hir as hir;
use rustc_hir::{BinOpKind, Expr, ExprKind, Guard, UnOp};
use rustc_lint::{LateContext, LateLintPass};

declare_late_lint!(
    pub CONDITIONAL_MAX_TWO_BRANCHES,
    Warn,
    "complex conditional in a branch; decompose or extract"
);

pub struct Pass;

impl_late_lint! {
    CONDITIONAL_MAX_TWO_BRANCHES,
    Pass,

    fn check_expr<'tcx>(&mut self, cx: &LateContext<'tcx>, e: &'tcx Expr<'tcx>) {
        match e.kind {
            ExprKind::If(cond, ..) | ExprKind::While(cond, ..) => {
                if !matches!(cond.kind, ExprKind::Let(..)) && atoms(cond) >= 2 {
                    common::span_lint(
                        cx,
                        CONDITIONAL_MAX_TWO_BRANCHES,
                        cond.span,
                        "complex conditional; extract to a named predicate or local variable",
                    );
                    // TODO: introduce multipart suggestion to name the predicate.
                }
            }
            ExprKind::Match(_, arms, _) => {
                for arm in *arms {
                    if let Some(Guard::If(guard)) = arm.guard {
                        if atoms(guard) >= 2 {
                            common::span_lint(
                                cx,
                                CONDITIONAL_MAX_TWO_BRANCHES,
                                guard.span,
                                "complex match guard; extract to a predicate",
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn atoms(e: &Expr<'_>) -> usize {
    match e.kind {
        ExprKind::Binary(op, lhs, rhs)
            if matches!(op.node, BinOpKind::And | BinOpKind::Or) =>
        {
            atoms(lhs) + atoms(rhs)
        }
        ExprKind::Unary(UnOp::Not, inner) => atoms(inner),
        ExprKind::Binary(op, ..)
            if matches!(
                op.node,
                BinOpKind::Eq | BinOpKind::Ne | BinOpKind::Lt | BinOpKind::Le | BinOpKind::Gt | BinOpKind::Ge
            ) =>
        {
            1
        }
        _ => 1,
    }
}
```

**Notes.** Parentheses are normalised away by HIR, so grouping does not affect
the atom count. Bitwise operators (`&`, `|`, `^`) are ignored unless they feed
a boolean context via casts. `if let`/`while let` are intentionally excluded
because they are matching patterns, not boolean predicates.

**Diagnostics.**

- Message: “Complex conditional; encapsulate the predicate in a well-named
  function or bind it to a local variable.”
- Note (why): “Complex conditionals hinder readability and contribute to the
  Complex Method smell; decompose the conditional to clarify the rule.”
- Suggestions:
  - Local binding (multipart, `Applicability::MaybeIncorrect`) that introduces
    `let <name> = <cond>;` immediately prior to the statement and swaps the
    condition for `<name>`.
  - Function extraction sketch describing `fn <meaningful_name>(…) -> bool {
    <cond> }` and updating the branch to call it.

**Configuration.**

```rust
#[derive(serde::Deserialize)]
struct Config {
    /// Maximum predicate atoms allowed in a branch condition. Default: 1.
    max_atoms: Option<usize>,
}
```

Read via `dylint_linting::config_or_default` and honour crate-level overrides
in `dylint.toml`.

**False positives / limitations.**

- Intentional short-circuiting (e.g. defensive double checks) may produce
  acceptable two-atom predicates; users can increase `max_atoms` or allow the
  lint locally.
- Predicate name inference in suggestions is non-trivial. Favour clear
  diagnostic text over brittle guesses; use placeholders such as
  `/* predicate */` when necessary.
- Extracting to a function must preserve ownership and short-circuit semantics;
  prefer local binding when moves or borrows complicate extraction.

**UI tests.**

```text
crates/conditional_max_two_branches/tests/ui/
├─ simple_and_bad.rs        # `if a() && b()` → warn
├─ simple_ok.rs             # `if is_ready()` → ok
├─ comparison_ok.rs         # `if x > 0` → ok
├─ three_atoms_bad.rs       # `if a() || b() || c()` → warn
├─ while_guard_bad.rs       # `while p() && !q()` → warn
├─ match_guard_bad.rs       # match guard with `&&` → warn
├─ if_let_excluded.rs       # `if let Some(_) = x` → ok
└─ macros_edge.rs           # `assert!(a && b)` currently ignored
```

Each `.rs` pairs with a `.stderr` expectation via `dylint_testing::ui_test`.

### 3.6 `test_must_not_have_example` (style, warn)

Forbid examples or fenced code blocks in `#[test]` docs.

Heuristic: detect Markdown `# Examples` heading or fenced code (``` / ```rust)
in collected doc text.

### 3.7 `module_max_400_lines` (maintainability, warn)

Lint when module span exceeds 400 lines. Configurable via `max_lines`.

## 4) Additional restriction lint (separate crate): `no_unwrap_or_else_panic`

Separate Dylint crate forbidding `unwrap_or_else(..)` closures that panic
outside tests or doctests. Suits teams enforcing “no panics in production”
policies.

### Intent

Discourage disguising panics as error handling. Expressions such as
`maybe.unwrap_or_else(|e| panic!("{e:?}"))` should propagate errors rather than
crashing. Provide a diagnostic nudging developers toward structured error
returns.

### Crate layout

```text
crates/no_unwrap_or_else_panic/
├─ Cargo.toml
└─ src/
   └─ lib.rs

crates/no_unwrap_or_else_panic/tests/ui/
  ├─ ok_map_err.rs
  ├─ bad_unwrap_or_else_panic.rs
  ├─ ok_in_test.rs
  └─ bad_indirect_panic.rs   # limitation: indirect panics not detected
```

### `Cargo.toml`

```toml
[package]
name = "no_unwrap_or_else_panic"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
dylint_linting = { workspace = true }
rustc_hir       = { workspace = true }
rustc_lint      = { workspace = true }
rustc_middle    = { workspace = true }
rustc_session   = { workspace = true }
rustc_span      = { workspace = true }
common          = { path = "../../common" }
serde           = { version = "1", features = ["derive"] }

clippy_utils    = { workspace = true, optional = true }

[features]
clippy = ["dep:clippy_utils"]

[dev-dependencies]
dylint_testing = "4"
```

> The optional `clippy` feature plugs into `clippy_utils::macros::is_panic`
> for higher-fidelity panic detection.

### `src/lib.rs` (skeleton)

```rust
#![allow(clippy::single_match_else)]
use dylint_linting::{declare_late_lint, impl_late_lint};
use rustc_hir as hir;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

declare_late_lint!(
    pub NO_UNWRAP_OR_ELSE_PANIC,
    Deny,
    "forbid `unwrap_or_else` whose closure panics (directly or via unwrap/expect)"
);

pub struct Pass;

#[derive(serde::Deserialize, Default)]
struct Config {
    /// Permit panicking closures inside `main`
    allow_in_main: Option<bool>,
}

impl_late_lint! {
    NO_UNWRAP_OR_ELSE_PANIC,
    Pass,

    fn check_expr<'tcx>(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::MethodCall(segment, recv, args, _) = expr.kind {
            if segment.ident.name.as_str() == "unwrap_or_else"
                && common::recv_is_option_or_result(cx, recv)
            {
                if let [closure_arg] = args {
                    if let Some(body_id) = as_closure_body(closure_arg) {
                        if should_lint_here(cx, expr)
                            && closure_contains_forbidden(cx, body_id)
                        {
                            common::span_lint(
                                cx,
                                NO_UNWRAP_OR_ELSE_PANIC,
                                expr.span,
                                "`unwrap_or_else` with a panicking closure; return an error instead",
                            );
                        }
                    }
                }
            }
        }
    }
}

fn as_closure_body(expr: &Expr<'_>) -> Option<hir::BodyId> {
    match expr.kind {
        ExprKind::Closure(hir::Closure { body, .. }) => Some(*body),
        _ => None,
    }
}

fn should_lint_here(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    if common::in_test_like_context(cx, expr.hir_id) {
        return false;
    }
    let cfg: Config = dylint_linting::config_or_default(cx.tcx, "no_unwrap_or_else_panic");
    if cfg.allow_in_main.unwrap_or(false) && common::is_in_main_fn(cx, expr.hir_id) {
        return false;
    }
    true
}

fn closure_contains_forbidden(cx: &LateContext<'_>, body_id: hir::BodyId) -> bool {
    let mut finder = Finder { cx, found: false };
    let body = cx.tcx.hir().body(body_id);
    rustc_hir::intravisit::Visitor::visit_body(&mut finder, body);
    finder.found
}

struct Finder<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    found: bool,
}

impl<'a, 'tcx> rustc_hir::intravisit::Visitor<'tcx> for Finder<'a, 'tcx> {
    fn visit_expr(&mut self, e: &'tcx Expr<'tcx>) {
        if self.found {
            return;
        }
        if is_panic_call(self.cx, e) || is_unwrap_or_expect(self.cx, e) {
            self.found = true;
            return;
        }
        rustc_hir::intravisit::walk_expr(self, e);
    }
}

#[cfg(feature = "clippy")]
fn is_panic_call(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    clippy_utils::macros::is_panic(cx, expr)
}

#[cfg(not(feature = "clippy"))]
fn is_panic_call(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    if let ExprKind::Call(callee, _) = expr.kind {
        if let Some(def_id) = common::def_id_of_expr_callee(cx, callee) {
            return common::is_path_to(cx, def_id, &[&["core", "panicking", "panic"],
                &["core", "panicking", "panic_fmt"],
                &["std", "panic", "panic_any"],
                &["std", "rt", "begin_panic"]]);
        }
    }
    false
}

fn is_unwrap_or_expect(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    if let ExprKind::MethodCall(segment, recv, ..) = expr.kind {
        let name = segment.ident.name.as_str();
        return (name == "unwrap" || name == "expect")
            && common::recv_is_option_or_result(cx, recv);
    }
    false
}
```

> Extend the `common` crate with `def_id_of_expr_callee` and `is_path_to` to
> keep this lint terse. They wrap `type_dependent_def_id` lookups and path
> matching.

### UI tests

```text
bad_unwrap_or_else_panic.rs   # warns on inline panic
ok_map_err.rs                 # ok: propagates errors via map_err/Result
ok_in_test.rs                 # ok: tests may panic intentionally
bad_indirect_panic.rs         # document limitation: helper fn panics indirectly
```

Pair each `.rs` with a `.stderr` expectation using `dylint_testing::ui_test`.

### Workspace integration

- Already covered by the `crates/*` glob in the workspace members.
- Intentionally excluded from the aggregated `suite` crate so teams can opt in
  separately when the policy fits.

```toml
[workspace.metadata.dylint]
libraries = [
  { git = "https://example.com/your/repo.git", pattern = "crates/no_unwrap_or_else_panic" }
]
```

### CI and build matrix

- Add `cargo test -p no_unwrap_or_else_panic` to the pipeline.
- Exercise both `--no-default-features` and `--features clippy` builds to ensure
  optional panic detection remains functional.

### Limitations and future work

- Indirect panics invoked through helper functions are not flagged by
  default; the UI test documents this behaviour.
- Consider a `detect_indirect` configuration knob backed by MIR analysis for
  closures that always diverge (`!` type), albeit at higher maintenance cost.
- Allow a module allowlist, mirroring `no_expect_outside_tests`, if teams
  need targeted exemptions.

## 5) Aggregated library (`suite`) — optional

Bundle all lint crates for users who prefer a single dynamic library. Enable
the `constituent` feature in each lint dependency to prevent them from
exporting their own `register_lints` symbol.

```toml
[package]
name = "suite"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
dylint_linting = { workspace = true }
function_attrs_follow_docs = { path = "../crates/function_attrs_follow_docs", features = ["constituent"] }
no_expect_outside_tests = { path = "../crates/no_expect_outside_tests", features = ["constituent"] }
public_fn_must_have_docs = { path = "../crates/public_fn_must_have_docs", features = ["constituent"] }
module_must_have_inner_docs = { path = "../crates/module_must_have_inner_docs", features = ["constituent"] }
test_must_not_have_example = { path = "../crates/test_must_not_have_example", features = ["constituent"] }
module_max_400_lines = { path = "../crates/module_max_400_lines", features = ["constituent"] }
```

```rust
use dylint_linting::{declare_combined_late_lint_pass, dylint_library};
use rustc_lint::{LateLintPass, LintStore};
use rustc_session::Session;

dylint_library!();

declare_combined_late_lint_pass!(CombinedPass => [
    function_attrs_follow_docs::Pass,
    no_expect_outside_tests::Pass,
    public_fn_must_have_docs::Pass,
    module_must_have_inner_docs::Pass,
    test_must_not_have_example::Pass,
    module_max_400_lines::Pass,
]);

#[no_mangle]
pub fn register_lints(sess: &Session, store: &mut LintStore) {
    dylint_linting::init_config(sess);
    store.register_lints(&[
        function_attrs_follow_docs::FUNCTION_ATTRS_FOLLOW_DOCS,
        no_expect_outside_tests::NO_EXPECT_OUTSIDE_TESTS,
        public_fn_must_have_docs::PUBLIC_FN_MUST_HAVE_DOCS,
        module_must_have_inner_docs::MODULE_MUST_HAVE_INNER_DOCS,
        test_must_not_have_example::TEST_MUST_NOT_HAVE_EXAMPLE,
        module_max_400_lines::MODULE_MAX_400_LINES,
    ]);
    store.register_late_pass(|_| Box::new(CombinedPass));
}
```

> Re-export helper constructors from each lint crate so the combined pass can
> reuse them without duplicating logic.

## 6) Installer CLI — optional

- Builds/stages the CDyLibs, runs `dylint-link`, copies them to a target dir,
  and prints a shell snippet:
  - `export DYLINT_LIBRARY_PATHS="$HOME/.local/share/dylint/lib"`

## 7) Testing strategy

- UI tests per crate using `dylint_testing`.
- Boilerplate:

```rust
#[test]
fn ui() { dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui"); }
```

## 8) CI (GitHub Actions)

- Matrix on Linux/macOS/Windows.
- Steps: build; `cargo dylint --workspace -- -D warnings`; UI tests;
  `cargo clippy -D warnings`; `cargo fmt --check`.
- Optional feature matrix for `clippy_utils`.

## 9) Consumer integration

**Workspace metadata** (preferred):

```toml
[workspace.metadata.dylint]
libraries = [ { git = "https://example.com/your/repo.git", pattern = "crates/*" } ]
```

Then:

```bash
cargo install cargo-dylint dylint-link
cargo dylint --all
```

VS Code rust-analyser integration uses `cargo dylint` as the check command.

## 10) Configuration knobs (examples)

- `module_max_400_lines.max_lines = 400`
- `conditional_max_two_branches.max_atoms = 1`
- `no_expect_outside_tests` allowlist of modules (regex)
- `no_unwrap_or_else_panic.allow_in_main = false`

## 11) Examples (bad → good excerpts)

- **Function attributes order**

```rust
// bad
#[inline]
/// Frobnicate.
pub fn frob() {}
// good
/// Frobnicate.
#[inline]
pub fn frob() {}
```

- **`.expect(..)` outside tests**

```rust
// bad
let n = env::var("PORT").expect("PORT missing");
// good
let n = env::var("PORT").map_err(|e| anyhow::anyhow!("PORT: {e}"))?;
```

- **Public fn must have docs**

```rust
// bad
pub fn important() {}
// good
/// Important entry point.
pub fn important() {}
```

- **Module must have `//!`**

```rust
//! Utilities
mod util { /* … */ }
```

- **Complex predicate (decompose conditional)**

```rust
// bad
if x.started() && y.running() { … }
// better
if should_process(x, y) { … }
fn should_process(x:&X,y:&Y)->bool{ x.started() && y.running() }
```

- **Tests without examples**

```rust
#[test]
fn adds() {}
```

- **Module line limit**
Split oversized modules into submodules.

## 12) Advanced lint feasibility: **Bumpy Road**

The “Bumpy Road” smell captures functions that contain several distinct
clusters of nested branching and complex predicates. The lint is practical to
implement with a Dylint `LateLintPass`: model a per-line complexity signal,
smooth it, and flag functions exhibiting two or more peaks (“bumps”).

**Lint contract.**

- Name: `bumpy_road_function` (alias `complexity_bumpy_road`).
- Kind: `style` (could be `maintainability`).
- Default level: `warn`.
- Scope: free functions, inherent and trait methods (closures optional).
- Trigger: at least two bumps above a configurable threshold.
- Message: “Multiple clusters of nested conditional logic; extract smaller
  functions to smooth this ‘bumpy road’.”

**Signal construction.** Traverse each function body, tracking nesting depth
and predicate complexity.

- Maintain a depth counter for entering/leaving `if`/`else`, `match`, loops.
- Count predicate atoms with `atoms(expr)` where `&&`/`||` add, `!` recurses,
  comparisons and boolean leaves count as one.
- Optionally add a control-flow weight for constructs such as `match` to reflect
  structural heft.

Collect segments `(start_line, end_line, value)` using `SourceMap` mapping and
accumulate contributions with weights (`wD = 1.0`, `wP = 0.5`, `wK = 0.5`).
Rasterise once per function to produce a per-line signal `C[line]` representing
local complexity.

```rust
fn atoms(expr: &Expr<'_>) -> usize {
    match expr.kind {
        ExprKind::Binary(op, lhs, rhs)
            if matches!(op.node, BinOpKind::And | BinOpKind::Or) =>
        {
            atoms(lhs) + atoms(rhs)
        }
        ExprKind::Unary(UnOp::Not, inner) => atoms(inner),
        ExprKind::Binary(op, ..)
            if matches!(
                op.node,
                BinOpKind::Eq | BinOpKind::Ne | BinOpKind::Lt | BinOpKind::Le | BinOpKind::Gt | BinOpKind::Ge
            ) =>
        {
            1
        }
        _ => 1,
    }
}
```

Apply a small moving-average window (`window = 3` by default) to smooth spikes.
Threshold the smoothed signal at `T = 3.0` to identify contiguous bumps; ignore
intervals shorter than `min_bump_lines` (default 2). Warn when the function has
two or more such bumps. Record severity via the area above the threshold and
highlight the top two intervals in the diagnostic.

**Algorithm sketch.**

1. Walk the function HIR, updating depth and collecting segments for blocks,
   branches, and predicate spans.
2. Rasterise segments to per-line values, then smooth with the configured
   window.
3. Detect bumps where the smoothed value meets or exceeds `threshold`.
4. Emit a diagnostic on the function name span when bumps ≥ 2, attaching labels
   on the largest intervals and explaining that distribution (multiple peaks)
   is the issue.

**Configuration** (via `dylint.toml`).

```toml
[bumpy_road_function]
threshold = 3.0
window = 3
min_bump_lines = 2
include_closures = false
weights = { depth = 1.0, predicate = 0.5, flow = 0.5 }
```

**Diagnostics and guidance.** The lint recommends extracting helper functions
or refactoring highlighted sections. Secondary labels point to the top bumps,
and a note clarifies that the smell concerns several peaks rather than a single
deep nest.

**Precision considerations.** Ignore spans from external macro expansions or
`#[automatically_derived]` contexts to avoid noise. Guard-clause heavy
functions typically remain below the threshold after smoothing. Deep single
nests fall under other lints such as `excessive_nesting`.

**Performance.** The pass is linear in the size of each function’s HIR. Segment
rasterisation touches at most the number of lines in the function, keeping the
overhead negligible for typical Rust code.

**Test plan.** Provide UI cases covering two separated nested blocks,
distributed complex predicates, guarded matches, and negative examples (single
peak, guard clauses, macro-heavy code from external crates).

**Implementation notes.** Use `SourceMap` for line mapping and `span` hygiene
checks (`span.from_expansion()`, `span.source_callee()`) to decide when to skip
data. Consider extracting the signal/bump detector into an internal helper
crate for unit tests. The approach dovetails with
`conditional_max_two_branches` and other maintainability lints for a
complementary suite.

**Verdict.** The Bumpy Road lint is realistic and actionable. It approximates
CodeScene’s smell by emphasising the distribution of complexity within a single
function and can ship as an experimental Dylint rule guarded by a feature flag.

## 13) Maintenance policy

- Treat nightly pin as a floor; bump alongside `dylint_linting`.
- Version each lint crate independently; offer `suite` for convenience.
- Keep a changelog; document behaviour changes (e.g., conditional lint
  semantics).

## 14) Deliverables checklist

- [ ] Seven `cdylib` crates under `crates/*` with skeletons.
- [ ] `common` helpers.
- [ ] Optional `suite` aggregated crate.
- [ ] Optional `installer` CLI.
- [ ] UI tests per crate.
- [ ] CI workflows.

## 15) Phased roadmap (small, nested tasks)

### Phase 0 — Repo scaffolding

- Initialise workspace
  - Create `Cargo.toml` with `[workspace]`, resolver = 2, members = `crates/*`,
    `common`, `suite`, `installer`.
  - Add `rust-toolchain.toml` (pin nightly) and `rustfmt.toml`.
  - Add `dylint.toml` (empty to start).
- Set up licensing, CODEOWNERS, CONTRIBUTING, README.
- Add a minimal `justfile`/Makefile for common commands.

### Phase 1 — Common infrastructure

- Implement `common` crate helpers
  - Attribute helpers; context (`in_test_like_context`, `is_test_fn`,
    `is_in_main_fn`).
  - Type helpers; span helpers; visibility helpers.
  - Diagnostics helpers and suggestion utilities.
- Add `dylint_testing` harness macro.
- CI skeleton
  - Workflow with cache + matrix (Linux/macOS/Windows).
  - Jobs: build; UI tests on all lints.

### Phase 2 — Implement seven core lints

- `function_attrs_follow_docs` (order checks + UI tests)
- `no_expect_outside_tests` (receiver type check + context guard + UI tests)
- `public_fn_must_have_docs` (effective visibility + UI tests)
- `module_must_have_inner_docs` (inline/file modules + UI tests)
- `conditional_max_two_branches` (**complex predicate** detector)
  - Count predicate atoms; config `max_atoms`; examples in diagnostics; UI
    tests for `if`/`while`/guards.
- `test_must_not_have_example` (doc text scan + UI tests)
- `module_max_400_lines` (line counting + config + UI tests)

### Phase 3 — Additional restriction lint (separate crate)

- `no_unwrap_or_else_panic`
  - Detect panicking closure; config `allow_in_main`.
  - UI tests: direct panic, allowed in tests; note indirect-panic limitation.

### Phase 4 — Aggregated suite (optional)

- `suite` cdylib with all lints as constituents; combined pass macro.
- UI test verifying registration via `cargo dylint`.

### Phase 5 — Installer CLI (optional)

- Enumerate cdylibs; build `--release`; run `dylint-link`; copy to dest.
- Print `DYLINT_LIBRARY_PATHS` snippet.
- Smoke-test with a tiny sample project.

### Phase 6 — Configuration, docs, and examples

- Per-lint docs: rationale, why, how to fix (include *Decompose Conditional*
  guidance).
- `examples/` projects (before/after and a kitchen sink crate).
- VS Code/rust-analyser override command snippet.

### Phase 7 — CI hardening and QA

- Enforce `cargo dylint -- -D warnings`, `cargo clippy -D warnings`,
  `cargo fmt --check`.
- Feature matrix for `clippy_utils` on/off.
- Deterministic UI tests; add `just fix-ui` target.

### Phase 8 — Publishing & consumer integration

- Tag/publish or provide Git URL usage.
- Document consumer setup via `[workspace.metadata.dylint]`.
- Adoption checklist for downstream teams.

### Phase 9 — Field feedback and tuning

- Collect FP/FN reports; add fixtures.
- Adjust defaults (`max_atoms`, `max_lines`); add targeted allowlists.

### Phase 10 — Extensions (nice-to-haves)

- Experimental **Bumpy Road** lint behind a feature flag.
- Auto-suggestions for local-binding extraction in complex predicates.
- Optional JSON report of lint counts per crate/module.

### Phase 11 — Maintenance

- Track rustc/dylint; bump nightly + deps in lockstep.
- Maintain CHANGELOG; periodic re-runs of examples; refresh UI snapshots.

# Architectural decision record (ADR) 006: directory-based module layout

## Status

Accepted (2026-10-16): Adopt `mod.rs`-rooted directory modules, and extract an
inline test module that outgrows the 400-line budget into a sibling file wired
with `#[cfg(test)] #[path = "..."] mod ...;`.

## Date

2026-10-16.

## Context and problem statement

`AGENTS.md` states that no single code file may be longer than 400 lines. The
rule is a readability constraint rather than a compile-time one: a module of
that length has stopped being reviewable in one sitting, so growth past the
limit is answered by decomposition, not by raising the threshold.

Rust offers two ways to turn a module file into a directory. The first is a
directory whose root file is named `mod.rs`. The second is a *self-named
sibling*: `install_flow.rs` sits next to an `install_flow/` directory that
holds its children. Both are legal, and the choice is not visible to the
compiler's semantics — only to the reader who has to find a module's source.

A second and larger source of growth is the inline test module. Whitaker
colocates unit tests with the code they exercise, and a
`#[cfg(test)] mod tests { … }` block grows faster than the production code
above it, because every edge case adds an arm. Several installer modules had
reached the point where the test block alone carried the file past the budget,
which means the budget could not be met without either weakening the tests or
moving them.

The decision is how to structure modules at the point they pass the budget, and
how to move a test module without disturbing the tests themselves.

## Decision drivers

- Keep every module within the 400-line budget that `AGENTS.md` sets.
- Keep a structural change purely structural, so the diff can be reviewed as a
  move rather than re-read as behaviour.
- Preserve each test module's path, so `super::` and `crate::` imports inside it
  and `cargo test` filter expressions that name it continue to resolve.
- Choose one directory-module form and apply it consistently.
- Keep the extracted file a real module, subject to the same documentation rules
  as the code it came from.

## Options considered

### Option A: keep test modules inline and raise the threshold

Rejected. The budget is the reason to decompose, so exempting the largest files
from it inverts the rule. It also leaves the production code and its tests in
the same file with no mechanism to separate them later.

### Option B: extract tests but use the self-named sibling form

Rejected. The repository's convention is the `mod.rs` form: 46 `mod.rs` files
are tracked outside `vendor/`, and no path in the tree has both `name.rs` and
`name/mod.rs`, so no module is ambiguous between the two. Two file modules
deviate and reach a child through a same-named directory:
`installer/src/cli.rs` resolves `mod source_policy;` to `cli/source_policy.rs`,
and `crates/conditional_max_n_branches/src/driver.rs` resolves `mod numeric;` to
`driver/numeric.rs`. Both are single-child outliers, and neither is a
convention anyone is expected to follow. Adopting the form deliberately would
make a module's location depend on which convention its author happened to use.

### Option C: `mod.rs` directory modules with `#[path]` sibling test files

Accepted.

## Decision outcome

A module that has grown submodules becomes a directory whose root file is
`mod.rs`. Directory modules nest, so `installer/src/toolchain/tests/` is itself
a `tests/` directory module with its own `mod.rs`.

An inline test module that outgrows the budget moves to a sibling file, and an
explicit `#[path]` attribute holds its module path still:

```rust
#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
```

The attribute is what makes the move reviewable. Without it, moving `mod tests`
into another file renames the module, and every `super::` reference inside the
tests, together with every `cargo test` filter that names the old path,
silently changes meaning. With it, the test bodies are byte-identical across
the move and the extraction can be checked as a move.

Three shapes occur, and the rule that distinguishes them is whether anything
above the extracted module already restricts it to test builds:

- *File-module sibling.* The extracted file sits beside the module file and is
  wired with `#[cfg(test)]` plus `#[path = "<name>_tests.rs"]`.
- *Directory-module `tests.rs`.* Inside a directory module the attribute is
  unnecessary, because Rust already resolves a bare `#[cfg(test)] mod tests;` to
  `tests.rs` in the same directory.
- *Nested tests directory.* When the tests need helpers and submodules of their
  own, they become a `tests/` directory module with a `mod.rs`.

`installer/src/deps/tests.rs` shows the variation that follows from the rule.
It is declared as `#[cfg(test)] mod tests;` by `installer/src/deps/mod.rs`, so
the whole file is already test-only and its children omit `#[cfg(test)]` for
that reason alone. They still carry `#[path]`, because there is no
`deps/tests/` directory for a bare `mod` to resolve into.

An extracted test file still opens with a `//!` module-level doc comment, so it
satisfies the repository rule that every module documents its purpose.

## Known risks and limitations

- `#[path]` is a manual anchor. Renaming or moving an extracted test file is not
  checked by the compiler until the module is built, so the attribute can go
  stale in a way that a conventional module path cannot.
- The attribute's *meaning* moves with the module's form. In a file module
  `name.rs`, `#[path = "p"]` resolves relative to the containing directory; in
  a directory module `name/mod.rs` it resolves relative to `name/` itself.
  Converting a file module to a directory module therefore doubles the module's
  own segment in any `#[path]` that already opens with it, as
  `install_flow/mod.rs` did when a rebase renamed `install_flow.rs` around a
  pre-existing anchor. A bare `mod` in a directory module carries no such
  hazard, which is why the decision outcome above confines the attribute to the
  file-module case.
- The 400-line budget is a repository convention that this decision serves, and
  it is **not** yet machine-enforced in this repository. The `module_max_lines`
  dylint lint implements the check and its own UI tests cover it, but the
  workspace does not run Whitaker's lints over itself: `make lint` runs
  `cargo doc` and `cargo clippy` only, and the repository's own `dylint.toml`
  configures `no_std_fs_operations` alone. Self-linting is a separate layer of
  work, and until it lands the budget is held by review.
- The extraction adds a file per module, so a directory listing no longer shows
  a module and its tests together. The `#[path]` attribute is the link between
  them, which is why it is written explicitly rather than inferred.

## Architectural rationale

The decision keeps two properties that matter more than the specific file
names. The first is that structure and behaviour stay separable: because
`#[path]` holds the module path fixed, a decomposition commit can be reviewed
as a move, and a behaviour change can never hide inside one. The second is that
the budget is answered by decomposition rather than exemption, so the rule
continues to describe the codebase instead of describing the exceptions to
itself.

Both properties follow from treating the module layout as a repository-wide
convention rather than a per-module judgement. 65 tracked `.rs` files already
carry a `#[path = "` anchor, so the mechanism this record standardizes was
already the repository's practice; the record fixes which cases use it and why.

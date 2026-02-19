# Whitaker

Snarky Dylint rules for the df12 logisphere.

Whitaker is a collection of opinionated
[Dylint](https://github.com/trailofbits/dylint) lints for Rust. We care about
readable code, sensible module sizes, and keeping panics out of production—so
our lints will gently (and occasionally snarkily) nudge you toward better
habits.

If your team has ever debated whether doc comments belong before or after
`#[inline]`, or wondered how many branches is too many in a conditional,
Whitaker has opinions. Strong ones.

## Quick Start

Add the following to your workspace `Cargo.toml`:

```toml
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "crates/*" }
]
```

Then run:

```sh
cargo dylint --all
```

For version pinning, installation details, and configuration options, see the
[User's Guide](docs/users-guide.md).

## The Lints

Whitaker currently ships eight lints, with more on the way:

| Lint                          | What it does                                                                                                           |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `function_attrs_follow_docs`  | Insists that doc comments come before other attributes. The docs are the star of the show—they go first.               |
| `bumpy_road_function`         | Flags functions with multiple separate clusters of nested conditional complexity.                                      |
| `no_expect_outside_tests`     | Bans `.expect()` on `Option` and `Result` outside test contexts. Production code deserves proper error handling.       |
| `module_must_have_inner_docs` | Requires every module to open with an inner doc comment (`//!`). Future you will thank present you.                    |
| `module_max_lines`            | Caps modules at 400 lines by default. Encourages you to decompose or extract before things get unwieldy.               |
| `conditional_max_n_branches`  | Flags conditionals with more than 2 branches in a single predicate. Complex boolean logic deserves its own home.       |
| `test_must_not_have_example`  | Flags test docs containing examples headings or fenced code blocks. Test docs should describe intent, not tutorials.   |
| `no_unwrap_or_else_panic`     | Catches sneaky panics hidden inside `unwrap_or_else` closures. If you're going to panic, at least be upfront about it. |
| `no_std_fs_operations`        | Forbids `std::fs` operations, nudging you toward capability-based filesystem access via `cap_std`.                     |

## Features

- **Localised diagnostics** — Messages available in English, Welsh (Cymraeg),
  and Scottish Gaelic (Gàidhlig). Set your preference via `DYLINT_LOCALE` or
  `dylint.toml`.
- **Configurable thresholds** — Adjust limits like `module_max_lines.max_lines`
  to match your team's standards.
- **Modular design** — Use individual lints or load the whole suite.

## Project Status

Whitaker is under active development. One additional lint
(`public_fn_must_have_docs`) is planned—see the [roadmap](docs/roadmap.md) for
details.

## Documentation

- [User's Guide](docs/users-guide.md) — Installation, configuration, and
  per-lint documentation
- [Design Document](docs/whitaker-dylint-suite-design.md) — Architecture and
  implementation details
- [Roadmap](docs/roadmap.md) — Development phases and progress

## Licence

Whitaker is released under the [ISC Licence](LICENSE).

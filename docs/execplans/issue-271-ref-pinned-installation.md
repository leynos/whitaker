# Add a `--ref` flag to whitaker-installer for pinned suite installation

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

Issue: [leynos/whitaker#271](https://github.com/leynos/whitaker/issues/271)

## Purpose / big picture

`whitaker-installer` currently installs the lint suite from a mutable source:
the prebuilt path downloads from the `rolling` GitHub release tag, and the
source-build fallback clones or pulls the default branch of
`leynos/whitaker`. Consuming repositories pin the *installer* version in CI
(for example `cargo binstall whitaker-installer@0.2.5`) but cannot pin the
*suite*, so a push to `main` can change lint behaviour across the whole
estate at once.

After this change, a user can run:

```bash
whitaker-installer --ref v0.2.5      # a tag
whitaker-installer --ref 1a2b3c4d…   # a commit SHA
```

and the installer will build and stage the lint suite from exactly that
commit of `leynos/whitaker`. Running `whitaker-installer` with no `--ref`
behaves exactly as today (rolling prebuilt artefacts, default-branch source
fallback). Rolling remains the intentional default; the maintainer does not
want to cut a new installer release for every suite update. This plan
deliberately implements only the explicit-pin part of issue #271, not the
version-matched default proposed there.

Observable success: `whitaker-installer --ref <tag> --build-only` on a clean
machine stages libraries built from the tagged commit (verifiable because the
staged filenames embed the toolchain channel recorded in that commit's
`rust-toolchain.toml`), `whitaker-installer --dry-run --ref <tag>` reports the
ref, and a subsequent un-pinned `whitaker-installer` run still works (the
clone recovers from the detached checkout).

## Constraints

- Default behaviour with no `--ref` must be byte-for-byte unchanged: rolling
  prebuilt download first, default-branch clone/pull fallback.
- The public crate API additions must be additive; no existing public
  function signatures in `whitaker_installer` may change in a way that breaks
  the behaviour tests' existing imports, except where the plan names the
  change explicitly (see `Interfaces and dependencies`).
- The installer must never mutate a user's own working tree: if the current
  directory is itself a Whitaker workspace, `--ref` must fail with a clear
  error rather than checking anything out.
- No new external dependencies. Git operations continue to go through
  `installer/src/git.rs` with the existing 5-minute timeout discipline.
- All work follows the repository gates: `make check-fmt`, `make lint`,
  `make test`, `make markdownlint` must pass before each commit.
- Commit messages follow the file-based workflow (`git commit -F`), no AI
  attribution trailers, en-GB-oxendict prose.
- This shell exports a stray `WHITAKER=true`; run every make gate with
  `env -u WHITAKER` to avoid silently skipping the whitaker lint target.

## Tolerances (exception triggers)

- Scope: if implementation (excluding tests and docs) exceeds ~400 net lines
  or touches more than 12 source files, stop and escalate.
- Interface: if pinning turns out to require changing the signature of a
  public function other than those listed in `Interfaces and dependencies`,
  stop and escalate.
- Dependencies: if a new crate dependency appears necessary, stop and
  escalate.
- Iterations: if a gate still fails after 3 fix attempts on the same failure,
  stop and escalate.
- Ambiguity: if `--ref` semantics interact with an existing flag in a way not
  settled by the Decision Log (for example a new conflict with
  `--no-update`), stop and present options.

## Risks

- Risk: a detached-HEAD checkout left by `--ref` breaks the next un-pinned
  install, because `ensure_workspace` runs plain `git pull`, which fails on a
  detached HEAD ("You are not currently on a branch").
  Severity: high. Likelihood: certain without mitigation.
  Mitigation: the update path must reattach the clone to the default branch
  before pulling (Stage C step 3). This is a required behaviour, tested.
- Risk: the prebuilt manifest's `git_sha` format (full versus abbreviated
  SHA) is not yet confirmed, so SHA comparison against a resolved ref could
  mis-match.
  Severity: medium. Likelihood: medium.
  Mitigation: Stage A confirms the format from
  `installer/src/artefact/git_sha.rs` and the release workflow; comparison
  uses prefix-tolerant matching only if the manifest stores an abbreviated
  SHA, otherwise exact equality.
- Risk: tags in the whitaker repository may not exist for every released
  installer version, making `--ref v0.2.5` fail for users.
  Severity: low (documentation issue, not a code defect).
  Mitigation: document that `--ref` accepts any commit-ish that exists in the
  repository; error messages surface the git failure verbatim.
- Risk: behaviour tests that construct `InstallArgs` literally will fail to
  compile when a field is added.
  Severity: low. Likelihood: high.
  Mitigation: most construction sites use `..InstallArgs::default()`; sweep
  with `env -u WHITAKER cargo check --workspace --all-targets` early in
  Stage C.

## Progress

- [x] (2026-07-08 12:20Z) Worktree created at
  `~/Projects/whitaker.worktrees/issue-271-ref-pinned-installation`, branch
  `issue-271-ref-pinned-installation` from `origin/main` (b1c69c2).
- [x] (2026-07-08 12:40Z) Read the installer source: `cli.rs`, `main.rs`,
  `install_flow.rs`, `workspace.rs`, `git.rs`, `prebuilt.rs`,
  `artefact/download.rs`; mapped the install flow and identified the
  detached-HEAD recovery requirement.
- [x] (2026-07-08 12:50Z) Drafted this ExecPlan.
- [x] (2026-07-08) User approved the plan; proceeding through all stages.
- [x] (2026-07-08) Stage A: confirmed manifest `git_sha` is abbreviated
  (`git rev-parse --short HEAD`) and default-branch discovery command
  (`git rev-parse --abbrev-ref origin/HEAD`); findings recorded below.
- [ ] Stage B: red tests (CLI parsing, git ref operations, workspace
  decision, prebuilt SHA validation, BDD scenarios).
- [ ] Stage C: implementation (CLI field, git helpers, workspace plumbing,
  prebuilt validation, dry-run output).
- [ ] Stage D: docs (users-guide, README, `--help` text), refactor, full
  gates, commit-by-commit delivery.
- [ ] Manual end-to-end validation transcript recorded under `Artifacts`.

## Surprises & discoveries

- (2026-07-08, Stage A) The manifest `git_sha` is **abbreviated**, not a full
  40-hex SHA. `.github/workflows/rolling-release.yml` line 142 writes it from
  `git rev-parse --short HEAD` into the package tool's `--git-sha`. Git's
  `--short` yields the shortest unambiguous prefix (7+ hex). The `GitSha`
  newtype in `installer/src/artefact/git_sha.rs` accepts 7–40 hex chars,
  consistent with this. **Comparison rule fixed:** a pinned install may use the
  rolling prebuilt only when the resolved *full* commit SHA `starts_with` the
  manifest's abbreviated `git_sha` (prefix-tolerant), not exact equality.
- (2026-07-08, Stage A) Default-branch discovery: the platform clone carries
  `refs/remotes/origin/HEAD` symbolic ref (`git symbolic-ref
  refs/remotes/origin/HEAD` → `refs/remotes/origin/main`; `git rev-parse
  --abbrev-ref origin/HEAD` → `origin/main`). `ensure_default_branch` uses
  `git rev-parse --abbrev-ref origin/HEAD`, strips the `origin/` prefix to get
  the branch name, and falls back to `git remote set-head origin --auto` once
  when the symbolic ref is absent from an older clone. The repository default
  branch is `main`.
- (2026-07-08, Stage A) The shared test manifest helper
  `test_utils::prebuilt_manifest_json` hardcodes `"git_sha":"abc1234"`. The
  prebuilt SHA-match tests therefore key off that literal: a full SHA beginning
  `abc1234…` matches; any other value is a mismatch.

## Decision log

- Decision: keep `rolling` as the default; `--ref` is an explicit opt-in pin.
  Rationale: maintainer direction — a version-matched default would force an
  installer release for every suite update. This supersedes the "default to
  the installer's own version tag" proposal in issue #271.
  Date/Author: 2026-07-08, user (pmcintosh).
- Decision: name the flag `--ref` (CLI field `git_ref`, since `ref` is a Rust
  keyword), accepting any commit-ish (SHA, tag, or branch name), documented
  with SHA and tag as the supported use cases.
  Rationale: user asked for "--ref … allowing a SHA or tag"; git resolves all
  commit-ish forms identically, so restricting the value would add validation
  code without adding safety. Branch names resolve at install time and are
  therefore not reproducible pins; the documentation says so.
  Date/Author: 2026-07-08, agent.
- Decision: `--ref` composes with `--no-update` rather than conflicting.
  With both flags, the installer resolves the ref against the existing clone
  without fetching; it fetches only when the ref cannot be resolved locally.
  Rationale: "use my existing clone, offline, at this ref" is a coherent and
  useful request; fetch-on-miss keeps the common pinned-CI path working.
  Date/Author: 2026-07-08, agent.
- Decision: with `--ref`, the prebuilt fast path is attempted only when the
  resolved commit SHA matches the rolling manifest's `git_sha`; any mismatch
  falls back to a source build of the pinned commit. Without `--ref`,
  prebuilt behaviour is unchanged.
  Rationale: the rolling release only ever carries artefacts for one commit,
  so a pinned install can use it only when the pin happens to match; the
  existing `PrebuiltResult::Fallback` mechanism makes the mismatch case free.
  Date/Author: 2026-07-08, agent.
- Decision: `--ref` while the current directory is itself a Whitaker
  workspace is an error (`InstallerError::WorkspaceNotFound` is wrong here; a
  new `InstallerError::RefUnsupported`-style variant carries the message).
  Rationale: checking out a ref in the user's own working tree could destroy
  uncommitted work; refusing is the only safe behaviour.
  Date/Author: 2026-07-08, agent.

## Outcomes & retrospective

To be completed at milestones and at the end.

## Context and orientation

The repository is a Cargo workspace (`Cargo.toml` members: `common`,
`crates/*`, `installer`, `suite`). The installer lives in `installer/` and
publishes to crates.io as `whitaker-installer`. Key modules, all paths
relative to the repository root:

- `installer/src/cli.rs` — clap definitions. `InstallArgs` is the flag
  struct; it has a hand-written `Default` impl and is flattened into `Cli`
  for the default (no-subcommand) install. Tests in
  `installer/src/cli_tests.rs`.
- `installer/src/main.rs` — orchestration. `run_install` performs: (1)
  dependency check, (2) `ensure_whitaker_workspace` (clone or update the
  platform clone at `~/.local/share/whitaker` on Linux), (3) crate and
  toolchain resolution (reads `rust-toolchain.toml` from the workspace root —
  note this happens *after* the workspace step, so a checked-out ref
  naturally supplies its own toolchain pin), (3.5)
  `try_fast_path_installation` (prebuilt download, then a test-only staged
  suite path), (4) source build via `pipeline::perform_build`, (5) wrapper
  scripts and metrics.
- `installer/src/workspace.rs` — `WHITAKER_REPO_URL`, `WorkspaceAction`
  (`UseCurrentDir` / `CloneTo` / `UpdateAt` / `UseExisting`),
  `decide_workspace_action(cwd, clone_dir, update)`, and
  `ensure_workspace(dirs, update)` which executes the action.
- `installer/src/git.rs` — `clone_repository`, `update_repository`, and the
  private `run_git_with_timeout(args, working_dir, operation)` helper (5-min
  timeout, threaded pipe draining). All new git operations reuse this helper.
- `installer/src/prebuilt.rs` — `PrebuiltConfig` (target, toolchain,
  destination, quiet), `attempt_prebuilt`, and the private `run_pipeline`
  that downloads the manifest, validates toolchain and target, downloads and
  verifies the archive, and extracts. Validation failures become
  `PrebuiltResult::Fallback` — never fatal.
- `installer/src/artefact/download.rs` — `ROLLING_TAG = "rolling"` and URL
  construction; `installer/src/artefact/manifest.rs` and
  `artefact/git_sha.rs` — the manifest carries a `git_sha()` accessor whose
  exact width Stage A confirms.
- `installer/src/install_flow.rs` — `PrebuiltInstallationContext` and
  `try_prebuilt_installation`, the seam through which `main.rs` passes data
  into `prebuilt.rs`.
- `installer/src/output.rs` — `DryRunInfo` renders `--dry-run` output.
- Behaviour tests: `installer/tests/behaviour_*.rs` with Gherkin features in
  `installer/tests/features/*.feature`, driven by `rstest-bdd`. CLI-facing
  scenarios live in `installer.feature`; prebuilt scenarios in
  `prebuilt_download.feature`.
- Docs: `docs/users-guide.md` (user-facing flags), `README.md`.

Terms: a *commit-ish* is any expression git can resolve to a commit (SHA,
tag, branch, `HEAD~2`, …). A *detached HEAD* is a checkout of a commit rather
than a branch; `git pull` refuses to run on one. The *platform clone* is the
installer-managed copy of this repository under the user's data directory.

## Plan of work

Stage A — confirm two facts, no code changes. First, read
`installer/src/artefact/git_sha.rs`, `artefact/manifest.rs`, and the release
workflow under `.github/workflows/` to establish whether the manifest
`git_sha` is a full 40-hex SHA or abbreviated; record the answer in
`Surprises & Discoveries` and fix the comparison rule (exact match for full,
`starts_with` for abbreviated). Second, decide the default-branch discovery
command for detached-HEAD recovery: prefer
`git rev-parse --abbrev-ref origin/HEAD` (strips to `origin/main`), falling
back to `git remote set-head origin --auto` once if the symbolic ref is
absent from an old clone.

Stage B — red tests. Add failing tests before each implementation slice:

1. `installer/src/cli_tests.rs`: parsing `--ref v0.2.5` populates
   `InstallArgs::git_ref`; absence leaves it `None`; `--ref` is accepted both
   bare and under the `install` subcommand.
2. `installer/src/git.rs` tests (new, using real `git` against `TempDir`
   fixtures — create a source repository with two commits and a tag, clone
   it, then exercise the new functions): `resolve_commit` resolves a tag and
   a SHA and errors on garbage; `checkout_detached` leaves HEAD at the
   commit; `ensure_default_branch` reattaches a detached clone so that a
   subsequent `update_repository` succeeds.
3. `installer/src/workspace.rs` tests: `decide_workspace_action` outcomes
   are unchanged; a new decision-level test that a ref plus
   `UseCurrentDir` yields the refusal error.
4. `installer/src/prebuilt_tests.rs`: with `expected_git_sha: Some(...)`
   mismatching the mocked manifest, the pipeline returns
   `PrebuiltResult::Fallback` whose reason mentions the SHA mismatch; with a
   matching SHA it succeeds as before; with `None` behaviour is unchanged.
5. BDD: extend `installer/tests/features/installer.feature` (and
   `behaviour_cli` steps) with scenarios: "Pin the suite to a tag" (parsing
   plus dry-run output contains the ref) and "Refuse --ref inside a Whitaker
   workspace". Extend `prebuilt_download.feature` with "Prebuilt is skipped
   when the pinned ref does not match the rolling manifest".

Each red test is run first and must fail for the expected reason (missing
field / missing function / missing validation) before its green slice lands.

Stage C — implementation, in five small commits:

1. CLI: add `git_ref: Option<String>` to `InstallArgs` in
   `installer/src/cli.rs` as `#[arg(long = "ref", value_name = "REF")]`, doc
   comment "Install the lint suite at a specific commit SHA or tag [default:
   rolling]". Update `Default`, the `after_help` examples, and any literal
   constructors that fail to compile.
2. Git helpers in `installer/src/git.rs`: `resolve_commit(repo, refspec) ->
   Result<String>` (`git rev-parse --verify <refspec>^{commit}`),
   `fetch_ref(repo, refspec)` (`git fetch origin <refspec> --tags`),
   `checkout_detached(repo, commit)` (`git checkout --detach <commit>`), and
   `ensure_default_branch(repo)` (no-op when already on a branch; otherwise
   discover the default branch per Stage A and check it out). All through
   `run_git_with_timeout`.
3. Workspace plumbing: extend `ensure_workspace` to
   `ensure_workspace(dirs, update, git_ref: Option<&str>)` (public signature
   change, named in `Interfaces and dependencies`). Behaviour: on
   `UseCurrentDir` with a ref → the refusal error; on `CloneTo` → clone then
   pin; on `UpdateAt` → `ensure_default_branch` then pull then pin; on
   `UseExisting` with a ref → pin without pulling. "Pin" means: try
   `resolve_commit`; on failure `fetch_ref` once and retry; then
   `checkout_detached`. Crucially, `UpdateAt` with *no* ref also calls
   `ensure_default_branch` first, fixing the recovery risk. Return both the
   workspace path and the resolved commit SHA (a small
   `WorkspaceCheckout { root: Utf8PathBuf, pinned_commit: Option<String> }`
   struct) so `main.rs` can hand the SHA to the prebuilt path.
4. Prebuilt: add `expected_git_sha: Option<&'a str>` to `PrebuiltConfig`,
   validate in `run_pipeline` after the target check using the Stage A
   comparison rule, and thread the value from `run_install` through
   `PrebuiltInstallationContext` and `try_prebuilt_installation`. Update the
   fixed-signature `AttemptPrebuiltFn` type alias and hooks in
   `install_flow.rs` as needed.
5. Dry run and messaging: add the ref to `DryRunInfo` and its
   `display_text`; in `ensure_whitaker_workspace`'s progress messages, print
   `Pinning Whitaker suite to REF (SHORT_SHA)...` when a ref is given.

Stage D — documentation and closure: document `--ref` in
`docs/users-guide.md` (a "Pinning the suite" subsection: what it accepts,
that branch pins are not reproducible, prebuilt-match behaviour, interplay
with `--no-update`) and in `README.md`; refresh the `--help` examples; run
the full gate suite via the scrutineer subagent; update this plan's living
sections; comment on issue #271 describing what shipped and what was
deliberately not changed (the rolling default).

## Concrete steps

All commands run from
`~/Projects/whitaker.worktrees/issue-271-ref-pinned-installation`.

Red example (Stage B slice 1):

```bash
env -u WHITAKER cargo nextest run -p whitaker-installer cli_tests 2>&1 \
  | tee /tmp/test-whitaker-issue-271.out
# expect: compile error E0609 (no field `git_ref`) or a failing
# `parses_ref_flag` assertion — the red reason must be the missing field.
```

Green example (after Stage C slice 1):

```bash
env -u WHITAKER cargo nextest run -p whitaker-installer cli_tests 2>&1 \
  | tee /tmp/test-whitaker-issue-271.out
# expect: all cli tests pass.
```

Full gates before each commit (delegate to the scrutineer subagent, which
logs to /tmp and returns a bounded report):

```bash
env -u WHITAKER make check-fmt
env -u WHITAKER make lint
env -u WHITAKER make test
env -u WHITAKER make markdownlint
```

Manual end-to-end check (Stage D, uses an older tag known to exist):

```bash
git tag --list 'v*' | tail -3         # pick a real tag, e.g. v0.2.4
cargo run -p whitaker-installer -- --dry-run --ref v0.2.4
# expect: dry-run output includes the ref and the workspace path
cargo run -p whitaker-installer -- --ref v0.2.4 --build-only \
  --target-dir /tmp/whitaker-ref-smoke
# expect: staged libraries under /tmp/whitaker-ref-smoke named for the
# toolchain channel pinned by v0.2.4's rust-toolchain.toml
cargo run -p whitaker-installer -- --dry-run
# expect: default behaviour unchanged; then verify the platform clone
# recovers: run a default install and confirm no detached-HEAD pull error
```

## Validation and acceptance

Acceptance is behavioural:

1. `whitaker-installer --ref <tag> --build-only` stages libraries built from
   the tagged commit; `whitaker-installer --dry-run --ref <tag>` prints the
   ref.
2. `whitaker-installer --ref <garbage>` exits non-zero with a git error
   naming the ref; nothing is staged.
3. `whitaker-installer --ref <anything>` run inside a Whitaker workspace
   checkout exits non-zero with the refusal message and does not touch the
   working tree.
4. After a pinned install, a plain `whitaker-installer` run succeeds (the
   clone reattaches to the default branch and pulls).
5. With `--ref` set and the rolling manifest's `git_sha` differing from the
   resolved commit, the installer prints the fallback message and builds
   from source; when they match, the prebuilt path is used.
6. `make test` passes with the new unit and behaviour tests; each new test
   demonstrably failed first (Red evidence retained in the tee'd logs under
   `/tmp/test-whitaker-issue-271.out` and summarized in `Artifacts`).
7. `make check-fmt`, `make lint` (clippy plus the dylint suite), and
   `make markdownlint` pass.

BDD specification driving slice 5 of Stage B (final wording may be adjusted
to the step vocabulary in `installer/tests/behaviour_cli/`):

```gherkin
Scenario: Pin the suite to a tag
  Given the --ref flag is set to "v0.2.5"
  When the CLI arguments are parsed
  Then the install arguments carry the ref "v0.2.5"

Scenario: Refuse --ref inside a Whitaker workspace
  Given the current directory is a Whitaker workspace
  And the --ref flag is set to "v0.2.5"
  When the workspace is prepared
  Then preparation fails with a ref-unsupported error

Scenario: Prebuilt is skipped when the pinned ref does not match
  Given a manifest whose git SHA differs from the pinned commit
  When a prebuilt installation is attempted
  Then the result is a fallback mentioning the SHA mismatch
```

## Idempotence and recovery

All steps are re-runnable. The worktree is disposable; `git worktree remove`
plus branch deletion resets everything. Test git repositories live in
`TempDir`s and clean themselves up. The manual smoke test writes only to
`/tmp/whitaker-ref-smoke` (delete freely) and the platform clone at
`~/.local/share/whitaker`, which the recovery behaviour (acceptance item 4)
restores to the default branch; if a mid-implementation failure leaves it
detached, `git -C ~/.local/share/whitaker checkout main` restores it by
hand. If a gate fails mid-commit, fix and re-run the gate; never commit over
a red gate.

## Artifacts and notes

Red/Green transcripts and the manual smoke-test transcript will be added
here as the work proceeds.

## Interfaces and dependencies

No new crate dependencies. At completion the following must exist:

In `installer/src/cli.rs`:

```rust
pub struct InstallArgs {
    // …existing fields…
    /// Install the lint suite at a specific commit SHA or tag.
    #[arg(long = "ref", value_name = "REF")]
    pub git_ref: Option<String>,
}
```

In `installer/src/git.rs` (all `pub`, all routed through
`run_git_with_timeout`):

```rust
pub fn resolve_commit(repo: &Utf8Path, refspec: &str) -> Result<String>;
pub fn fetch_ref(repo: &Utf8Path, refspec: &str) -> Result<()>;
pub fn checkout_detached(repo: &Utf8Path, commit: &str) -> Result<()>;
pub fn ensure_default_branch(repo: &Utf8Path) -> Result<()>;
```

In `installer/src/workspace.rs` (signature change, the one sanctioned
breaking change; all in-repo callers updated in the same commit):

```rust
pub struct WorkspaceCheckout {
    pub root: Utf8PathBuf,
    pub pinned_commit: Option<String>,
}

pub fn ensure_workspace(
    dirs: &dyn BaseDirs,
    update: bool,
    git_ref: Option<&str>,
) -> Result<WorkspaceCheckout>;
```

In `installer/src/prebuilt.rs`:

```rust
pub struct PrebuiltConfig<'a> {
    // …existing fields…
    /// When set, require the manifest git SHA to match this commit.
    pub expected_git_sha: Option<&'a str>,
}
```

`installer/src/install_flow.rs` threads `pinned_commit` from
`WorkspaceCheckout` into `PrebuiltInstallationContext` and on into
`PrebuiltConfig`. `installer/src/output.rs`'s `DryRunInfo` gains a
`git_ref: Option<&'a str>` field rendered in `display_text`.

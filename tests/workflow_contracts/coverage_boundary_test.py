"""CV-005: only `main` writes persistent coverage state.

A pull-request lane here measures coverage and stops. Nothing compares the
report with a baseline: the ratchet half of CV-005 is deferred, and the
developers' guide says why under "The half of CV-005 that is deferred here".
What the lane must not do is publish the report, call the CodeScene action, run
a `cs-coverage` command, or hold the credential either of those needs. The
check step that used to sit in `ci.yml` is why a CodeScene outage or a token
change could redden a pull request that had touched nothing to do with
coverage.

Every assertion here that reads this repository's files is paired with one that
drives the reader over a synthetic document. Over the repository's own
workflows a reader that answered nothing agrees with a correct one exactly, so
the rule would pass with every detector deleted.

Run via ``make test-workflow-contracts``.
"""

import functools
import typing as typ

import pytest
from coverage_boundary import (
    CODESCENE_HOST,
    COVERAGE_COMMAND,
    CREDENTIAL_ENVIRONMENT_KEY,
    GENERATE_COVERAGE_ACTION,
    PUBLICATION_OPT_OUT_INPUT,
    PUBLICATION_OPT_OUT_VALUE,
    UPLOAD_COVERAGE_ACTION,
    action_of,
    coverage_surface_offenders,
    publishes_the_coverage_report,
)
from pull_request_reach import pull_request_closure
from ubicloud_workflow_support import (
    WORKFLOWS_DIRECTORY,
    job_steps,
    load_job,
    parse_workflow,
)

#: The lane that owns the upload, and is therefore the one exemption.
PUBLISHER_WORKFLOW: typ.Final[str] = "coverage-main.yml"

#: The pull-request lane that measures coverage. Named so the removal below is
#: guarded: "no CodeScene on a pull request" is satisfied by deleting the
#: coverage build too, and this repository's coverage run *is* its test run.
MEASURING_JOB: typ.Final[str] = "coverage-check"

#: What that job must keep doing. `make coverage` is this repository's own
#: `cargo llvm-cov nextest` driver, reusing the `make test` selection rather
#: than the shared action's.
MEASURING_COMMAND: typ.Final[str] = "make coverage"


def _synthetic(step_body: str) -> dict[str, typ.Any]:
    """Return a one-job pull-request workflow declaring the given steps."""
    text = (
        "on:\n  pull_request:\njobs:\n  a:\n    runs-on: ubuntu-latest\n"
        f"    steps:\n{step_body}"
    )
    parsed = parse_workflow(text)
    assert isinstance(parsed, dict), "the synthetic workflow must parse to a mapping"
    return parsed


def _workflow_names() -> list[str]:
    """Return every checked-in workflow file name, for parametrization."""
    # Case-insensitive, so a `.YML` workflow is not skipped in silence.
    return sorted(
        path.name
        for path in WORKFLOWS_DIRECTORY.iterdir()
        if path.suffix.lower() in (".yml", ".yaml")
    )


@functools.cache
def _repository_closure() -> frozenset[str]:
    """Return the checked-in workflows a pull request can cause to run."""
    documents = {
        name: parsed
        for name in _workflow_names()
        if isinstance(
            parsed := parse_workflow(
                (WORKFLOWS_DIRECTORY / name).read_text(encoding="utf-8")
            ),
            dict,
        )
    }
    return pull_request_closure(documents)


def test_the_repository_closure_is_not_empty() -> None:
    """The sweep below ranges over the closure, so an empty one proves nothing.

    A reader that recognized no trigger would exempt every workflow and pass.
    `ci.yml` answers `pull_request` and must be in it.
    """
    assert "ci.yml" in _repository_closure(), (
        f"ci.yml serves pull requests; the closure read {_repository_closure()}"
    )


@pytest.mark.parametrize("name", _workflow_names())
def test_no_pull_request_workflow_touches_the_publication_surface(name: str) -> None:
    """The boundary, over every workflow a pull request can reach.

    `coverage-main.yml` is the exemption and the only one. A lane that a pull
    request can start, directly or through a reusable-workflow call, must not
    publish the report, invoke the CodeScene action, run its command, name its
    host, or carry its credential, because each needs a secret that a pull
    request's own run cannot be trusted with and none of them tells the author
    anything the ratchet does not.
    """
    if name == PUBLISHER_WORKFLOW or name not in _repository_closure():
        return
    raw = (WORKFLOWS_DIRECTORY / name).read_text(encoding="utf-8")
    document = parse_workflow(raw)
    assert isinstance(document, dict), f"{name} is in the closure, so it parsed"
    offenders = coverage_surface_offenders(name, document, raw)
    assert not offenders, (
        f"{name} can be reached by a pull request, so it must leave the "
        f"coverage publication surface to {PUBLISHER_WORKFLOW}: {offenders}"
    )


def test_the_measuring_lane_still_runs_the_instrumented_suite() -> None:
    """Removing the upload must not leave the lane measuring nothing.

    Deleting a CodeScene step satisfies "no CodeScene on a pull request", and
    it would also satisfy it with the coverage build deleted. Here that would
    delete the tests as well: `make coverage` drives `cargo llvm-cov nextest`
    over the same selection `make test` uses, so this lane's coverage run is
    this repository's test run.

    Half of CV-005 is deferred here, deliberately. The rule also asks a
    pull-request lane to call the shared `generate-coverage` action with
    `with-ratchet: true`, so that changed-line feedback comes from a baseline
    the trunk wrote. This repository calls that action on neither lane: it runs
    `make coverage`, whose documented reason is that the driver reuses the
    exact crate selection and warning policy `make test` uses.

    So a pull request here gets no changed-line comparison at all. Adopting the
    ratchet means first answering whether `generate-coverage`'s inputs can
    reproduce that selection exactly; until then the driver decision stands and
    this contract asserts only that the lane still measures. See "The half of
    CV-005 that is deferred here" in the developers' guide.
    """
    runs = [
        str(step.get("run", "")) for step in job_steps(load_job(MEASURING_JOB))
    ]
    assert any(MEASURING_COMMAND in script for script in runs), (
        f"{MEASURING_JOB} must still run `{MEASURING_COMMAND}`; without it the "
        f"pull-request lane runs no tests at all"
    )


def test_the_publisher_keeps_the_upload_this_boundary_moved_to_it() -> None:
    """A rule that only forbids the upload elsewhere is satisfied by deleting it.

    The upload is not abolished; it is owned. If `coverage-main.yml` stopped
    making it, CodeScene would have no coverage at all and every contract above
    would still pass.
    """
    raw = (WORKFLOWS_DIRECTORY / PUBLISHER_WORKFLOW).read_text(encoding="utf-8")
    document = parse_workflow(raw)
    assert isinstance(document, dict), f"{PUBLISHER_WORKFLOW} must parse"
    # The closure rather than the triggers, so a pull-request workflow calling
    # the publisher cannot make its exemption a hole.
    assert PUBLISHER_WORKFLOW not in _repository_closure(), (
        f"{PUBLISHER_WORKFLOW} must not be reachable by a pull request, or "
        f"moving the upload into it moves nothing"
    )
    uploads = [
        step
        for definition in (document.get("jobs") or {}).values()
        if isinstance(definition, dict)
        for step in job_steps(definition)
        if action_of(step) == UPLOAD_COVERAGE_ACTION
    ]
    assert uploads, (
        f"{PUBLISHER_WORKFLOW} must keep the CodeScene upload; without it "
        f"nothing publishes coverage and every rule above is vacuous"
    )


@pytest.mark.parametrize(
    ("step_body", "expected"),
    [
        pytest.param(
            f"      - uses: {UPLOAD_COVERAGE_ACTION}@abc\n",
            "invokes the CodeScene coverage action",
            id="the-codescene-action",
        ),
        pytest.param(
            f"      - run: {COVERAGE_COMMAND} check --format lcov\n",
            f"runs a {COVERAGE_COMMAND} command",
            id="the-command-form",
        ),
        pytest.param(
            "      - uses: actions/upload-artifact@abc\n"
            "        with:\n          path: lcov.info\n",
            "publishes the coverage report as an artefact",
            id="an-artefact-upload-of-the-report",
        ),
        pytest.param(
            "      - uses: actions/upload-artifact@abc\n",
            "publishes the coverage report as an artefact",
            id="an-artefact-upload-of-the-workspace",
        ),
        pytest.param(
            f"      - uses: {GENERATE_COVERAGE_ACTION}@abc\n",
            "without declining its own archive",
            id="a-coverage-call-that-keeps-its-archive",
        ),
    ],
)
def test_each_forbidden_element_is_reported(step_body: str, expected: str) -> None:
    """Each offence, added back one at a time.

    This repository's workflows carry none of these, so the contract above
    cannot separate a working detector from a deleted one. Each case here is
    the only evidence that one of them fires.
    """
    offenders = coverage_surface_offenders("scratch.yml", _synthetic(step_body), "")
    assert any(expected in offence for offence in offenders), (
        f"a lane declaring this step must be reported as {expected!r}; "
        f"the reading gave {offenders}"
    )


@pytest.mark.parametrize(
    "path",
    [
        pytest.param("lcov.info", id="the-report-by-name"),
        pytest.param(".", id="the-workspace-as-a-dot"),
        pytest.param("./", id="the-workspace-with-a-separator"),
        pytest.param(".//", id="the-workspace-spelt-oddly"),
        pytest.param("../workspace", id="a-path-reaching-upward"),
        pytest.param("**/*.info", id="a-glob-that-matches-it"),
        pytest.param("dist/*", id="a-glob-that-does-not"),
        pytest.param("", id="an-empty-path"),
        pytest.param("dist/\nlcov.info", id="one-safe-entry-and-one-not"),
        pytest.param("dist/\n.", id="one-safe-entry-and-the-workspace"),
        pytest.param("${{ github.workspace }}", id="the-workspace-expression"),
        pytest.param("${{ github.workspace }}/", id="the-workspace-expression-slashed"),
        pytest.param("$GITHUB_WORKSPACE", id="the-workspace-variable"),
        pytest.param("~", id="the-home-directory"),
        pytest.param("/home/runner/work", id="an-absolute-ancestor"),
        pytest.param("/", id="the-root"),
    ],
)
def test_an_artefact_path_that_could_carry_the_report_is_an_offence(
    path: str,
) -> None:
    """Fail closed: the question is what can leave, not how it is spelt.

    A substring test for `lcov.info` clears every one of these. `.` and `./`
    upload the workspace, which holds the report. `..` reaches above the
    directory the entry names. A glob may match the report however innocent it
    looks, and `dist/*` is here because the reader does not evaluate patterns:
    it refuses them, which is the safe direction for a rule whose false
    negatives are silent. An expression, a variable, `~` and an absolute path
    are refused for the same reason: each may resolve to the workspace or an
    ancestor of it, and the reader does not resolve them.

    `path` is newline-separated, so one unsafe entry publishes the report
    whatever the others name.
    """
    # A multi-line `path` has to go in as a YAML block scalar. Writing it as a
    # quoted scalar with `\n` inside puts a literal backslash-n in the value,
    # and the reader then sees one entry rather than two: the first draft of
    # this case passed for that reason while proving nothing.
    if "\n" in path:
        rendered = "|\n" + "".join(f"            {line}\n" for line in path.split("\n"))
    else:
        rendered = f"{path!r}\n"
    step_body = (
        "      - uses: actions/upload-artifact@abc\n"
        "        with:\n"
        f"          path: {rendered}"
    )
    offenders = coverage_surface_offenders(
        "scratch.yml", _synthetic(step_body), ""
    )
    assert any(
        "publishes the coverage report" in offence for offence in offenders
    ), f"a path of {path!r} can carry the report; the reading gave {offenders}"


def test_an_ordinary_lane_is_not_accused() -> None:
    """The other direction, so the detectors discriminate rather than accuse."""
    offenders = coverage_surface_offenders(
        "scratch.yml", _synthetic("      - run: make test\n"), ""
    )
    assert not offenders, (
        f"an ordinary lane must produce no offence; it produced {offenders}"
    )


def test_the_compliant_coverage_call_passes_its_own_rule() -> None:
    """The shape the contract asks for must itself be accepted."""
    offenders = coverage_surface_offenders(
        "scratch.yml",
        _synthetic(
            f"      - uses: {GENERATE_COVERAGE_ACTION}@abc\n"
            f"        with:\n"
            f"          with-ratchet: 'true'\n"
            f"          {PUBLICATION_OPT_OUT_INPUT}: '{PUBLICATION_OPT_OUT_VALUE}'\n"
        ),
        "",
    )
    assert not offenders, f"the prescribed shape must pass; it gave {offenders}"


@pytest.mark.parametrize("suffix", ["-legacy", "-v2"])
def test_a_lookalike_action_is_a_different_action(suffix: str) -> None:
    """Splitting on the version separator is what tells them apart.

    A prefix match would report `upload-codescene-coverage-legacy` as the real
    action, which is an accusation rather than a finding.
    """
    offenders = coverage_surface_offenders(
        "scratch.yml",
        _synthetic(f"      - uses: {UPLOAD_COVERAGE_ACTION}{suffix}@abc\n"),
        "",
    )
    assert not offenders, (
        f"{UPLOAD_COVERAGE_ACTION}{suffix} is a different action; the reading "
        f"gave {offenders}"
    )


@pytest.mark.parametrize(
    "path",
    [
        pytest.param("dist/", id="a-directory"),
        pytest.param("sccache-stats.json", id="the-shape-this-repository-uploads"),
        pytest.param("./target/nextest/junit.xml", id="a-nested-file"),
    ],
)
def test_an_artefact_step_naming_another_path_is_not_an_offence(path: str) -> None:
    """Uploading something other than the report is allowed.

    The narrow half of the fail-closed reading: a concrete relative path that
    descends from the workspace, and does not name the report, is cleared.
    """
    step_body = (
        "      - uses: actions/upload-artifact@abc\n"
        f"        with:\n          path: {path}\n"
    )
    document = _synthetic(step_body)
    assert not coverage_surface_offenders("scratch.yml", document, ""), (
        "an artefact step naming a path that is not the report must pass"
    )
    assert not publishes_the_coverage_report(job_steps(document["jobs"]["a"])[0])


def test_the_credential_is_found_in_text_the_parser_would_drop() -> None:
    """A comment is not a hiding place.

    The parsed scan alone would miss a credential named in a comment or in a
    shape the parser flattened, and a workflow that mentions it is a workflow
    somebody is about to wire it into.
    """
    clean = _synthetic("      - run: make test\n")
    offenders = coverage_surface_offenders(
        "scratch.yml", clean, f"# see {CREDENTIAL_ENVIRONMENT_KEY} in main\n"
    )
    assert any("raw text" in offence for offence in offenders), (
        f"the credential must be reported from the raw text; got {offenders}"
    )

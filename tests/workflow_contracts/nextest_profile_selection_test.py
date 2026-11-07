"""Keep workflow profile selection visible in each suite command.

Purpose
-------
Ensure workflow lanes pass Nextest profile selection to the test command.

Usage
-----
Run this contract with ``make test-workflow-contracts``.
"""

from __future__ import annotations

from suite_lanes import (
    PROFILE_VARIABLE,
    _scopes_declaring_profile,
    _workflow_documents,
)


def test_no_workflow_selects_a_profile_through_the_environment() -> None:
    """Require workflow lanes to select Nextest profiles on the command line.

    ``timeout_ordering_test`` learns each lane's profile from
    ``NEXTEST_PROFILE=ci`` in the command. An environment declaration would
    make that reader judge the lane under the wrong profile's budgets.
    """
    declared = _scopes_declaring_profile(_workflow_documents())
    assert not declared, (
        f"{PROFILE_VARIABLE} is declared in an env block at {declared}; the "
        f"lane reader takes a lane's profile from its command line. Pass it "
        f"there, or teach the reader to resolve the environment first"
    )

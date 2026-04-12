"""Unit tests for rolling-release workflow expression helpers."""

from __future__ import annotations

import pytest

from tests.workflows.rolling_release_workflow_test_support import (
    _github_expression_compares_operand_to_false,
    _github_expression_mentions_operand,
    _github_expression_negates_operand,
)


OPERAND = "steps.assets.outputs.has_assets"


@pytest.mark.parametrize(
    ("expression", "expected"),
    [
        ("steps.assets.outputs.has_assets == 'true'", True),
        ("${{ steps.assets.outputs.has_assets == 'true' }}", True),
        ("other.steps.assets.outputs.has_assets", False),
        ("steps.assets.outputs.has_assets_extra", False),
        ("needs.build-dependency-binaries.result != 'failure'", False),
    ],
)
def test_github_expression_mentions_operand(
    expression: str,
    expected: bool,
) -> None:
    """Report whether the expression mentions the operand as a whole token."""
    assert _github_expression_mentions_operand(expression, OPERAND) is expected


@pytest.mark.parametrize(
    ("expression", "expected"),
    [
        ("!steps.assets.outputs.has_assets", True),
        ("! steps.assets.outputs.has_assets", True),
        ("!(steps.assets.outputs.has_assets)", True),
        ("!!steps.assets.outputs.has_assets", False),
        ("steps.assets.outputs.has_assets == 'true'", False),
    ],
)
def test_github_expression_negates_operand(
    expression: str,
    expected: bool,
) -> None:
    """Report whether the expression contains a real negation."""
    assert _github_expression_negates_operand(expression, OPERAND) is expected


@pytest.mark.parametrize(
    ("expression", "expected"),
    [
        ("steps.assets.outputs.has_assets == false", True),
        ("steps.assets.outputs.has_assets == 'false'", True),
        ('steps.assets.outputs.has_assets == "false"', True),
        ("false == steps.assets.outputs.has_assets", True),
        ("steps.assets.outputs.has_assets  ==  false", True),
        ("steps.assets.outputs.has_assets != 'false'", False),
        ("steps.assets.outputs.has_assets == 'true'", False),
    ],
)
def test_github_expression_compares_operand_to_false(
    expression: str,
    expected: bool,
) -> None:
    """Report whether the expression uses equality against false."""
    assert _github_expression_compares_operand_to_false(
        expression,
        OPERAND,
    ) is expected

"""The one boundary that reads the nextest configuration from disk.

The timeout contracts ask many questions of `.config/nextest.toml`, and
each asks it of a parsed document or of text handed to it. Reading the
file is done here and nowhere else, so a missing, unreadable, undecodable
or malformed configuration fails as ``NextestConfigurationError`` naming
the file, rather than as whatever the failure produced at collection.

Run via ``make test-workflow-contracts``.
"""

import pathlib
import tomllib
import typing as typ

from nextest_durations import NextestConfigurationError
from timeout_budgets import NEXTEST_CONFIG


def nextest_config_text(path: pathlib.Path = NEXTEST_CONFIG) -> str:
    """Return the nextest configuration's text, or fail naming the file.

    The one place the configuration is read from disk. A missing or
    unreadable file, and one that is not UTF-8, each raise the same
    named error rather than whatever the failure happened to produce, so
    a contract that could not read its input says so instead of failing
    somewhere unrelated at collection.

    Parameters
    ----------
    path : pathlib.Path
        The configuration to read; the repository's own by default.

    Returns
    -------
    str
        The file's text.

    Raises
    ------
    NextestConfigurationError
        If the file cannot be read or is not UTF-8.
    """
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        message = f"cannot read the nextest configuration {path}: {error}"
        raise NextestConfigurationError(message) from error


def load_nextest_config(path: pathlib.Path = NEXTEST_CONFIG) -> dict[str, typ.Any]:
    """Return the nextest configuration, read and parsed at one boundary.

    Parameters
    ----------
    path : pathlib.Path
        The configuration to read; the repository's own by default.

    Returns
    -------
    dict[str, typ.Any]
        The parsed document.

    Raises
    ------
    NextestConfigurationError
        If the file cannot be read, is not UTF-8, or is not valid TOML.
    """
    try:
        return tomllib.loads(nextest_config_text(path))
    except tomllib.TOMLDecodeError as error:
        message = f"the nextest configuration {path} is not valid TOML: {error}"
        raise NextestConfigurationError(message) from error

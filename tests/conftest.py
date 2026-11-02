"""Shared fixtures: the fixture projects and pages generated from them by the real program."""

from collections.abc import Callable
from pathlib import Path

import pytest

from chartedcode._binary import BinaryNotFoundError, find_binary, run_binary

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "rust" / "tests" / "fixtures"
RUST = ROOT / "rust"


def _rust_sources() -> list[Path]:
    """The files the installed executable and extension are built from."""
    return [
        *(RUST / "src").rglob("*.rs"),
        *(RUST / "python" / "src").rglob("*.rs"),
        *(RUST / "assets").glob("*"),
        RUST / "Cargo.toml",
        RUST / "python" / "Cargo.toml",
    ]


def pytest_sessionstart(session: pytest.Session) -> None:
    """Refuse to run against an executable older than the Rust sources.

    `uv run` re-syncs the project and can put back an older cached build over the one `uv run inv build`
    installed; a stale executable that still works would pass these tests without being the code under test.
    """
    del session
    try:
        installed = find_binary().stat().st_mtime
    except BinaryNotFoundError as error:
        pytest.exit(f"{error}", returncode=1)
    newest = max(path.stat().st_mtime for path in _rust_sources())
    if installed < newest:
        pytest.exit(
            "The installed chartedcode executable is older than the Rust sources. "
            "Run `uv run inv build`, then run the tests with `uv run --no-sync pytest` "
            "(or just `uv run inv test`, which builds first).",
            returncode=1,
        )


@pytest.fixture(scope="session")
def fixtures() -> Path:
    """The directory of fixture projects shared with the Rust tests."""
    return FIXTURES


@pytest.fixture(scope="session")
def render_page(tmp_path_factory: pytest.TempPathFactory) -> Callable[[str, str], Path]:
    """A function that draws `view` of the fixture project `project` and returns the page's path."""

    def render(view: str, project: str) -> Path:
        out = tmp_path_factory.mktemp(f"{view}_{project}") / f"{view}.html"
        done = run_binary([view, str(FIXTURES / project), "--no-gitignore", "-o", str(out)], capture=True)
        assert done.returncode == 0, done.stderr
        return out

    return render

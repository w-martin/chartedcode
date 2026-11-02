"""The command line end to end: the installed executable, the in-process extension, and their agreement."""

import json
import subprocess
import sys
import tomllib
from importlib.metadata import version
from pathlib import Path

import pytest

from chartedcode._binary import IN_PROCESS_VARIABLE, run_binary

from .conftest import ROOT

EXIT_FAILURE = 1
EXIT_USAGE = 2


def test_the_version_is_the_same_everywhere() -> None:
    """pyproject, both Cargo manifests, the installed metadata and the executable all agree."""
    declared = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))["project"]["version"]
    for manifest in ("rust/Cargo.toml", "rust/python/Cargo.toml"):
        cargo = tomllib.loads((ROOT / manifest).read_text(encoding="utf-8"))["package"]["version"]
        assert cargo == declared, manifest
    assert version("chartedcode") == declared
    assert run_binary(["--version"], capture=True).stdout.strip() == f"chartedcode {declared}"


def test_the_executable_and_the_extension_write_identical_json(monkeypatch: pytest.MonkeyPatch, fixtures: Path) -> None:
    """Running inside the interpreter changes nothing about the result."""
    arguments = ["flow", str(fixtures / "calls"), "--format", "json", "--no-gitignore"]
    monkeypatch.delenv(IN_PROCESS_VARIABLE, raising=False)
    separate = run_binary(arguments, capture=True)
    monkeypatch.setenv(IN_PROCESS_VARIABLE, "1")
    inside = run_binary(arguments, capture=True)
    assert separate.returncode == inside.returncode == 0
    assert separate.stdout == inside.stdout
    assert json.loads(separate.stdout)["schema_version"] == 1


@pytest.mark.parametrize("view", ["classes", "calls", "flow"])
def test_the_executable_and_the_extension_write_identical_pages(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, fixtures: Path, view: str
) -> None:
    """The same bytes either way, and the same bytes on a second run."""
    pages: list[str] = []
    for index, in_process in enumerate(("0", "1", "0")):
        monkeypatch.setenv(IN_PROCESS_VARIABLE, in_process)
        out = tmp_path / f"{view}{index}.html"
        done = run_binary([view, str(fixtures / "calls"), "--no-gitignore", "-o", str(out)], capture=True)
        assert done.returncode == 0, done.stderr
        pages.append(out.read_text(encoding="utf-8"))
    assert pages[0] == pages[1] == pages[2]
    assert pages[0].startswith("<!doctype html>")


def test_the_default_page_name_is_announced_and_written_in_the_current_directory(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, fixtures: Path
) -> None:
    """Without `-o` the page is `<project>-<view>.chartedcode.html`, and the program says so."""
    monkeypatch.chdir(tmp_path)
    done = run_binary(["calls", str(fixtures / "calls"), "--no-gitignore"], capture=True)
    assert done.returncode == 0, done.stderr
    assert done.stdout.strip() == "wrote calls-calls.chartedcode.html"
    assert (tmp_path / "calls-calls.chartedcode.html").read_text(encoding="utf-8").startswith("<!doctype html>")


def test_failures_have_exit_statuses_and_messages(fixtures: Path) -> None:
    """A usage error is 2, a failed analysis is 1 and names the path."""
    usage = run_binary(["nonsense"], capture=True)
    assert usage.returncode == EXIT_USAGE
    missing = run_binary(["calls", str(fixtures / "missing"), "--format", "json"], capture=True)
    assert missing.returncode == EXIT_FAILURE
    assert "cannot read" in missing.stderr
    assert str(fixtures / "missing") in missing.stderr


def test_the_gitignore_is_respected_unless_turned_off(tmp_path: Path) -> None:
    """Ignored files are left out by default and included with `--no-gitignore`."""
    (tmp_path / ".gitignore").write_text("skipped.py\n", encoding="utf-8")
    (tmp_path / "kept.py").write_text("def kept():\n    pass\n", encoding="utf-8")
    (tmp_path / "skipped.py").write_text("def skipped():\n    pass\n", encoding="utf-8")

    def names(*extra: str) -> set[str]:
        done = run_binary(["calls", str(tmp_path), "--format", "json", *extra], capture=True)
        assert done.returncode == 0, done.stderr
        return {f["name"] for f in json.loads(done.stdout)["functions"]}

    assert names() == {"kept"}
    assert names("--no-gitignore") == {"kept", "skipped"}


@pytest.mark.parametrize("arguments", [["-m", "chartedcode"], None])
def test_both_entry_points_start_the_program(arguments: list[str] | None) -> None:
    """`python -m chartedcode` and `chartedcode-py` print the version."""
    command = [sys.executable, *arguments] if arguments else [str(Path(sys.executable).parent / "chartedcode-py")]
    done = subprocess.run([*command, "--version"], capture_output=True, text=True, check=False)
    assert done.returncode == 0, done.stderr
    assert done.stdout.strip() == f"chartedcode {version('chartedcode')}"

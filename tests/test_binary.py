"""Finding and running the executable, and the in-process mode."""

import os
import runpy
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

from chartedcode import _binary
from chartedcode._binary import BinaryNotFoundError

EXIT_FAILURE = 3
EXIT_USAGE = 2


class FakeExtension:
    """Stands in for the compiled extension module."""

    @staticmethod
    def main(argv: list[str]) -> int:
        """Pretend to run; the status is the number of arguments after the program name."""
        return len(argv) - 1

    @staticmethod
    def run_captured(argv: list[str]) -> tuple[int, str, str]:
        """Pretend to run and capture."""
        return 0, f"out:{' '.join(argv)}", "err"


def scripts_layout(monkeypatch: pytest.MonkeyPatch, tmp_path: Path, *, where: str) -> Path:
    """Point the interpreter's script directories at `tmp_path`, with an executable in `where` only."""
    directories = {"scripts": tmp_path / "scripts", "user": tmp_path / "user", "target": tmp_path / "site" / "bin"}
    for directory in directories.values():
        directory.mkdir(parents=True, exist_ok=True)
    monkeypatch.setattr(_binary, "get_config_var", lambda _name: "")
    monkeypatch.setattr(_binary, "get_preferred_scheme", lambda _scheme: "user")
    monkeypatch.setattr(
        _binary,
        "get_path",
        lambda _name, scheme=None: str(directories["user" if scheme else "scripts"]),
    )
    monkeypatch.setattr(_binary, "__file__", str(tmp_path / "site" / "chartedcode" / "_binary.py"))
    monkeypatch.setattr(_binary.shutil, "which", lambda _name: None)
    executable = directories[where] / "chartedcode"
    executable.write_text("#!/bin/sh\n", encoding="utf-8")
    return executable


@pytest.mark.parametrize("where", ["scripts", "user", "target"])
def test_the_executable_is_found_in_each_install_location(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, where: str
) -> None:
    """The scripts directory, the user's scripts directory and a `--target` install's `bin` are all searched."""
    expected = scripts_layout(monkeypatch, tmp_path, where=where)
    assert _binary.find_binary() == expected


def test_the_executable_is_found_on_path_as_a_last_resort(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    """Failing the install locations, an executable on PATH is used."""
    scripts_layout(monkeypatch, tmp_path, where="scripts").unlink()
    on_path = tmp_path / "elsewhere" / "chartedcode"
    monkeypatch.setattr(_binary.shutil, "which", lambda _name: str(on_path))
    assert _binary.find_binary() == on_path


def test_a_missing_executable_says_how_to_get_it(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    """The error names where it looked and how to build it."""
    scripts_layout(monkeypatch, tmp_path, where="scripts").unlink()
    with pytest.raises(BinaryNotFoundError, match=r"executable was not found.*uv run inv build"):
        _binary.find_binary()


@pytest.mark.parametrize(
    ("value", "requested"),
    [
        (None, False),
        ("", False),
        ("0", False),
        ("false", False),
        ("No", False),
        (" off ", False),
        ("1", True),
        ("yes", True),
    ],
)
def test_the_environment_variable_asks_for_in_process_mode(
    monkeypatch: pytest.MonkeyPatch, value: str | None, *, requested: bool
) -> None:
    """Empty and false-like values leave it off; anything else asks for it."""
    if value is None:
        monkeypatch.delenv(_binary.IN_PROCESS_VARIABLE, raising=False)
    else:
        monkeypatch.setenv(_binary.IN_PROCESS_VARIABLE, value)
    assert _binary._requested_in_process() is requested


def test_the_extension_is_imported_or_reported_missing(monkeypatch: pytest.MonkeyPatch) -> None:
    """An import failure becomes the same error as a missing executable."""
    monkeypatch.setattr(_binary, "import_module", lambda _name: FakeExtension)
    assert _binary._extension() is FakeExtension

    def refuse(_name: str) -> None:
        raise ImportError

    monkeypatch.setattr(_binary, "import_module", refuse)
    with pytest.raises(BinaryNotFoundError, match="extension module"):
        _binary._extension()


def test_executable_missing_reports_both_ways(monkeypatch: pytest.MonkeyPatch) -> None:
    """The helper is true exactly when the lookup fails."""
    monkeypatch.setattr(_binary, "find_binary", lambda: Path("here"))
    assert _binary._executable_missing() is False

    def fail() -> Path:
        raise BinaryNotFoundError

    monkeypatch.setattr(_binary, "find_binary", fail)
    assert _binary._executable_missing() is True


@pytest.mark.parametrize(
    ("requested", "missing", "has_extension", "expected"),
    [(True, False, False, True), (False, False, True, False), (False, True, True, True), (False, True, False, False)],
)
def test_in_process_mode_is_chosen_when_asked_or_when_only_the_extension_exists(
    monkeypatch: pytest.MonkeyPatch, *, requested: bool, missing: bool, has_extension: bool, expected: bool
) -> None:
    """In-process: when asked to, or when the executable is missing but the extension is not."""
    monkeypatch.setattr(_binary, "_requested_in_process", lambda: requested)
    monkeypatch.setattr(_binary, "_executable_missing", lambda: missing)

    def extension() -> type[FakeExtension]:
        if not has_extension:
            raise BinaryNotFoundError
        return FakeExtension

    monkeypatch.setattr(_binary, "_extension", extension)
    assert _binary._in_process() is expected


def test_run_binary_in_process_returns_the_status_or_the_captured_output(monkeypatch: pytest.MonkeyPatch) -> None:
    """Both in-process forms pass the program name first."""
    monkeypatch.setattr(_binary, "_in_process", lambda: True)
    monkeypatch.setattr(_binary, "_extension", lambda: FakeExtension)
    shared = _binary.run_binary(["a", "b"])
    assert shared.returncode == EXIT_USAGE
    captured = _binary.run_binary(["a"], capture=True)
    assert (captured.returncode, captured.stdout, captured.stderr) == (0, "out:chartedcode a", "err")


def test_run_binary_starts_the_installed_executable() -> None:
    """The real executable answers `--version`."""
    done = _binary.run_binary(["--version"], capture=True)
    assert done.returncode == 0
    assert done.stdout.startswith("chartedcode ")


def test_main_runs_in_process_and_exits_with_its_status(monkeypatch: pytest.MonkeyPatch) -> None:
    """In-process mode exits with the program's status."""
    monkeypatch.setattr(_binary, "_in_process", lambda: True)
    monkeypatch.setattr(_binary, "_extension", lambda: FakeExtension)
    monkeypatch.setattr(sys, "argv", ["chartedcode", "x", "y", "z"])
    with pytest.raises(SystemExit) as stopped:
        _binary.main()
    assert stopped.value.code == EXIT_FAILURE


def test_main_runs_a_child_where_there_is_no_exec(monkeypatch: pytest.MonkeyPatch) -> None:
    """Windows has no `exec`, so the executable runs as a child."""
    monkeypatch.setattr(sys, "platform", "win32")
    monkeypatch.setattr(_binary, "run_binary", lambda _args: subprocess.CompletedProcess([], EXIT_FAILURE))
    monkeypatch.setattr(sys, "argv", ["chartedcode", "x"])
    with pytest.raises(SystemExit) as stopped:
        _binary.main()
    assert stopped.value.code == EXIT_FAILURE


def test_main_replaces_this_process_with_the_executable(monkeypatch: pytest.MonkeyPatch) -> None:
    """Elsewhere the executable takes this process's place, with these arguments."""
    monkeypatch.setattr(sys, "platform", "linux")
    monkeypatch.setattr(_binary, "_in_process", lambda: False)
    monkeypatch.setattr(_binary, "find_binary", lambda: Path("/bin/chartedcode"))
    calls: list[tuple[str, list[str]]] = []
    monkeypatch.setattr(os, "execv", lambda path, argv: calls.append((path, argv)))
    monkeypatch.setattr(sys, "argv", ["chartedcode", "calls", "."])
    _binary.main()
    assert calls == [("/bin/chartedcode", ["/bin/chartedcode", "calls", "."])]


def test_main_in_process_never_starts_the_executable(monkeypatch: pytest.MonkeyPatch) -> None:
    """`chartedcode-py` always uses the extension."""
    monkeypatch.setattr(_binary, "_extension", lambda: FakeExtension)
    monkeypatch.setattr(sys, "argv", ["chartedcode-py", "a"])
    with pytest.raises(SystemExit) as stopped:
        _binary.main_in_process()
    assert stopped.value.code == 1


def test_python_dash_m_runs_main(monkeypatch: pytest.MonkeyPatch) -> None:
    """`python -m chartedcode` calls the shim's `main`."""
    called = SimpleNamespace(count=0)

    def fake_main() -> None:
        called.count += 1

    monkeypatch.setattr(_binary, "main", fake_main)
    runpy.run_module("chartedcode", run_name="__main__")
    assert called.count == 1

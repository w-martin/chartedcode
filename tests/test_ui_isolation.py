"""What a page does to its surroundings: it requests nothing, and sets no cookies or stored data."""

from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from .ui_support import drive, requires_chrome

pytestmark = [pytest.mark.ui, requires_chrome]

JS = Path(__file__).parent / "js"
PAGES = [("classes", "imports"), ("calls", "calls"), ("flow", "flow")]


def read(name: str) -> str:
    """The text of a script in `tests/js`."""
    return (JS / name).read_text(encoding="utf-8")


@pytest.mark.parametrize(("view", "project"), PAGES, ids=[view for view, _ in PAGES])
def test_a_page_in_use_requests_nothing_and_stores_nothing(
    render_page: Callable[[str, str], Path], view: str, project: str
) -> None:
    """From before its first script, through loading, changing state and laying out again: no attempt to reach out."""
    seen = drive(render_page(view, project), read("isolation.js"), early=read("net_recorder.js"))
    assert seen["boxes"] > 0, "the page drew nothing, so the check proves nothing"
    assert seen["attempts"] == []
    assert seen["cookie"] == ""
    assert all(size in {0, "unavailable"} for size in seen["storage"].values())


def test_the_recorder_does_see_an_image_a_fetch_and_a_script(render_page: Callable[[str, str], Path]) -> None:
    """The positive control: the same recorder reports deliberate requests, so a clean result above means something."""
    seen: dict[str, Any] = drive(
        render_page("calls", "calls"), read("isolation_control.js"), early=read("net_recorder.js")
    )
    kinds = sorted(attempt.split(":")[0] for attempt in seen["attempts"])
    assert "img" in kinds
    assert "fetch" in kinds
    assert any(kind.startswith("element <script") for kind in kinds)

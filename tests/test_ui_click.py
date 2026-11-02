"""Click-to-focus on the call-graph page: one click arms, a second does it, a drag never counts."""

from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from .ui_support import drive, requires_chrome

pytestmark = [pytest.mark.ui, requires_chrome]

FOCUSED = "Service.run"
OTHER = "Tool.work"

SCRIPT = (
    (Path(__file__).parent / "js" / "click.js")
    .read_text(encoding="utf-8")
    .replace("__FOCUSED__", FOCUSED)
    .replace("__OTHER__", OTHER)
)


@pytest.fixture(scope="module")
def steps(render_page: Callable[[str, str], Path]) -> dict[str, dict[str, Any]]:
    """The page's state after each action, by label."""
    recorded = drive(render_page("calls", "calls"), SCRIPT, "gran=function")
    return {entry["label"]: entry for entry in recorded}


def test_the_status_line_is_always_visible(steps: dict[str, dict[str, Any]]) -> None:
    """The line that says what a click will do never disappears."""
    assert all(entry["hintVisible"] for entry in steps.values())


def test_the_first_click_arms_and_says_what_the_second_will_do(steps: dict[str, dict[str, Any]]) -> None:
    """Nothing is focused yet; the box is armed and the line names it."""
    idle, armed = steps["idle"], steps["click A once"]
    assert idle["hint"].startswith("No focus")
    assert idle["clearDisabled"]
    assert armed["hint"].startswith(f"Click “{FOCUSED}” again to focus on it")
    assert armed["armed"]
    assert not armed["focused"]
    assert armed["focus"] == ""


def test_the_second_click_focuses(steps: dict[str, dict[str, Any]]) -> None:
    """The focus is set, the Clear button enables, and nothing is armed any more."""
    done = steps["click A twice"]
    assert done["hint"].startswith(f"Focused on “{FOCUSED}”")
    assert done["focus"].endswith(FOCUSED)
    assert not done["clearDisabled"]
    assert done["focused"]
    assert not done["armed"]


def test_arming_another_box_leaves_the_focus_alone(steps: dict[str, dict[str, Any]]) -> None:
    """One click on a different box only arms it."""
    entry = steps["click B once while A focused"]
    assert entry["hint"].startswith(f"Click “{OTHER}” again to focus on it")
    assert entry["focus"].endswith(FOCUSED)


def test_empty_space_arms_clearing_and_escape_cancels(steps: dict[str, dict[str, Any]]) -> None:
    """Clicking empty space arms "clear focus"; Escape puts the status back without clearing."""
    assert steps["click empty once"]["hint"].startswith("Click empty space again to clear")
    assert steps["click empty once"]["focus"].endswith(FOCUSED)
    assert steps["Escape"]["hint"].startswith(f"Focused on “{FOCUSED}”")
    assert steps["Escape"]["focus"].endswith(FOCUSED)


def test_two_clicks_on_empty_space_clear_the_focus(steps: dict[str, dict[str, Any]]) -> None:
    """The focus goes, and the Clear button disables again."""
    done = steps["click empty twice"]
    assert done["hint"].startswith("No focus")
    assert done["focus"] == ""
    assert done["clearDisabled"]


def test_clicking_the_focused_box_twice_clears_it(steps: dict[str, dict[str, Any]]) -> None:
    """The first click says it will clear the focus; the second does."""
    assert steps["refocus A"]["focused"]
    assert steps["click focused A once"]["hint"].startswith(f"Click “{FOCUSED}” again to clear the focus")
    assert steps["click focused A once"]["focus"].endswith(FOCUSED)
    assert steps["click focused A twice"]["focus"] == ""


def test_a_drag_is_not_a_click(steps: dict[str, dict[str, Any]]) -> None:
    """Dragging from a box pans and arms nothing."""
    done = steps["drag from a box"]
    assert done["hint"].startswith("No focus")
    assert not done["armed"]
    assert done["focus"] == ""

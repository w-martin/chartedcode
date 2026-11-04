"""Host options in the URL fragment, on every page: `embed=1` and `theme=light|dark`, also applied after load."""

from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from .ui_support import drive, requires_chrome

pytestmark = [pytest.mark.ui, requires_chrome]

LIGHT_BACKGROUND = "#fff"
DARK_BACKGROUND = "#0f172a"
PAGES = [("classes", "imports"), ("calls", "calls"), ("flow", "flow")]


@pytest.fixture(scope="module", params=PAGES, ids=[view for view, _ in PAGES])
def states(request: pytest.FixtureRequest, render_page: Callable[[str, str], Path]) -> dict[str, Any]:
    """The page's state after each change of its URL fragment."""
    view, project = request.param
    script = (Path(__file__).parent / "js" / "embed.js").read_text(encoding="utf-8")
    return drive(render_page(view, project), script)


def test_the_page_starts_in_its_full_form_with_the_system_theme(states: dict[str, Any]) -> None:
    """No fragment: header shown, no theme override, and the colours of whichever theme the system uses."""
    initial = states["initial"]
    assert not initial["embed"]
    assert initial["theme"] is None
    assert initial["headerHeight"] > 0
    assert initial["bg"] in {LIGHT_BACKGROUND, DARK_BACKGROUND}
    assert initial["boxes"] > 0


def test_embed_hides_the_header_and_gives_the_diagram_the_room(states: dict[str, Any]) -> None:
    """The header and legend go; the diagram grows; the status line stays where there is one."""
    embedded = states["embed"]
    assert embedded["embed"]
    assert embedded["headerHeight"] == 0
    assert embedded["svgHeight"] > states["initial"]["svgHeight"]
    assert embedded["hintVisible"] in {True, None}
    assert embedded["boxes"] == states["initial"]["boxes"]


def test_the_theme_can_be_forced_either_way(states: dict[str, Any]) -> None:
    """The colours follow the option, not the system setting (whichever it is); and with embed too."""
    assert states["dark"]["theme"] == "dark"
    assert states["dark"]["bg"] == DARK_BACKGROUND
    assert states["light"]["theme"] == "light"
    assert states["light"]["bg"] == LIGHT_BACKGROUND
    assert states["embedDark"]["embed"]
    assert states["embedDark"]["bg"] == DARK_BACKGROUND


def test_an_unknown_theme_is_ignored_and_removing_the_options_restores_the_defaults(states: dict[str, Any]) -> None:
    """Only `light` and `dark` mean anything; an empty fragment undoes everything."""
    system = states["initial"]["bg"]
    assert states["invalid"]["theme"] is None
    assert states["invalid"]["bg"] == system
    reset = states["reset"]
    assert not reset["embed"]
    assert reset["theme"] is None
    assert reset["bg"] == system
    assert reset["headerHeight"] == states["initial"]["headerHeight"]

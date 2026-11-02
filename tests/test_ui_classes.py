"""The class-diagram page in a real browser: relationships can be shown and hidden by kind."""

import json
import re
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from chartedcode._binary import run_binary

from .conftest import FIXTURES
from .ui_support import drive, requires_chrome

pytestmark = [pytest.mark.ui, requires_chrome]


def relationships(stats: str) -> int:
    """The number of relationships a status line reports ("N classes · M relationships · ...")."""
    found = re.search(r"(\d+) relationships", stats)
    assert found, stats
    return int(found.group(1))


@pytest.fixture(scope="module")
def log(render_page: Callable[[str, str], Path]) -> dict[str, Any]:
    """The page's state with the defaults, with has-a shown, and with every kind hidden."""
    script = (Path(__file__).parent / "js" / "classes.js").read_text(encoding="utf-8")
    return drive(render_page("classes", "imports"), script)


@pytest.fixture(scope="module")
def counts() -> dict[str, int]:
    """How many relationships of each kind the program found in the imports fixture."""
    done = run_binary(["classes", str(FIXTURES / "imports"), "--format", "json", "--no-gitignore"], capture=True)
    return json.loads(done.stdout)["counts"]


def test_inheritance_and_protocol_conformance_start_shown_and_has_a_starts_hidden(
    log: dict[str, Any], counts: dict[str, int]
) -> None:
    """The defaults match the data: inherits and implements on, has-a off."""
    initial = log["initial"]
    assert (initial["inherits"], initial["implementsOn"], initial["composes"]) == (True, True, False)
    assert relationships(initial["stats"]) == counts["inherits"] + counts["implements"]


def test_ticking_has_a_adds_exactly_the_field_type_relationships(log: dict[str, Any], counts: dict[str, int]) -> None:
    """Every kind together equals the program's total."""
    assert relationships(log["withComposes"]["stats"]) == sum(counts.values())
    assert log["withComposes"]["boxes"] >= log["initial"]["boxes"]


def test_hiding_every_kind_leaves_no_relationships(log: dict[str, Any]) -> None:
    """With nothing ticked the page reports none."""
    assert relationships(log["none"]["stats"]) == 0

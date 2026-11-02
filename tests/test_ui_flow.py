"""The control-flow page in a real browser: what is drawn, the filters, and opening a callee's flow."""

import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from chartedcode._binary import run_binary

from .conftest import FIXTURES
from .ui_support import drive, requires_chrome

pytestmark = [pytest.mark.ui, requires_chrome]

JS = Path(__file__).parent / "js"
MAIN = "pkg/app::main"
CALLEE = "pkg/app::Service.__init__"


def flow_data(project: str) -> dict[str, dict[str, Any]]:
    """The control-flow graphs of a fixture project, by function id, as the program writes them."""
    done = run_binary(["flow", str(FIXTURES / project), "--format", "json", "--no-gitignore"], capture=True)
    assert done.returncode == 0, done.stderr
    return {g["function"]: g for g in json.loads(done.stdout)["cfgs"]}


def named_labels(graph: dict[str, Any]) -> list[str]:
    """The labels on a graph's edges, ignoring plain flow, sorted."""
    return sorted(e["label"] for e in graph["edges"] if e["label"])


def joins(graph: dict[str, Any]) -> int:
    """How many blocks hold no statements and no calls and are neither start nor end: the join points."""
    return sum(1 for b in graph["blocks"] if not b["lines"] and not b["calls"] and b["kind"] not in {"entry", "exit"})


@pytest.fixture(scope="module")
def graphs() -> dict[str, dict[str, Any]]:
    """The flow fixture's graphs, straight from the program."""
    return flow_data("flow")


@pytest.fixture(scope="module")
def shapes(render_page: Callable[[str, str], Path]) -> dict[str, Any]:
    """What the page shows for several functions of the flow fixture."""
    return drive(render_page("flow", "flow"), (JS / "flow_shapes.js").read_text(encoding="utf-8"))


@pytest.fixture(scope="module")
def navigation(render_page: Callable[[str, str], Path]) -> dict[str, Any]:
    """The page's state while opening a callee's flow, going back, cancelling and dragging."""
    return drive(
        render_page("flow", "calls"), (JS / "flow_navigation.js").read_text(encoding="utf-8"), f"function={MAIN}"
    )


def test_the_most_complex_function_is_shown_first_and_all_are_listed(
    shapes: dict[str, Any], graphs: dict[str, Any]
) -> None:
    """Ties in complexity are broken by name; every function with a graph can be chosen."""
    top = max(g["complexity"] for g in graphs.values())
    first = min(f for f, g in graphs.items() if g["complexity"] == top)
    assert shapes["initial"]["fn"] == first
    assert sorted(o[0] for o in shapes["initial"]["options"]) == sorted(graphs)


@pytest.mark.parametrize(("snapshot", "function"), [("branch", "flow::branch"), ("loop", "flow::loop_while_break")])
def test_the_page_draws_the_graph_it_was_given(
    shapes: dict[str, Any], graphs: dict[str, Any], snapshot: str, function: str
) -> None:
    """One box per block (joins are dots), one arrow per edge, and a label on each named edge."""
    graph, shown = graphs[function], shapes[snapshot]
    assert shown["blocks"] == len(graph["blocks"])
    assert shown["edges"] >= len(graph["edges"])
    assert sorted(shown["labels"]) == named_labels(graph)
    assert shown["circles"] == joins(graph)
    assert f"complexity {graph['complexity']}" in shown["stats"]


def test_a_branch_is_labelled_true_and_false(shapes: dict[str, Any]) -> None:
    """The if/else of `branch` shows its two outcomes."""
    assert sorted(shapes["branch"]["labels"]) == ["false", "true"]


def test_unreachable_code_is_marked_in_the_block_and_in_the_status_line(shapes: dict[str, Any]) -> None:
    """Code after `while True` and after `return` says so."""
    assert any("(unreachable)" in text for text in shapes["infinite"]["blockText"])
    assert "unreachable" in shapes["infinite"]["hint"]
    assert any("(unreachable)" in text for text in shapes["afterReturn"]["blockText"])


def test_the_complexity_filter_keeps_the_busy_functions_and_the_one_shown(
    shapes: dict[str, Any], graphs: dict[str, Any]
) -> None:
    """At complexity 3 or more only those functions are listed, busiest first; the filter is undone by "all"."""
    minimum = 3
    expected = {f for f, g in graphs.items() if g["complexity"] >= minimum}
    listed = [o[0] for o in shapes["minc3"]["options"]]
    assert set(listed) == expected
    complexities = [graphs[f]["complexity"] for f in listed]
    assert complexities == sorted(complexities, reverse=True)
    assert len(shapes["minc0"]["options"]) == len(graphs)


def test_hovering_a_block_dims_the_other_edges(shapes: dict[str, Any]) -> None:
    """Some edges fade while a block is hovered, and all return afterwards."""
    hover = shapes["hover"]
    assert 0 < hover["during"] < hover["paths"]
    assert hover["after"] == 0


def test_the_start_is_the_function_asked_for_with_a_call_that_has_a_flow(navigation: dict[str, Any]) -> None:
    """`main` calls `Service(...)`, whose constructor has a flow of its own."""
    assert navigation["start"]["fn"] == MAIN
    assert navigation["start"]["hasCalleeLine"]
    assert navigation["start"]["backDisabled"]


def test_one_click_on_a_call_arms_it_and_says_what_the_second_will_do(navigation: dict[str, Any]) -> None:
    """Nothing opens yet; the call is highlighted and the status line names the target."""
    armed = navigation["armed"]
    assert armed["fn"] == MAIN
    assert armed["armed"] == CALLEE
    assert armed["armedLines"] >= 1
    assert armed["hint"].startswith("Click “Service()” again to open its flow")


def test_the_second_click_opens_the_callee_and_back_returns(navigation: dict[str, Any]) -> None:
    """The callee's flow is shown, Back becomes available, and Back goes to where we were."""
    opened, back = navigation["opened"], navigation["back"]
    assert opened["fn"] == CALLEE
    assert opened["history"] == 1
    assert not opened["backDisabled"]
    assert opened["armed"] is None
    assert back["fn"] == MAIN
    assert back["backDisabled"]


def test_escape_cancels_and_a_drag_is_not_a_click(navigation: dict[str, Any]) -> None:
    """Neither opens anything or leaves a call armed."""
    for step in ("escape", "drag"):
        assert navigation[step]["armed"] is None, step
        assert navigation[step]["fn"] == MAIN, step
        assert navigation[step]["armedLines"] == 0, step


def test_the_status_line_stays_visible_and_private_functions_can_be_listed(navigation: dict[str, Any]) -> None:
    """The line never disappears; ticking private adds functions to the list."""
    assert all(navigation[step]["hintVisible"] for step in navigation)
    assert navigation["privateShown"]["options"] > navigation["start"]["options"]

"""The call-graph page in a real browser: call order, the derived views, filters and the steps panel."""

import itertools
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from .ui_support import drive, requires_chrome

pytestmark = [pytest.mark.ui, requires_chrome]

A, U, R = "pkg/app::", "pkg/util::", "pkg/rec::"
P, INT = "pkg/priv::", "pkg/_internal::"
RECURSIVE_STEPS = 2
FIRST_STEPS = [str(i) for i in range(1, 12)]

SCRIPT = (Path(__file__).parent / "js" / "calls.js").read_text(encoding="utf-8")

HIDDEN = {
    P + "_private_fn",
    P + "_PrivateClass.run",
    P + "Public._hidden_method",
    INT + "internal_fn",
    P + "_PrivateClass.__init__",
}


@pytest.fixture(scope="module")
def log(render_page: Callable[[str, str], Path]) -> dict[str, Any]:
    """Every scenario the script walks through, recorded once."""
    return drive(render_page("calls", "calls"), SCRIPT, "gran=function")


def is_dag(edges: list[list[str]]) -> bool:
    """Whether the edges, taken as directed, hold no cycle."""
    nodes = {n for e in edges for n in e}
    indegree = dict.fromkeys(nodes, 0)
    for _, target in edges:
        indegree[target] += 1
    queue = [n for n, d in indegree.items() if d == 0]
    seen = 0
    while queue:
        node = queue.pop()
        seen += 1
        for source, target in edges:
            if source == node:
                indegree[target] -= 1
                if indegree[target] == 0:
                    queue.append(target)
    return seen == len(nodes)


def ids(snapshot: dict[str, Any]) -> set[str]:
    """The ids of the boxes drawn in a snapshot."""
    return {g["id"] for g in snapshot["groups"]}


def test_first_layer_callees_share_a_layer_and_follow_call_order(log: dict[str, Any]) -> None:
    """Callees that call each other sit one layer further along; the rest run top-to-bottom in call order."""
    groups = {g["id"]: g for g in log["s1"]["groups"]}
    layer1 = [U + "other", A + "Port.send", U + "Tool.work", U + "Sub.go", U + "Tool.make"]
    assert len({groups[k]["x"] for k in layer1}) == 1
    heights = [groups[k]["y"] for k in layer1]
    assert heights == sorted(heights)


def test_step_numbers_are_drawn_on_the_arrows(log: dict[str, Any]) -> None:
    """The two extra "1" badges are the calls between callees."""
    expected = ["1,10", "2", "3,11", "4,5", "6", "7", "8", "1", "1"]
    assert sorted(log["s1"]["badges"]) == sorted(expected)


def test_the_steps_panel_lists_every_step_in_order(log: dict[str, Any]) -> None:
    """Steps 1 to 11 of `Service.run`, in order, with the callback step marked."""
    snapshot = log["s1"]
    assert snapshot["stepNums"] == FIRST_STEPS
    assert "Port.send" in snapshot["stepLines"][2]
    assert "passes callback to helper" in snapshot["stepLines"][9]


def test_untraced_calls_are_explained_in_the_panel(log: dict[str, Any]) -> None:
    """The panel says how many calls were not traced and which."""
    note = log["s1"]["note"]
    assert "3 calls not traced" in note
    assert "items.append" in note


def test_the_execution_flow_has_a_step_per_traced_call(log: dict[str, Any]) -> None:
    """The external step (json.dumps, step 9) is hidden until external calls are ticked."""
    present = [k for k in range(1, 12) if f"s:{A}Service.run#{k}" in ids(log["s2"])]
    assert present == [1, 2, 3, 4, 5, 6, 7, 8, 10, 11]


def test_the_execution_flow_is_acyclic_chained_in_order_and_shows_dispatch(log: dict[str, Any]) -> None:
    """Steps follow each other, and a Protocol method leads to its implementers."""
    snapshot = log["s2"]
    assert is_dag(snapshot["edges"])
    present = [k for k in range(1, 12) if f"s:{A}Service.run#{k}" in ids(snapshot)]
    chain = [f"e:{A}Service.run", *[f"s:{A}Service.run#{k}" for k in present]]
    assert all([a, b] in snapshot["edges"] for a, b in itertools.pairwise(chain))
    assert [f"e:{A}Port.send", f"e:{A}Impl.send"] in snapshot["edges"]
    assert [f"e:{A}Port.send", f"e:{A}Other.send"] in snapshot["edges"]


def test_the_steps_panel_follows_the_execution_focus(log: dict[str, Any]) -> None:
    """In the execution flow the panel describes the flow's function, and granularity is fixed."""
    assert log["s2"]["sub"].startswith("Service.run")
    assert log["s2"]["granDisabled"]


def test_recursive_calls_are_marked_and_the_flow_stays_acyclic(log: dict[str, Any]) -> None:
    """Exactly the two calls that close a cycle say so, and are not drawn as edges."""
    marks = [t for g in log["s3"]["groups"] for t in g["text"] if "recursive, not expanded" in t]
    assert len(marks) == RECURSIVE_STEPS
    assert is_dag(log["s3"]["edges"])


def test_mutual_recursion_collapses_to_one_node(log: dict[str, Any]) -> None:
    """`is_even` and `is_odd` become one "cycle of 2", and nothing is left pointing at itself."""
    snapshot = log["s4"]
    cycles = [g for g in snapshot["groups"] if g["id"].startswith("scc:")]
    assert len(cycles) == 1
    assert "cycle of 2" in cycles[0]["text"][0]
    assert R + "is_even" not in ids(snapshot)
    assert R + "is_odd" not in ids(snapshot)
    assert is_dag(snapshot["edges"])
    assert all(a != b for a, b in snapshot["edges"])
    assert "1 cycle collapsed" in snapshot["stats"]


def test_class_granularity_disables_step_numbers_without_hiding_the_control(log: dict[str, Any]) -> None:
    """No badges at class granularity; the checkbox stays, disabled."""
    assert not log["s6"]["badges"]
    assert log["s6"]["stepsDisabled"]


def test_the_execution_flow_without_a_focus_picks_the_busiest_entry_point(log: dict[str, Any]) -> None:
    """The function nobody calls that makes the most calls, and the page says why."""
    assert log["s5"]["execFid"].endswith("Service.run")
    assert "nobody calls it" in log["s5"]["hint"]


def test_private_names_are_hidden_by_default_and_dunders_stay(log: dict[str, Any]) -> None:
    """Private functions, classes and modules are hidden; public and dunder names stay; the count is stated."""
    shown = ids(log["s7"])
    assert not HIDDEN & shown
    assert {P + "public_fn", P + "Public.visible", U + "Tool.__init__"} <= shown
    assert "private functions hidden" in log["s7"]["stats"]


def test_the_steps_panel_skips_private_targets_and_explains_the_gap(log: dict[str, Any]) -> None:
    """Hidden targets are not listed; the panel says how many, and numbers keep their gaps."""
    assert log["s7b"]["stepNums"] == ["3"]
    assert "2 calls to private functions hidden" in log["s7b"]["note"]


def test_ticking_private_shows_them(log: dict[str, Any]) -> None:
    """With the checkbox ticked every private name appears and every step is listed."""
    assert ids(log["s8"]) >= HIDDEN
    assert log["s8b"]["stepNums"] == ["1", "2", "3"]


def test_the_focus_list_can_be_limited_to_busy_functions(log: dict[str, Any]) -> None:
    """At least 5 calls leaves one function; at least 3 leaves five; the focused one stays listed."""
    assert [o[0] for o in log["s9"]["opts"]] == ["", A + "Service.run"]
    assert "1 of" in log["s9"]["opts"][0][1]
    at_least_three = {o[0] for o in log["s9b"]["opts"]} - {""}
    assert at_least_three == {A + "Service.run", A + "main", R + "top", P + "public_fn", P + "Public.visible"}
    below_filter = [o[0] for o in log["s9c"]["opts"]]
    assert U + "Tool.work" in below_filter
    assert A + "Service.run" in below_filter


def test_the_execution_flow_lists_the_busiest_functions_first(log: dict[str, Any]) -> None:
    """Sorted by call count, with the count shown."""
    first = log["s10"]["opts"][1]
    assert first[0] == A + "Service.run"
    assert "11 calls" in first[1]


def test_the_acyclic_view_says_what_it_did(log: dict[str, Any]) -> None:
    """Either what it collapsed, or that it is the same as the call graph."""
    assert "Collapsed 1 cycle" in log["s11"]["hint"]
    assert "No cycles at this granularity" in log["s11b"]["hint"]

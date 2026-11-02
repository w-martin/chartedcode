//! Control-flow graphs on `tests/fixtures/flow/flow.py`.
//!
//! The expected shapes were worked out by hand from the fixture's source: each function's number of decision
//! points (complexity is that plus one), the labels on its branching edges, which edges lead into the block
//! holding each call, and which calls sit in code nothing can reach.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chartedcode_core::{analyze, Analysis, FlowGraph, Options};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn flow_analysis(include_source: bool) -> Analysis {
    analyze(
        &fixture("flow"),
        &Options {
            respect_gitignore: false,
            control_flow: true,
            include_source,
        },
    )
    .expect("fixture analyses")
}

fn graph<'a>(a: &'a Analysis, name: &str) -> &'a FlowGraph {
    let id = format!("flow::{name}");
    a.cfgs
        .iter()
        .find(|g| g.function == id)
        .unwrap_or_else(|| panic!("no flow graph for {id}"))
}

fn sorted(labels: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = labels.iter().map(ToString::to_string).collect();
    v.sort();
    v
}

/// The labels of the edges that lead into the block that holds the call `text`.
fn incoming(g: &FlowGraph, text: &str) -> Vec<String> {
    let block = g
        .blocks
        .iter()
        .find(|b| b.calls.iter().any(|c| c.text == text))
        .unwrap_or_else(|| panic!("{}: no block holds a call to {text}", g.function));
    let mut labels: Vec<String> = g
        .edges
        .iter()
        .filter(|e| e.to == block.id)
        .map(|e| e.label.clone())
        .collect();
    labels.sort();
    labels
}

fn is_unreachable(g: &FlowGraph, text: &str) -> bool {
    g.blocks
        .iter()
        .find(|b| b.calls.iter().any(|c| c.text == text))
        .is_some_and(|b| b.unreachable)
}

/// One function: complexity, non-empty edge labels, `(call, labels into its block)`, calls in unreachable code.
struct Shape {
    function: &'static str,
    complexity: usize,
    labels: &'static [&'static str],
    calls: &'static [(&'static str, &'static [&'static str])],
    unreachable: &'static [&'static str],
}

const SHAPES: &[Shape] = &[
    Shape {
        function: "straight",
        complexity: 1,
        labels: &[],
        calls: &[("a", &[""]), ("b", &[""])],
        unreachable: &[],
    },
    Shape {
        function: "branch",
        complexity: 2,
        labels: &["true", "false"],
        calls: &[("a", &["true"]), ("b", &["false"]), ("c", &["", ""])],
        unreachable: &[],
    },
    Shape {
        function: "elif_chain",
        complexity: 3,
        labels: &["true", "false", "true", "false"],
        calls: &[
            ("a", &["true"]),
            ("b", &["true"]),
            ("c", &["", "", "false"]),
        ],
        unreachable: &[],
    },
    Shape {
        function: "loop_for",
        complexity: 2,
        labels: &["next item", "no more items", "loop"],
        calls: &[("a", &["next item"]), ("b", &["no more items"])],
        unreachable: &[],
    },
    Shape {
        function: "loop_while_break",
        complexity: 3,
        labels: &["true", "false", "true", "false", "break", "loop"],
        calls: &[
            ("cond", &["", "loop"]),
            ("a", &["false"]),
            ("b", &["break", "false"]),
        ],
        unreachable: &[],
    },
    Shape {
        function: "infinite_loop",
        complexity: 1,
        labels: &["true", "loop"],
        calls: &[("a", &["true"]), ("b", &[])],
        unreachable: &["b"],
    },
    Shape {
        function: "try_except",
        complexity: 3,
        labels: &["exception", "exception"],
        calls: &[
            ("a", &[""]),
            ("b", &["exception"]),
            ("c", &["exception"]),
            ("d", &["", "", ""]),
        ],
        unreachable: &[],
    },
    Shape {
        function: "try_finally_return",
        complexity: 1,
        labels: &["return", "return, after finally"],
        calls: &[("a", &[""]), ("b", &["return"]), ("c", &[])],
        unreachable: &["c"],
    },
    Shape {
        function: "with_stmt",
        complexity: 1,
        labels: &[],
        calls: &[("open_x", &[""]), ("a", &[""])],
        unreachable: &[],
    },
    Shape {
        function: "match_stmt",
        complexity: 3,
        labels: &["case", "case", "case"],
        calls: &[
            ("a", &["case"]),
            ("b", &["case"]),
            ("c", &["case"]),
            ("d", &["", "", ""]),
        ],
        unreachable: &[],
    },
    Shape {
        function: "assert_stmt",
        complexity: 2,
        labels: &["assertion fails"],
        calls: &[("a", &[""])],
        unreachable: &[],
    },
    Shape {
        function: "raise_in_try",
        complexity: 2,
        labels: &["exception", "raise"],
        calls: &[("ValueError", &[""]), ("a", &["exception", "raise"])],
        unreachable: &[],
    },
    Shape {
        function: "nested_def_calls",
        complexity: 1,
        labels: &[],
        calls: &[("a", &[""]), ("inner", &[""])],
        unreachable: &[],
    },
    Shape {
        function: "continue_in_for",
        complexity: 3,
        labels: &[
            "next item",
            "no more items",
            "loop",
            "true",
            "false",
            "continue",
        ],
        calls: &[("a", &["false"])],
        unreachable: &[],
    },
    Shape {
        function: "after_return",
        complexity: 1,
        labels: &["return"],
        calls: &[("a", &[""]), ("b", &[])],
        unreachable: &["b"],
    },
    Shape {
        function: "for_else",
        complexity: 2,
        labels: &["next item", "no more items", "loop"],
        calls: &[
            ("a", &["next item"]),
            ("b", &["no more items"]),
            ("c", &[""]),
        ],
        unreachable: &[],
    },
];

#[test]
fn each_function_has_the_expected_shape() {
    let a = flow_analysis(true);
    for shape in SHAPES {
        let g = graph(&a, shape.function);
        assert_eq!(
            g.complexity, shape.complexity,
            "{}: complexity",
            shape.function
        );
        let mut labels: Vec<String> = g
            .edges
            .iter()
            .filter(|e| !e.label.is_empty())
            .map(|e| e.label.clone())
            .collect();
        labels.sort();
        assert_eq!(
            labels,
            sorted(shape.labels),
            "{}: edge labels",
            shape.function
        );
        for (call, into) in shape.calls {
            assert_eq!(
                incoming(g, call),
                sorted(into),
                "{}: edges into the block with {call}",
                shape.function
            );
        }
        for call in shape.unreachable {
            assert!(
                is_unreachable(g, call),
                "{}: {call} should be unreachable",
                shape.function
            );
        }
        let reachable_calls: Vec<&str> = shape
            .calls
            .iter()
            .map(|(c, _)| *c)
            .filter(|c| !shape.unreachable.contains(c))
            .collect();
        for call in reachable_calls {
            assert!(
                !is_unreachable(g, call),
                "{}: {call} should be reachable",
                shape.function
            );
        }
    }
}

#[test]
fn every_function_in_the_fixture_has_a_graph_with_sound_structure() {
    let a = flow_analysis(true);
    let functions = a.functions.iter().filter(|f| !f.synthetic).count();
    assert_eq!(a.cfgs.len(), functions, "a function lost its graph");
    for g in &a.cfgs {
        let n = g.blocks.len();
        assert!(
            g.blocks.iter().enumerate().all(|(i, b)| b.id == i),
            "{}: block ids",
            g.function
        );
        assert!(
            g.edges.iter().all(|e| e.from < n && e.to < n),
            "{}: edge endpoints",
            g.function
        );
        assert!(
            g.edges.iter().all(|e| e.to != g.entry),
            "{}: nothing enters the entry",
            g.function
        );
        assert!(
            g.edges.iter().all(|e| e.from != g.exit),
            "{}: nothing leaves the exit",
            g.function
        );
        assert!(g.blocks[g.entry].lines.is_empty() && g.blocks[g.exit].lines.is_empty());
    }
}

#[test]
fn call_numbers_in_blocks_are_the_call_graphs_step_numbers() {
    let a = flow_analysis(true);
    for g in &a.cfgs {
        let mut steps: Vec<usize> = vec![];
        for call in g.blocks.iter().flat_map(|b| &b.calls) {
            let Some(step) = call.step else {
                assert!(
                    call.targets.is_empty(),
                    "{}: {} has targets but no step",
                    g.function,
                    call.text
                );
                continue;
            };
            steps.push(step);
            for t in &call.targets {
                let edge = a
                    .calls
                    .iter()
                    .find(|e| {
                        e.from == g.function
                            && e.to == t.to
                            && e.kind == t.kind
                            && e.evidence == t.evidence
                    })
                    .unwrap_or_else(|| {
                        panic!("{}: no call edge for {} -> {}", g.function, call.text, t.to)
                    });
                assert!(
                    edge.steps.contains(&step),
                    "{}: step {step} of {} missing from the edge",
                    g.function,
                    call.text
                );
            }
        }
        steps.sort_unstable();
        let expected: Vec<usize> = (1..=steps.len()).collect();
        assert_eq!(
            steps, expected,
            "{}: steps are 1..n without gaps or repeats",
            g.function
        );
    }
}

#[test]
fn shown_text_matches_the_source_line_it_names() {
    let a = flow_analysis(true);
    let source = std::fs::read_to_string(fixture("flow").join("flow.py")).expect("fixture source");
    let lines: Vec<&str> = source.lines().collect();
    let mut checked = 0;
    for line in a.cfgs.iter().flat_map(|g| &g.blocks).flat_map(|b| &b.lines) {
        let shown = line.text.trim_end_matches('…').trim();
        let original = lines[line.line - 1].trim();
        assert!(
            original.contains(shown),
            "line {}: {shown:?} is not part of {original:?}",
            line.line
        );
        checked += 1;
    }
    assert!(checked > 40, "only {checked} lines checked");
}

#[test]
fn leaving_the_source_out_keeps_line_numbers() {
    let with = flow_analysis(true);
    let without = flow_analysis(false);
    let numbers = |a: &Analysis| -> Vec<usize> {
        a.cfgs
            .iter()
            .flat_map(|g| &g.blocks)
            .flat_map(|b| &b.lines)
            .map(|l| l.line)
            .collect()
    };
    assert_eq!(numbers(&with), numbers(&without));
    assert!(without
        .cfgs
        .iter()
        .flat_map(|g| &g.blocks)
        .flat_map(|b| &b.lines)
        .all(|l| l.text.is_empty()));
}

#[test]
fn graphs_are_not_built_unless_asked_for() {
    let a = analyze(
        &fixture("flow"),
        &Options {
            respect_gitignore: false,
            ..Options::default()
        },
    )
    .expect("analyses");
    assert!(a.cfgs.is_empty());
}

#[test]
fn block_kinds_name_what_they_are() {
    let a = flow_analysis(true);
    let kinds = |name: &str| -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for b in &graph(&a, name).blocks {
            *m.entry(b.kind.clone()).or_default() += 1;
        }
        m
    };
    assert_eq!(kinds("loop_for").get("loop"), Some(&1));
    assert_eq!(kinds("try_except").get("handler"), Some(&2));
    assert_eq!(kinds("try_finally_return").get("finally"), Some(&1));
    assert_eq!(kinds("straight").get("entry"), Some(&1));
    assert_eq!(kinds("straight").get("exit"), Some(&1));
}

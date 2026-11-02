//! Writing the self-contained HTML diagrams.
//!
//! Each page holds its data, the layout engine and the viewer code, so it works offline and needs no server.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::model::{Analysis, CallEdge, CallStats, ClassEdge, ClassNode, FlowGraph, FunctionNode};

/// elkjs 0.11.0, unmodified (EPL-2.0; see NOTICE). Its checksum is recorded beside it and tested.
const ELK: &str = include_str!("../assets/elk.bundled.js");
const CLASSES: &str = include_str!("../assets/classes.html");
const CALLS: &str = include_str!("../assets/calls.html");
const FLOW: &str = include_str!("../assets/flow.html");

/// Which diagram to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// Inheritance, Protocol conformance and field types.
    Classes,
    /// Calls between functions, with call order and an execution-flow view.
    Calls,
    /// The control-flow graph of each function.
    Flow,
}

impl View {
    /// The name used in file names and messages.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Classes => "classes",
            Self::Calls => "calls",
            Self::Flow => "flow",
        }
    }
}

/// How the page starts out.
#[derive(Clone, Debug, Default)]
pub struct RenderOptions {
    /// Start with private names (a single leading underscore) shown.
    pub show_private: bool,
    /// For the control-flow view, only include functions of at least this complexity.
    pub min_complexity: usize,
}

#[derive(Serialize)]
struct Config {
    #[serde(rename = "showPrivate")]
    show_private: bool,
}

#[derive(Serialize)]
struct ClassesData<'a> {
    root: &'a str,
    files: usize,
    nodes: &'a [ClassNode],
    edges: &'a [ClassEdge],
}

#[derive(Serialize)]
struct CallsData<'a> {
    root: &'a str,
    files: usize,
    functions: &'a [FunctionNode],
    calls: &'a [CallEdge],
    external_functions: &'a BTreeSet<String>,
    call_stats: &'a CallStats,
}

#[derive(Serialize)]
struct FlowData<'a> {
    root: &'a str,
    files: usize,
    functions: &'a [FunctionNode],
    cfgs: Vec<&'a FlowGraph>,
}

/// JSON that is safe inside a `<script>` element: `<` is written as `<`, so `</script>` and `<!--` cannot appear.
fn script_safe(json: &str) -> String {
    json.replace('<', "\\u003c")
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Fill the `@@NAME@@` slots of `template` in one pass: the values are never searched for slots themselves,
/// so source text that happens to contain `@@ELK@@` stays text.
fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut out =
        String::with_capacity(template.len() + values.iter().map(|(_, v)| v.len()).sum::<usize>());
    let mut rest = template;
    while let Some((at, slot, value)) = values
        .iter()
        .filter_map(|(slot, value)| rest.find(slot).map(|at| (at, *slot, *value)))
        .min_by_key(|(at, _, _)| *at)
    {
        out.push_str(&rest[..at]);
        out.push_str(value);
        rest = &rest[at + slot.len()..];
    }
    out.push_str(rest);
    out
}

/// The page for `view` over `analysis`.
///
/// # Errors
///
/// If the analysis cannot be written as JSON.
pub fn render_html(
    view: View,
    analysis: &Analysis,
    options: &RenderOptions,
) -> Result<String, serde_json::Error> {
    let root = analysis.root.as_str();
    let (template, data) = match view {
        View::Classes => (
            CLASSES,
            serde_json::to_string(&ClassesData {
                root,
                files: analysis.files,
                nodes: &analysis.nodes,
                edges: &analysis.edges,
            })?,
        ),
        View::Calls => (
            CALLS,
            serde_json::to_string(&CallsData {
                root,
                files: analysis.files,
                functions: &analysis.functions,
                calls: &analysis.calls,
                external_functions: &analysis.external_functions,
                call_stats: &analysis.call_stats,
            })?,
        ),
        View::Flow => (
            FLOW,
            serde_json::to_string(&FlowData {
                root,
                files: analysis.files,
                functions: &analysis.functions,
                cfgs: analysis
                    .cfgs
                    .iter()
                    .filter(|g| g.complexity >= options.min_complexity)
                    .collect(),
            })?,
        ),
    };
    let config = serde_json::to_string(&Config {
        show_private: options.show_private,
    })?;
    Ok(fill(
        template,
        &[
            ("@@TITLE@@", &escape_html(root)),
            ("@@CONFIG@@", &script_safe(&config)),
            ("@@DATA@@", &script_safe(&data)),
            ("@@ELK@@", ELK),
        ],
    ))
}

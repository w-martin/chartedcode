//! Analysing a project: from a path to the classes, functions and calls in it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::calls::SiteInfo;
use crate::model::Project;
use crate::model::{
    Analysis, AttrNode, ClassNode, FlowBlock, FlowCall, FlowEdge, FlowGraph, FlowLine,
    FunctionNode, SCHEMA_VERSION,
};
use crate::resolve::Resolver;
use crate::{calls, classes, discover, parse};

/// What to analyse and which files to leave out.
#[derive(Clone, Debug)]
pub struct Options {
    /// Skip files matched by `.gitignore` and `.ignore` files.
    pub respect_gitignore: bool,
    /// Also build control-flow graphs (they are large, so they are off unless asked for).
    pub control_flow: bool,
    /// Keep source text in the control-flow graphs; without it blocks show only line numbers.
    pub include_source: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            control_flow: false,
            include_source: true,
        }
    }
}

/// Why a project could not be analysed.
#[derive(Debug)]
pub enum Error {
    /// The path could not be read.
    Io(PathBuf, io::Error),
    /// The path holds no Python files (after ignore files were applied).
    NoPythonFiles(PathBuf),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "cannot read {}: {e}", path.display()),
            Self::NoPythonFiles(path) => {
                write!(f, "no Python files found under {}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}

/// Analyse the Python file or directory at `path`.
///
/// # Errors
///
/// [`Error::Io`] if `path` cannot be read; [`Error::NoPythonFiles`] if it holds nothing to analyse.
pub fn analyze(path: &Path, options: &Options) -> Result<Analysis, Error> {
    let sources = discover::python_files(path, options.respect_gitignore)
        .map_err(|e| Error::Io(path.to_path_buf(), e))?;
    if sources.files.is_empty() {
        return Err(Error::NoPythonFiles(path.to_path_buf()));
    }
    let project = parse::load_project(
        &sources.root_dir,
        &sources.files,
        options.control_flow.then_some(options.include_source),
    );
    let resolver = Resolver::new(&project.mods, &project.classes, &project.funcs);
    let class_graph = classes::build(&project, &resolver);
    let call_graph = calls::build(&project, &resolver, &class_graph);

    let mut nodes: Vec<ClassNode> = project
        .classes
        .iter()
        .enumerate()
        .map(|(i, c)| ClassNode {
            id: class_graph.ids[i].clone(),
            name: c.name.clone(),
            module: project.mods[c.module].dotted.clone(),
            file: project.mods[c.module].rel.clone(),
            external: false,
            methods: c.methods.clone(),
            attrs: c
                .attrs
                .iter()
                .map(|a| AttrNode {
                    name: a.name.clone(),
                    ty: a.ty.clone(),
                })
                .collect(),
        })
        .collect();
    nodes.extend(class_graph.externals.iter().map(|b| ClassNode {
        id: format!("ext:{b}"),
        name: b.clone(),
        module: String::new(),
        file: String::new(),
        external: true,
        methods: vec![],
        attrs: vec![],
    }));
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut evidence: BTreeMap<String, usize> = BTreeMap::new();
    for e in &class_graph.edges {
        *counts.entry(e.kind.clone()).or_default() += 1;
        *evidence
            .entry(format!("{}/{}", e.kind, e.evidence))
            .or_default() += 1;
    }
    let root = sources.root_dir.file_name().map_or_else(
        || "project".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let cfgs = flow_graphs(&project, &call_graph.functions, &call_graph.sites);
    Ok(Analysis {
        schema_version: SCHEMA_VERSION,
        root,
        files: project.files,
        parse_failures: project.parse_failures,
        counts,
        evidence,
        unresolved_refs: class_graph.unresolved,
        nodes,
        edges: class_graph.edges,
        functions: call_graph.functions,
        calls: call_graph.calls,
        external_functions: call_graph.external.into_iter().collect::<BTreeSet<_>>(),
        call_stats: call_graph.stats,
        cfgs,
    })
}

/// The control-flow graphs, each block's calls linked to the call graph's step numbers.
fn flow_graphs(
    project: &Project,
    nodes: &[FunctionNode],
    sites: &[Vec<SiteInfo>],
) -> Vec<FlowGraph> {
    project
        .funcs
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            let cfg = f.cfg.as_ref()?;
            let blocks = cfg
                .blocks
                .iter()
                .enumerate()
                .map(|(id, b)| FlowBlock {
                    id,
                    kind: b.kind.name().into(),
                    lines: b
                        .lines
                        .iter()
                        .map(|l| FlowLine {
                            line: l.line,
                            text: l.text.clone(),
                        })
                        .collect(),
                    calls: b
                        .calls
                        .iter()
                        .map(|&c| {
                            let site = &sites[i][c];
                            FlowCall {
                                text: site.text.clone(),
                                status: site.status.into(),
                                step: site.step,
                                targets: site.targets.clone(),
                            }
                        })
                        .collect(),
                    unreachable: b.unreachable,
                })
                .collect();
            Some(FlowGraph {
                function: nodes[i].id.clone(),
                complexity: cfg.complexity,
                entry: cfg.entry,
                exit: cfg.exit,
                blocks,
                edges: cfg
                    .edges
                    .iter()
                    .map(|e| FlowEdge {
                        from: e.from,
                        to: e.to,
                        label: e.label.clone(),
                    })
                    .collect(),
            })
        })
        .collect()
}

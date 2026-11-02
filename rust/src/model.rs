//! Plain data the analysis builds: modules, classes, functions and their import bindings.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub(crate) enum Binding {
    Mod(String),
    Sym(String, String),
}

pub(crate) struct Module {
    pub(crate) rel: String,
    pub(crate) scope: PathBuf,
    pub(crate) dotted: String,
    pub(crate) bindings: HashMap<String, Binding>,
    pub(crate) stars: Vec<String>,
}

pub(crate) struct Attr {
    pub(crate) name: String,
    pub(crate) ty: String,
    pub(crate) refs: Vec<String>,
}

pub(crate) struct Class {
    pub(crate) module: usize,
    pub(crate) name: String,
    pub(crate) bases: Vec<String>,
    pub(crate) methods: Vec<String>,
    pub(crate) attrs: Vec<Attr>,
    pub(crate) ctor_attrs: Vec<(String, String)>,
}

pub(crate) enum Callee {
    Name(String),
    Dotted(Vec<String>),
    Super(String),
    Other,
}

pub(crate) struct CallSite {
    pub(crate) callee: Callee,
    pub(crate) passed: Vec<Callee>,
}

/// What a basic block is for: only used to style the diagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockKind {
    Entry,
    Exit,
    Plain,
    Branch,
    Loop,
    Handler,
    Finally,
}

impl BlockKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Entry => "entry",
            Self::Exit => "exit",
            Self::Plain => "plain",
            Self::Branch => "branch",
            Self::Loop => "loop",
            Self::Handler => "handler",
            Self::Finally => "finally",
        }
    }
}

/// A source line shown in a block; `text` is empty when source text is left out.
pub(crate) struct CfgLine {
    pub(crate) line: usize,
    pub(crate) text: String,
}

pub(crate) struct CfgBlock {
    pub(crate) kind: BlockKind,
    pub(crate) lines: Vec<CfgLine>,
    /// Indices into the function's call sites, in evaluation order.
    pub(crate) calls: Vec<usize>,
    /// No path from the function's entry reaches this block.
    pub(crate) unreachable: bool,
}

pub(crate) struct CfgEdge {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) label: String,
}

/// The basic blocks of a function and the branches between them.
pub(crate) struct Cfg {
    pub(crate) blocks: Vec<CfgBlock>,
    pub(crate) edges: Vec<CfgEdge>,
    pub(crate) entry: usize,
    pub(crate) exit: usize,
    /// Decision points plus one: branches, loops, handlers, case arms and asserts.
    pub(crate) complexity: usize,
}

pub(crate) struct Func {
    pub(crate) module: usize,
    pub(crate) owner: Option<usize>,
    pub(crate) name: String,
    pub(crate) params: Vec<(String, Vec<String>)>,
    pub(crate) calls: Vec<CallSite>,
    pub(crate) locals: HashMap<String, Vec<String>>,
    pub(crate) cfg: Option<Cfg>,
}

/// Everything read from the source files, before names are resolved.
pub(crate) struct Project {
    pub(crate) mods: Vec<Module>,
    pub(crate) classes: Vec<Class>,
    pub(crate) funcs: Vec<Func>,
    pub(crate) parse_failures: Vec<String>,
    pub(crate) files: usize,
}

/// The version of the JSON layout below; it changes when a field is renamed, removed or reinterpreted.
pub const SCHEMA_VERSION: u32 = 1;

/// The result of analysing a project: classes and how they relate, functions and who calls whom.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Analysis {
    pub schema_version: u32,
    pub root: String,
    pub files: usize,
    pub parse_failures: Vec<String>,
    pub counts: BTreeMap<String, usize>,
    pub evidence: BTreeMap<String, usize>,
    pub unresolved_refs: Vec<UnresolvedRef>,
    pub nodes: Vec<ClassNode>,
    pub edges: Vec<ClassEdge>,
    pub functions: Vec<FunctionNode>,
    pub calls: Vec<CallEdge>,
    pub external_functions: BTreeSet<String>,
    pub call_stats: CallStats,
    #[serde(default)]
    pub cfgs: Vec<FlowGraph>,
}

/// A name that is imported from the project but is not a class.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UnresolvedRef {
    #[serde(rename = "in")]
    pub within: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

/// A field of a class, with its annotation as written.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AttrNode {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// A class in the project, or a base class outside it (`external`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ClassNode {
    pub id: String,
    pub name: String,
    pub module: String,
    pub file: String,
    pub external: bool,
    pub methods: Vec<String>,
    pub attrs: Vec<AttrNode>,
}

/// How two classes relate: `inherits`, `composes` (a field's type) or `implements` (a Protocol, by method names).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ClassEdge {
    pub kind: String,
    pub from: String,
    pub to: String,
    pub evidence: String,
    pub written: String,
}

/// A function or method, a module's top-level code (`<module>`), or the implicit constructor of a class (`synthetic`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FunctionNode {
    pub id: String,
    pub name: String,
    pub module: String,
    pub file: String,
    pub class: Option<String>,
    pub unresolved: usize,
    pub unresolved_distinct: usize,
    pub unresolved_examples: Vec<String>,
    pub synthetic: bool,
}

/// Calls from one function to another, merged by kind and evidence, with the order of each call in the caller.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CallEdge {
    pub kind: String,
    pub from: String,
    pub to: String,
    pub evidence: String,
    pub count: usize,
    pub steps: Vec<usize>,
    pub written: String,
}

/// Totals over all calls.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CallStats {
    pub functions: usize,
    pub call_sites_resolved: usize,
    pub unresolved: usize,
    pub ignored_builtin_or_complex: usize,
    pub evidence: BTreeMap<String, usize>,
}

/// Make `names` unique by numbering repeats (`name`, `name#2`, ...), keeping their order.
pub(crate) fn unique_ids(names: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    names
        .map(|base| {
            let n = seen.entry(base.clone()).or_default();
            *n += 1;
            if *n == 1 {
                base
            } else {
                format!("{base}#{n}")
            }
        })
        .collect()
}

/// The control-flow graph of one function (or of a module's top-level code).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FlowGraph {
    pub function: String,
    pub complexity: usize,
    pub entry: usize,
    pub exit: usize,
    pub blocks: Vec<FlowBlock>,
    pub edges: Vec<FlowEdge>,
}

/// A basic block: straight-line statements, and the calls made in them.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FlowBlock {
    pub id: usize,
    pub kind: String,
    pub lines: Vec<FlowLine>,
    pub calls: Vec<FlowCall>,
    pub unreachable: bool,
}

/// A source line; `text` is empty when the source was left out.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FlowLine {
    pub line: usize,
    pub text: String,
}

/// A call made in a block. `status` is `traced` (it reaches a function and has a `step`, the same number the
/// call graph shows), `untraced` (the target could not be determined) or `ignored` (a builtin or a call on an
/// expression).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FlowCall {
    pub text: String,
    pub status: String,
    pub step: Option<usize>,
    pub targets: Vec<FlowTarget>,
}

/// Where a traced call goes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FlowTarget {
    pub kind: String,
    pub to: String,
    pub evidence: String,
}

/// A possible transfer of control; `label` says when (`true`, `false`, `loop`, `break`, ...), empty for plain flow.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FlowEdge {
    pub from: usize,
    pub to: usize,
    pub label: String,
}

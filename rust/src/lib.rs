//! chartedcode: diagrams for Python projects, drawn as self-contained HTML.

mod analysis;
mod calls;
mod cfg;
mod classes;
pub mod cli;
mod discover;
mod model;
mod modules;
mod parse;
mod render;
mod resolve;

pub use analysis::{analyze, Error, Options};
pub use model::{
    Analysis, AttrNode, CallEdge, CallStats, ClassEdge, ClassNode, FlowBlock, FlowCall, FlowEdge,
    FlowGraph, FlowLine, FlowTarget, FunctionNode, UnresolvedRef, SCHEMA_VERSION,
};
pub use render::{render_html, RenderOptions, View};

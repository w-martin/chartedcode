# chartedcode — requirements (PoC)

Deterministic, single-binary Rust tool that extracts class, call and control-flow structure from Python source and renders self-contained diagrams. Purpose: documentation, explainability, auditability, accuracy.

## Problem

- Existing extractors emit DOT-era output or Mermaid at one level of detail. pyreverse needs Graphviz installed to produce PNG/SVG, so results vary by machine.
- Existing modern renderers (D2, LikeC4, Structurizr) need a hand-written model; they do not extract from code.
- Call graphs for dynamic languages are inherently approximate (code2flow documents this). Tools that hide the uncertainty mislead readers.

## Goals (PoC)

1. Input: a local path to a Python project.
2. Views: class inheritance/composition, call graph, per-function control-flow graph.
3. Output: a self-contained file; no Graphviz or other external renderer.
4. Deterministic: same input gives byte-identical output.
5. Accurate: every edge records how it was derived.

## Design

- **Parse**: Rust Python parser (candidates: `ruff_python_parser`, tree-sitter-python; not yet evaluated).
- **Graph IR**: typed, versioned, language-neutral. Nodes at module / class / function / basic-block level. Edges carry a kind and an evidence label: `resolved`, `inferred`, `declared`. Unresolved calls are emitted as unresolved, never dropped. The schema must allow edges from an external source (SCIP) later.
- **Views**: projections of the IR.
- **Layout + render**: layered layout and edge routing producing SVG/HTML with no runtime dependencies. This is the main risk. Fallback is embedding a vendored JS layout engine.

## Acceptance criteria

- Zero installs beyond the binary.
- Two runs on the same input produce identical bytes.
- On a real mid-size Python repo the class and call views are legible without manual tweaking.
- Each call edge states its resolution evidence; unresolved calls are visible.

## Additional requirements from the spike

- Respect `.gitignore` when discovering files.
- Class view includes attributes/fields and composition (attribute types), and Protocol conformance, not only inheritance.
- Resolve bases and field types through imports (spike covers module imports, relative imports, TYPE_CHECKING, star imports and re-exports). Function-local imports, `__all__` and namespace packages are open. Name-only matching is dropped; unresolved references stay visible as external/unresolved.
- Each evidence value has a distinct visual encoding; relationship kind is encoded separately (line style).
- Protocol conformance and composition are scoped by package/imports so duplicate class names in different packages do not cross-match.
- Conformance compares method names and signatures; name-only matches stay labelled as weak evidence.
- Large graphs: collapse and filter by module/package; a 100+ class diagram must be usable.

- Call graph: relationship kinds `calls`, `constructs`, `passes` (callback references) and `dispatches` (Protocol to implementers), each with an evidence label; unresolvable calls are counted and shown per function, never dropped silently.
- Call graph viewer: function / class / module granularity, focus on a node with depth and direction, and a default granularity that keeps the first view readable.
- Call order: each call site is numbered per caller in evaluation order; numbers are shown on arrows, in a steps panel, and in an execution-flow DAG that merges shared callees and marks recursion. Branches and loops are flattened (order written, not guaranteed run order).
- Derived acyclic view: cycles of mutually calling nodes collapse into one node at any granularity.
- Interaction: two-step click-to-focus with a persistent status line; no controls appear or disappear (disabled instead).
- Untraced calls are explained in plain language, counted per function, and never drawn as edges.
- Private names (single leading underscore on a function, class or module segment; dunders excepted) are hidden by default with a visible toggle and a stated hidden count.
- The focus list can be limited to functions with at least N traced calls, always keeping the focused function listed; the execution flow lists the busiest first.
- Derived views state plainly when they are identical to the source view.
- Return-type inference and dispatch through subclass overrides are open.

## Later phases (out of PoC)

- Intra-program def-use data flow, built on the CFG.
- Optional SCIP index ingestion for precise resolution.
- Git-native input (repo URL or ref without checkout).
- More languages; cross-repo and cross-language links (HTTP routes/OpenAPI, protobuf/gRPC, message topics, package manifests); org-scale enumeration; collapsible output for large graphs.

## Non-goals (PoC)

- Perfect call resolution for dynamic Python.
- Editing diagrams or hand-authored models.
- Any language other than Python.

## Prior art

| Tool | Relevance |
|---|---|
| [pyreverse](https://pylint.pycqa.org/en/stable/additional_tools/pyreverse/index.html) | Python class/package UML; DOT, PlantUML, Mermaid; Graphviz for images |
| [code2flow](https://pypi.org/project/code2flow) | Heuristic call graphs; documented dynamic-language limits |
| [ast-to-mermaid](https://docs.rs/ast-to-mermaid), [infigraph-core](https://docs.rs/infigraph-core/latest/infigraph_core/sequence/index.html), [codetwin](https://docs.rs/crate/codetwin/0.1.9/source/README.md) | Rust tree-sitter tools emitting Mermaid; maturity not checked |
| [LikeC4](https://github.com/likec4/likec4), [Structurizr](https://docs.structurizr.com/ai), [D2](https://d2lang.com/tour/layouts/) | Architecture-as-code renderers; no extraction |
| [SCIP](https://sourcegraph.com/blog/announcing-scip) | Code index format; Go and Rust bindings; indexers for many languages |
| [stack-graphs](https://github.com/github/stack-graphs/releases/tag/tree-sitter-stack-graphs-v0.7.1) | Archived 2025-09-09; do not build on it |

Not verified: py2puml, pydeps, LikeC4 maturity.

Layout crates found on crates.io: `rust-sugiyama`, `dagre` (Rust port of dagre.js), `layout-rs`, and `merman-elk-layered` (EPL-2.0 port of Eclipse ELK, used via the `merman` crate). See `spike/FINDINGS.md`.

## Open questions

- Parser: spike favours the ruff crates (git-tag pinned, as in typedframes). The IR stays parser-agnostic.
- Layout: spike favours ELK. Open: elkjs embedded in the HTML vs a Rust-native engine; elkjs in a real browser, merman `layout-elk`, compound graphs and left-to-right layouts are untested.
- Test corpus: a few local repos plus one large public Python project.

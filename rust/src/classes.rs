//! Relationships between classes: inheritance, field types and Protocol conformance.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::model::{unique_ids, Binding, ClassEdge, Project, UnresolvedRef};
use crate::resolve::{Res, Resolver};

/// The classes' ids and relationships, and what the call graph needs to know about them.
pub(crate) struct ClassGraph {
    pub(crate) ids: Vec<String>,
    pub(crate) edges: Vec<ClassEdge>,
    /// For each class, the project classes its bases resolve to.
    pub(crate) resolved_bases: Vec<Vec<usize>>,
    /// For each Protocol, the classes whose method names satisfy it.
    pub(crate) implementers: BTreeMap<usize, Vec<usize>>,
    pub(crate) externals: BTreeSet<String>,
    pub(crate) unresolved: Vec<UnresolvedRef>,
}

fn edge(kind: &str, from: &str, to: &str, evidence: &str, written: String) -> ClassEdge {
    ClassEdge {
        kind: kind.into(),
        from: from.into(),
        to: to.into(),
        evidence: evidence.into(),
        written,
    }
}

pub(crate) fn build(project: &Project, r: &Resolver) -> ClassGraph {
    let Project { mods, classes, .. } = project;
    let ids = unique_ids(
        classes
            .iter()
            .map(|c| format!("{}::{}", mods[c.module].rel, c.name)),
    );
    let mut graph = ClassGraph {
        edges: vec![],
        resolved_bases: vec![vec![]; classes.len()],
        implementers: BTreeMap::new(),
        externals: BTreeSet::new(),
        unresolved: vec![],
        ids,
    };
    let mut protocols: BTreeSet<usize> = BTreeSet::new();
    for (i, c) in classes.iter().enumerate() {
        for b in &c.bases {
            let (to, ev) = match r.resolve(c.module, b, Some(i)) {
                Res::Class(j, ev) => {
                    graph.resolved_bases[i].push(j);
                    (graph.ids[j].clone(), ev)
                }
                res => {
                    if b.rsplit('.').next() == Some("Protocol") {
                        protocols.insert(i);
                    }
                    graph.externals.insert(b.clone());
                    let ev = if matches!(res, Res::Unresolved) {
                        graph.unresolved.push(UnresolvedRef {
                            within: graph.ids[i].clone(),
                            reference: b.clone(),
                        });
                        "unresolved"
                    } else {
                        "external"
                    };
                    (format!("ext:{b}"), ev)
                }
            };
            graph
                .edges
                .push(edge("inherits", &graph.ids[i], &to, ev, b.clone()));
        }
    }

    // Composition: field type names that resolve to a project class.
    for (i, c) in classes.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for a in &c.attrs {
            for t in &a.refs {
                match r.resolve(c.module, t, None) {
                    Res::Class(j, ev) if seen.insert((a.name.clone(), j)) => {
                        let written = format!("{}: {}", a.name, a.ty);
                        graph.edges.push(edge(
                            "composes",
                            &graph.ids[i],
                            &graph.ids[j],
                            ev,
                            written,
                        ));
                    }
                    Res::Unresolved => graph.unresolved.push(UnresolvedRef {
                        within: graph.ids[i].clone(),
                        reference: t.clone(),
                    }),
                    _ => {}
                }
            }
        }
    }

    implements(project, r, &protocols, &mut graph);
    graph
}

/// Protocol conformance by method names; signatures are not compared.
fn implements(
    project: &Project,
    r: &Resolver,
    protocols: &BTreeSet<usize>,
    graph: &mut ClassGraph,
) {
    let Project { mods, classes, .. } = project;
    let all_methods = |start: usize| -> BTreeSet<&str> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![start];
        let mut out = BTreeSet::new();
        while let Some(k) = stack.pop() {
            if seen.insert(k) {
                out.extend(classes[k].methods.iter().map(String::as_str));
                stack.extend(&graph.resolved_bases[k]);
            }
        }
        out
    };
    let ancestors = |start: usize| -> BTreeSet<usize> {
        let mut out = BTreeSet::new();
        let mut stack = graph.resolved_bases[start].clone();
        while let Some(k) = stack.pop() {
            if out.insert(k) {
                stack.extend(&graph.resolved_bases[k]);
            }
        }
        out
    };
    // The source roots a module imports from: a class may satisfy a Protocol from another root (test fakes).
    let imported_scopes: Vec<BTreeSet<PathBuf>> = mods
        .iter()
        .map(|m| {
            let targets = m
                .bindings
                .values()
                .map(|b| match b {
                    Binding::Mod(x) | Binding::Sym(x, _) => x.as_str(),
                })
                .chain(m.stars.iter().map(String::as_str));
            targets
                .filter_map(|t| r.find_module(&m.scope, t))
                .map(|(i, _)| mods[i].scope.clone())
                .collect()
        })
        .collect();
    let mut found: Vec<ClassEdge> = vec![];
    let mut implementers: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &p in protocols {
        let need: BTreeSet<&str> = classes[p]
            .methods
            .iter()
            .map(String::as_str)
            .filter(|m| !m.starts_with("__"))
            .collect();
        if need.is_empty() {
            continue;
        }
        let protocol_scope = &mods[classes[p].module].scope;
        for (i, c) in classes.iter().enumerate() {
            let visible = mods[c.module].scope == *protocol_scope
                || imported_scopes[c.module].contains(protocol_scope);
            if i == p || protocols.contains(&i) || !visible || ancestors(i).contains(&p) {
                continue;
            }
            if need.is_subset(&all_methods(i)) {
                let written = need.iter().copied().collect::<Vec<_>>().join(", ");
                found.push(edge(
                    "implements",
                    &graph.ids[i],
                    &graph.ids[p],
                    "method-names",
                    written,
                ));
                implementers.entry(p).or_default().push(i);
            }
        }
    }
    graph.edges.extend(found);
    graph.implementers = implementers;
}

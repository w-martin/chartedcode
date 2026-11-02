//! Call resolution: which function or method each call site reaches, and how sure we are.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::classes::ClassGraph;
use crate::model::{
    unique_ids, CallEdge, CallStats, Callee, Class, FlowTarget, Func, FunctionNode, Project,
};
use crate::resolve::{FoundSym, Res, Resolver, Sym, SymRes};

pub(crate) enum Target {
    Func(usize),
    Ctor(usize),
    Ext(String),
}

pub(crate) type Edge = (Target, &'static str, &'static str);

pub(crate) enum Outcome {
    Edges(Vec<Edge>),
    Unresolved,
    Ignored,
}

pub(crate) const WRAPPERS: &[&str] = &[
    "Optional",
    "Union",
    "Final",
    "ClassVar",
    "Annotated",
    "None",
    "Required",
    "NotRequired",
];

pub(crate) const CONTAINERS: &[&str] = &[
    "list",
    "List",
    "dict",
    "Dict",
    "set",
    "Set",
    "tuple",
    "Tuple",
    "frozenset",
    "Sequence",
    "Iterable",
    "Iterator",
    "Mapping",
    "MutableMapping",
    "Collection",
    "deque",
    "defaultdict",
    "Callable",
    "Generator",
    "AsyncIterator",
    "Awaitable",
    "Coroutine",
    "type",
];

pub(crate) const BUILTIN_RECEIVERS: &[&str] = &[
    "str",
    "int",
    "float",
    "bool",
    "list",
    "dict",
    "set",
    "tuple",
    "bytes",
    "bytearray",
    "frozenset",
    "object",
    "type",
];

pub(crate) struct CallCtx<'a, 'b> {
    pub(crate) r: &'a Resolver<'b>,
    pub(crate) funcs: &'a [Func],
    pub(crate) classes: &'a [Class],
    pub(crate) bases: &'a [Vec<usize>],
    pub(crate) methods: &'a HashMap<(usize, String), usize>,
}

impl CallCtx<'_, '_> {
    pub(crate) fn find_method(
        &self,
        start: usize,
        name: &str,
        skip_own: bool,
    ) -> Option<(usize, bool)> {
        fn go(
            cx: &CallCtx,
            c: usize,
            name: &str,
            first: bool,
            skip_own: bool,
            seen: &mut BTreeSet<usize>,
        ) -> Option<(usize, bool)> {
            if !seen.insert(c) {
                return None;
            }
            if !(first && skip_own) {
                if let Some(&f) = cx.methods.get(&(c, name.to_string())) {
                    return Some((f, !first));
                }
            }
            for &b in &cx.bases[c] {
                if let Some(x) = go(cx, b, name, false, skip_own, seen) {
                    return Some(x);
                }
            }
            None
        }
        go(self, start, name, true, skip_own, &mut BTreeSet::new())
    }

    pub(crate) fn find_attr(&self, start: usize, name: &str) -> Option<(Vec<String>, usize)> {
        fn go(
            cx: &CallCtx,
            c: usize,
            name: &str,
            seen: &mut BTreeSet<usize>,
        ) -> Option<(Vec<String>, usize)> {
            if !seen.insert(c) {
                return None;
            }
            let cl = &cx.classes[c];
            if let Some(a) = cl.attrs.iter().find(|a| a.name == name) {
                return Some((a.refs.clone(), cl.module));
            }
            if let Some((_, r)) = cl.ctor_attrs.iter().find(|(n, _)| n == name) {
                return Some((vec![r.clone()], cl.module));
            }
            for &b in &cx.bases[c] {
                if let Some(x) = go(cx, b, name, seen) {
                    return Some(x);
                }
            }
            None
        }
        go(self, start, name, &mut BTreeSet::new())
    }

    // Classes a typed receiver can be; containers and unions of wrappers are not treated as the element class.
    pub(crate) fn receiver_classes(&self, module: usize, refs: &[String]) -> Vec<usize> {
        let last = |s: &str| s.rsplit('.').next().unwrap_or("").to_string();
        if refs.iter().any(|x| CONTAINERS.contains(&last(x).as_str())) {
            return vec![];
        }
        let mut out = vec![];
        for x in refs {
            if WRAPPERS.contains(&last(x).as_str()) {
                continue;
            }
            if let Res::Class(c, _) = self.r.resolve(module, x, None) {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
        }
        out
    }

    pub(crate) fn sym_evidence(f: &FoundSym) -> &'static str {
        if f.local {
            "same-module"
        } else if f.other_root {
            "other-root"
        } else if f.reexport {
            "re-export"
        } else {
            "import"
        }
    }

    pub(crate) fn ctor(&self, class: usize, ev: &'static str) -> Edge {
        match self.find_method(class, "__init__", false) {
            Some((f, _)) => (Target::Func(f), "constructs", ev),
            None => (Target::Ctor(class), "constructs", ev),
        }
    }

    pub(crate) fn via_symbol(&self, module: usize, parts: &[&str]) -> Outcome {
        let n = parts.len();
        match self.r.resolve_symbol_path(module, parts) {
            SymRes::Sym(f, consumed) => {
                let ev = Self::sym_evidence(&f);
                if consumed == n {
                    return Outcome::Edges(vec![match f.sym {
                        Sym::Func(t) => (Target::Func(t), "calls", ev),
                        Sym::Class(c) => self.ctor(c, ev),
                    }]);
                }
                if let (true, Sym::Class(c)) = (consumed + 1 == n, f.sym) {
                    if let Some((t, _)) = self.find_method(c, parts[n - 1], false) {
                        return Outcome::Edges(vec![(Target::Func(t), "calls", ev)]);
                    }
                }
                Outcome::Unresolved
            }
            SymRes::External => {
                Outcome::Edges(vec![(Target::Ext(parts.join(".")), "calls", "external")])
            }
            SymRes::Unresolved => Outcome::Unresolved,
            SymRes::Unbound => {
                if n == 1 || BUILTIN_RECEIVERS.contains(&parts[0]) {
                    Outcome::Ignored
                } else {
                    Outcome::Unresolved
                }
            }
        }
    }

    pub(crate) fn typed_method_call(
        &self,
        module: usize,
        classes: Vec<usize>,
        method: &str,
        ev: &'static str,
    ) -> Outcome {
        let _ = module;
        let edges: Vec<Edge> = classes
            .into_iter()
            .filter_map(|c| {
                self.find_method(c, method, false)
                    .map(|(t, _)| (Target::Func(t), "calls", ev))
            })
            .collect();
        if edges.is_empty() {
            Outcome::Unresolved
        } else {
            Outcome::Edges(edges)
        }
    }

    pub(crate) fn resolve_call(&self, fi: usize, callee: &Callee) -> Outcome {
        let f = &self.funcs[fi];
        match callee {
            Callee::Other => Outcome::Ignored,
            Callee::Super(m) => match f.owner.and_then(|c| self.find_method(c, m, true)) {
                Some((t, _)) => Outcome::Edges(vec![(Target::Func(t), "calls", "super")]),
                None => Outcome::Unresolved,
            },
            Callee::Name(n) => {
                if f.params.iter().any(|p| p.0 == *n) || f.locals.contains_key(n) {
                    return Outcome::Unresolved;
                }
                self.via_symbol(f.module, &[n.as_str()])
            }
            Callee::Dotted(parts) => {
                let head = parts[0].as_str();
                if head == "self" || head == "cls" {
                    let Some(c) = f.owner else {
                        return Outcome::Unresolved;
                    };
                    return match parts.len() {
                        2 => match self.find_method(c, &parts[1], false) {
                            Some((t, inherited)) => Outcome::Edges(vec![(
                                Target::Func(t),
                                "calls",
                                if inherited { "inherited" } else { "self" },
                            )]),
                            None => Outcome::Unresolved,
                        },
                        3 => match self.find_attr(c, &parts[1]) {
                            Some((refs, module)) => {
                                let cls = self.receiver_classes(module, &refs);
                                self.typed_method_call(module, cls, &parts[2], "field-type")
                            }
                            None => Outcome::Unresolved,
                        },
                        _ => Outcome::Unresolved,
                    };
                }
                let typed = f
                    .params
                    .iter()
                    .find(|p| p.0 == head)
                    .map(|p| (p.1.clone(), "param-type"))
                    .or_else(|| f.locals.get(head).map(|l| (l.clone(), "local-type")));
                if let Some((refs, ev)) = typed {
                    if parts.len() != 2 {
                        return Outcome::Unresolved;
                    }
                    let cls = self.receiver_classes(f.module, &refs);
                    return self.typed_method_call(f.module, cls, &parts[1], ev);
                }
                let ps: Vec<&str> = parts.iter().map(String::as_str).collect();
                self.via_symbol(f.module, &ps)
            }
        }
    }
}

// Distinct callee texts in first-seen order, with a count when one repeats.
pub(crate) fn distinct_calls(calls: &[String]) -> Vec<String> {
    let mut order: Vec<&String> = vec![];
    let mut counts: HashMap<&String, usize> = HashMap::new();
    for c in calls {
        if !counts.contains_key(c) {
            order.push(c);
        }
        *counts.entry(c).or_default() += 1;
    }
    order
        .into_iter()
        .map(|c| match counts[c] {
            1 => c.clone(),
            n => format!("{c} ×{n}"),
        })
        .collect()
}

/// Every function, the calls between them, and totals.
/// What became of one call site, for the control-flow view.
#[derive(Clone)]
pub(crate) struct SiteInfo {
    pub(crate) text: String,
    pub(crate) status: &'static str,
    pub(crate) step: Option<usize>,
    pub(crate) targets: Vec<FlowTarget>,
}

pub(crate) struct CallGraph {
    pub(crate) functions: Vec<FunctionNode>,
    pub(crate) calls: Vec<CallEdge>,
    pub(crate) external: BTreeSet<String>,
    pub(crate) stats: CallStats,
    /// For each function, one entry per call site, in the order of `Func::calls`.
    pub(crate) sites: Vec<Vec<SiteInfo>>,
}

/// Methods by class and name; the first definition wins.
pub(crate) fn method_index(funcs: &[Func]) -> HashMap<(usize, String), usize> {
    let mut index = HashMap::new();
    for (i, f) in funcs.iter().enumerate() {
        if let Some(c) = f.owner {
            index.entry((c, f.name.clone())).or_insert(i);
        }
    }
    index
}

fn callee_text(c: &Callee) -> String {
    match c {
        Callee::Name(n) => n.clone(),
        Callee::Dotted(p) => p.join("."),
        Callee::Super(m) => format!("super().{m}"),
        Callee::Other => "<expr>".into(),
    }
}

type CallKey = (String, String, &'static str, &'static str);
type CallTotals = (usize, BTreeSet<String>, BTreeSet<usize>);

/// Resolve every call site, number them in evaluation order, and add Protocol dispatch.
pub(crate) fn build(project: &Project, r: &Resolver, class_graph: &ClassGraph) -> CallGraph {
    let Project {
        mods,
        classes,
        funcs,
        ..
    } = project;
    let methods = method_index(funcs);
    let cx = CallCtx {
        r,
        funcs,
        classes,
        bases: &class_graph.resolved_bases,
        methods: &methods,
    };
    let fids = unique_ids(funcs.iter().map(|f| {
        let qualified = match f.owner {
            Some(c) => format!("{}.{}", classes[c].name, f.name),
            None => f.name.clone(),
        };
        format!("{}::{}", mods[f.module].rel, qualified)
    }));
    let mut unresolved_calls: Vec<Vec<String>> = vec![vec![]; funcs.len()];
    let mut ignored_calls = 0usize;
    let mut agg: BTreeMap<CallKey, CallTotals> = BTreeMap::new();
    let mut synthetic: BTreeMap<usize, String> = BTreeMap::new();
    let mut external: BTreeSet<String> = BTreeSet::new();
    let mut sites: Vec<Vec<SiteInfo>> = vec![vec![]; funcs.len()];
    for (fi, f) in funcs.iter().enumerate() {
        let mut step = 0usize;
        for site in &f.calls {
            let mut hits: Vec<(Edge, String)> = vec![];
            let mut status = "ignored";
            match cx.resolve_call(fi, &site.callee) {
                Outcome::Ignored => ignored_calls += 1,
                Outcome::Unresolved => {
                    status = "untraced";
                    unresolved_calls[fi].push(callee_text(&site.callee));
                }
                Outcome::Edges(es) => {
                    hits.extend(es.into_iter().map(|e| (e, callee_text(&site.callee))))
                }
            }
            // A function or method reference passed as an argument is assumed to be invoked by the callee.
            for arg in &site.passed {
                if let Outcome::Edges(es) = cx.resolve_call(fi, arg) {
                    for (t, kind, ev) in es {
                        if let (Target::Func(_), "calls") = (&t, kind) {
                            hits.push(((t, "passes", ev), callee_text(arg)));
                        }
                    }
                }
            }
            if hits.is_empty() {
                sites[fi].push(SiteInfo {
                    text: callee_text(&site.callee),
                    status,
                    step: None,
                    targets: vec![],
                });
                continue;
            }
            step += 1;
            let mut targets = vec![];
            for ((t, kind, ev), text) in hits {
                let to = match t {
                    Target::Func(j) => fids[j].clone(),
                    Target::Ctor(c) => synthetic
                        .entry(c)
                        .or_insert_with(|| {
                            format!(
                                "{}::{}.__init__",
                                mods[classes[c].module].rel, classes[c].name
                            )
                        })
                        .clone(),
                    Target::Ext(name) => {
                        external.insert(name.clone());
                        format!("ext:{name}")
                    }
                };
                targets.push(FlowTarget {
                    kind: kind.into(),
                    to: to.clone(),
                    evidence: ev.into(),
                });
                let totals = agg.entry((fids[fi].clone(), to, kind, ev)).or_default();
                totals.0 += 1;
                if totals.1.len() < 3 {
                    totals.1.insert(text);
                }
                totals.2.insert(step);
            }
            sites[fi].push(SiteInfo {
                text: callee_text(&site.callee),
                status: "traced",
                step: Some(step),
                targets,
            });
        }
    }
    // A call to a Protocol method may run any implementer's method of that name.
    let called: BTreeSet<String> = agg
        .keys()
        .filter(|k| k.2 != "dispatches")
        .map(|k| k.1.clone())
        .collect();
    for (&p, implementers) in &class_graph.implementers {
        for (fi, f) in funcs.iter().enumerate() {
            if f.owner != Some(p) || !called.contains(&fids[fi]) {
                continue;
            }
            for &i in implementers {
                if let Some((t, _)) = cx.find_method(i, &f.name, false) {
                    let key = (
                        fids[fi].clone(),
                        fids[t].clone(),
                        "dispatches",
                        "method-names",
                    );
                    let totals = agg.entry(key).or_default();
                    totals.0 += 1;
                    totals.1.insert(f.name.clone());
                }
            }
        }
    }

    let mut functions: Vec<FunctionNode> = funcs
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let distinct = distinct_calls(&unresolved_calls[i]);
            FunctionNode {
                id: fids[i].clone(),
                name: match f.owner {
                    Some(c) => format!("{}.{}", classes[c].name, f.name),
                    None => f.name.clone(),
                },
                module: mods[f.module].dotted.clone(),
                file: mods[f.module].rel.clone(),
                class: f.owner.map(|c| class_graph.ids[c].clone()),
                unresolved: unresolved_calls[i].len(),
                unresolved_distinct: distinct.len(),
                unresolved_examples: distinct.into_iter().take(5).collect(),
                synthetic: false,
            }
        })
        .collect();
    for (c, id) in &synthetic {
        functions.push(FunctionNode {
            id: id.clone(),
            name: format!("{}.__init__", classes[*c].name),
            module: mods[classes[*c].module].dotted.clone(),
            file: mods[classes[*c].module].rel.clone(),
            class: Some(class_graph.ids[*c].clone()),
            unresolved: 0,
            unresolved_distinct: 0,
            unresolved_examples: vec![],
            synthetic: true,
        });
    }
    let mut evidence: BTreeMap<String, usize> = BTreeMap::new();
    for ((_, _, kind, ev), (n, _, _)) in &agg {
        *evidence.entry(format!("{kind}/{ev}")).or_default() += n;
    }
    let stats = CallStats {
        functions: funcs.len(),
        call_sites_resolved: agg.values().map(|v| v.0).sum(),
        unresolved: unresolved_calls.iter().map(Vec::len).sum(),
        ignored_builtin_or_complex: ignored_calls,
        evidence,
    };
    let calls = agg
        .into_iter()
        .map(|((from, to, kind, ev), (count, written, steps))| CallEdge {
            kind: kind.into(),
            from,
            to,
            evidence: ev.into(),
            count,
            steps: steps.into_iter().take(40).collect(),
            written: written.into_iter().collect::<Vec<_>>().join(", "),
        })
        .collect();
    CallGraph {
        functions,
        calls,
        external,
        stats,
        sites,
    }
}

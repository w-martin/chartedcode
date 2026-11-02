//! Reading Python source: classes, functions, imports and call sites, with the ruff parser.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use ruff_python_ast::visitor::{walk_expr, walk_stmt, Visitor};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use ruff_source_file::LineIndex;

use crate::cfg::{self, Flow};
use crate::model::{Attr, Binding, CallSite, Callee, Class, Func, Module, Project};
use crate::modules::{module_of, relative_base};

pub(crate) fn chain(e: &Expr) -> Option<Vec<String>> {
    match e {
        Expr::Name(n) => Some(vec![n.id.as_str().to_string()]),
        Expr::Attribute(a) => {
            let mut v = chain(&a.value)?;
            v.push(a.attr.as_str().to_string());
            Some(v)
        }
        _ => None,
    }
}

pub(crate) fn callee_of(e: &Expr) -> Callee {
    match e {
        Expr::Name(n) => Callee::Name(n.id.as_str().to_string()),
        Expr::Attribute(a) => {
            if let Expr::Call(c) = &*a.value {
                if matches!(&*c.func, Expr::Name(n) if n.id.as_str() == "super") {
                    return Callee::Super(a.attr.as_str().to_string());
                }
            }
            match chain(e) {
                Some(v) => Callee::Dotted(v),
                None => Callee::Other,
            }
        }
        _ => Callee::Other,
    }
}

pub(crate) struct Collector {
    pub(crate) calls: Vec<CallSite>,
    pub(crate) locals: HashMap<String, Vec<String>>,
    pub(crate) skip_defs: bool,
}

impl<'a> Visitor<'a> for Collector {
    fn visit_stmt(&mut self, s: &'a Stmt) {
        if self.skip_defs && matches!(s, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        match s {
            Stmt::Assign(a) if a.targets.len() == 1 => {
                if let (Expr::Name(t), Expr::Call(c)) = (&a.targets[0], &*a.value) {
                    if let Some(ch) = chain(&c.func) {
                        self.locals
                            .entry(t.id.as_str().to_string())
                            .or_default()
                            .push(ch.join("."));
                    }
                }
            }
            Stmt::AnnAssign(a) => {
                if let Expr::Name(t) = &*a.target {
                    let mut rs = vec![];
                    refs(&a.annotation, &mut rs);
                    self.locals
                        .entry(t.id.as_str().to_string())
                        .or_default()
                        .extend(rs);
                }
            }
            _ => {}
        }
        walk_stmt(self, s);
    }

    // Children first: arguments and the callee expression are evaluated before the call itself.
    fn visit_expr(&mut self, e: &'a Expr) {
        walk_expr(self, e);
        if let Expr::Call(c) = e {
            let args = c
                .arguments
                .args
                .iter()
                .chain(c.arguments.keywords.iter().map(|k| &k.value));
            let passed = args
                .filter(|a| matches!(a, Expr::Name(_) | Expr::Attribute(_)))
                .filter_map(|a| match callee_of(a) {
                    c @ (Callee::Name(_) | Callee::Dotted(_)) => Some(c),
                    _ => None,
                })
                .collect();
            self.calls.push(CallSite {
                callee: callee_of(&c.func),
                passed,
            });
        }
    }
}

pub(crate) fn make_func(
    f: &ruff_python_ast::StmtFunctionDef,
    module: usize,
    owner: Option<usize>,
    flow: Option<&Flow>,
) -> Func {
    let mut params = vec![];
    for p in f
        .parameters
        .posonlyargs
        .iter()
        .chain(&f.parameters.args)
        .chain(&f.parameters.kwonlyargs)
    {
        let mut rs = vec![];
        if let Some(a) = &p.parameter.annotation {
            refs(a, &mut rs);
        }
        params.push((p.parameter.name.as_str().to_string(), rs));
    }
    let mut col = Collector {
        calls: vec![],
        locals: HashMap::new(),
        skip_defs: false,
    };
    for s in &f.body {
        col.visit_stmt(s);
    }
    let cfg = flow.and_then(|fl| cfg::build(&f.body, col.calls.len(), fl, false));
    Func {
        module,
        owner,
        name: f.name.as_str().to_string(),
        params,
        calls: col.calls,
        locals: col.locals,
        cfg,
    }
}

pub(crate) fn dotted(e: &Expr) -> Option<String> {
    match e {
        Expr::Name(n) => Some(n.id.as_str().to_string()),
        Expr::Attribute(a) => Some(format!("{}.{}", dotted(&a.value)?, a.attr.as_str())),
        Expr::Subscript(s) => dotted(&s.value),
        _ => None,
    }
}

pub(crate) fn tokens(s: &str, out: &mut Vec<String>) {
    for t in s.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')) {
        let t = t.trim_matches('.');
        if t.chars().next().is_some_and(|c| !c.is_ascii_digit()) {
            out.push(t.to_string());
        }
    }
}

pub(crate) fn refs(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Name(_) | Expr::Attribute(_) => out.extend(dotted(e)),
        Expr::Subscript(s) => {
            refs(&s.value, out);
            refs(&s.slice, out);
        }
        Expr::BinOp(b) => {
            refs(&b.left, out);
            refs(&b.right, out);
        }
        Expr::Tuple(t) => t.elts.iter().for_each(|x| refs(x, out)),
        Expr::List(t) => t.elts.iter().for_each(|x| refs(x, out)),
        Expr::StringLiteral(s) => tokens(s.value.to_str(), out),
        _ => {}
    }
}

pub(crate) fn attr_of(name: &str, ann: &Expr, src: &str) -> Attr {
    let mut rs = vec![];
    refs(ann, &mut rs);
    let r = ann.range();
    Attr {
        name: name.into(),
        ty: src[r.start().to_usize()..r.end().to_usize()].to_string(),
        refs: rs,
    }
}

pub(crate) fn is_self_attr(e: &Expr) -> Option<String> {
    if let Expr::Attribute(a) = e {
        if matches!(&*a.value, Expr::Name(n) if n.id.as_str() == "self") {
            return Some(a.attr.as_str().to_string());
        }
    }
    None
}

pub(crate) fn class_members(
    c: &ruff_python_ast::StmtClassDef,
    src: &str,
) -> (Vec<String>, Vec<Attr>, Vec<(String, String)>) {
    let mut methods = vec![];
    let mut attrs: Vec<Attr> = vec![];
    let mut ctor_attrs: Vec<(String, String)> = vec![];
    let add = |a: Attr, attrs: &mut Vec<Attr>| {
        if !attrs.iter().any(|x| x.name == a.name) {
            attrs.push(a);
        }
    };
    for b in &c.body {
        match b {
            Stmt::FunctionDef(f) => {
                methods.push(f.name.as_str().to_string());
                if f.name.as_str() == "__init__" {
                    let mut ann: BTreeMap<String, &Expr> = BTreeMap::new();
                    for p in f
                        .parameters
                        .posonlyargs
                        .iter()
                        .chain(&f.parameters.args)
                        .chain(&f.parameters.kwonlyargs)
                    {
                        if let Some(a) = &p.parameter.annotation {
                            ann.insert(p.parameter.name.as_str().to_string(), a);
                        }
                    }
                    for s in &f.body {
                        match s {
                            Stmt::AnnAssign(a) => {
                                if let Some(n) = is_self_attr(&a.target) {
                                    add(attr_of(&n, &a.annotation, src), &mut attrs);
                                }
                            }
                            Stmt::Assign(a) => {
                                let Some(t) = a.targets.first().and_then(is_self_attr) else {
                                    continue;
                                };
                                match &*a.value {
                                    Expr::Name(v) => {
                                        if let Some(an) = ann.get(v.id.as_str()) {
                                            add(attr_of(&t, an, src), &mut attrs);
                                        }
                                    }
                                    Expr::Call(call) => {
                                        if let Some(ch) = chain(&call.func) {
                                            ctor_attrs.push((t, ch.join(".")));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Stmt::AnnAssign(a) => {
                if let Expr::Name(n) = &*a.target {
                    add(attr_of(n.id.as_str(), &a.annotation, src), &mut attrs);
                }
            }
            _ => {}
        }
    }
    (methods, attrs, ctor_attrs)
}

pub(crate) fn collect_items(
    body: &[Stmt],
    module: usize,
    src: &str,
    out: &mut Vec<Class>,
    funcs: &mut Vec<Func>,
    flow: Option<&Flow>,
) {
    for s in body {
        match s {
            Stmt::ClassDef(c) => {
                let bases = c
                    .arguments
                    .as_ref()
                    .map(|a| a.args.iter().filter_map(dotted).collect())
                    .unwrap_or_default();
                let (methods, attrs, ctor_attrs) = class_members(c, src);
                let idx = out.len();
                out.push(Class {
                    module,
                    name: c.name.as_str().into(),
                    bases,
                    methods,
                    attrs,
                    ctor_attrs,
                });
                for b in &c.body {
                    match b {
                        Stmt::FunctionDef(f) => funcs.push(make_func(f, module, Some(idx), flow)),
                        Stmt::ClassDef(_) => {
                            collect_items(std::slice::from_ref(b), module, src, out, funcs, flow)
                        }
                        _ => {}
                    }
                }
            }
            Stmt::FunctionDef(f) => funcs.push(make_func(f, module, None, flow)),
            Stmt::If(i) => {
                collect_items(&i.body, module, src, out, funcs, flow);
                for cl in &i.elif_else_clauses {
                    collect_items(&cl.body, module, src, out, funcs, flow);
                }
            }
            Stmt::Try(t) => collect_items(&t.body, module, src, out, funcs, flow),
            _ => {}
        }
    }
}

pub(crate) fn collect_imports(body: &[Stmt], pkg: &str, m: &mut Module) {
    for s in body {
        match s {
            Stmt::Import(i) => {
                for a in &i.names {
                    let full = a.name.as_str().to_string();
                    match &a.asname {
                        Some(n) => m.bindings.insert(n.as_str().into(), Binding::Mod(full)),
                        None => {
                            let head = full.split('.').next().unwrap_or_default().to_string();
                            m.bindings.insert(head.clone(), Binding::Mod(head))
                        }
                    };
                }
            }
            Stmt::ImportFrom(f) => {
                let Some(base) = relative_base(pkg, f.level, f.module.as_ref().map(|x| x.as_str()))
                else {
                    continue;
                };
                for a in &f.names {
                    if a.name.as_str() == "*" {
                        m.stars.push(base.clone());
                    } else {
                        let local = a.asname.as_ref().unwrap_or(&a.name).as_str().to_string();
                        m.bindings
                            .insert(local, Binding::Sym(base.clone(), a.name.as_str().into()));
                    }
                }
            }
            Stmt::If(i) => {
                collect_imports(&i.body, pkg, m);
                for cl in &i.elif_else_clauses {
                    collect_imports(&cl.body, pkg, m);
                }
            }
            Stmt::Try(t) => collect_imports(&t.body, pkg, m),
            _ => {}
        }
    }
}

/// Read `files` (reported relative to `root_dir`) into modules, classes and functions.
///
/// Files that are not valid Python are listed in `parse_failures`; files that cannot be read are skipped.
pub(crate) fn load_project(
    root_dir: &Path,
    files: &[PathBuf],
    control_flow: Option<bool>,
) -> Project {
    let mut project = Project {
        mods: vec![],
        classes: vec![],
        funcs: vec![],
        parse_failures: vec![],
        files: files.len(),
    };
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(root_dir)
            .unwrap_or(path)
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(parsed) = ruff_python_parser::parse_module(&src) else {
            project.parse_failures.push(rel);
            continue;
        };
        let (scope, dotted_name, is_init) = module_of(path);
        let package = if is_init {
            dotted_name.clone()
        } else {
            dotted_name
                .rsplit_once('.')
                .map(|(head, _)| head.to_string())
                .unwrap_or_default()
        };
        let index = project.mods.len();
        let mut module = Module {
            rel,
            scope,
            dotted: dotted_name,
            bindings: HashMap::new(),
            stars: vec![],
        };
        let body = &parsed.syntax().body;
        collect_imports(body, &package, &mut module);
        project.mods.push(module);
        let line_index = LineIndex::from_source_text(&src);
        let flow = control_flow.map(|text| Flow {
            src: &src,
            index: &line_index,
            text,
        });
        collect_items(
            body,
            index,
            &src,
            &mut project.classes,
            &mut project.funcs,
            flow.as_ref(),
        );
        // Top-level statements outside any function or class become a function of their own.
        let mut top_level = Collector {
            calls: vec![],
            locals: HashMap::new(),
            skip_defs: true,
        };
        for statement in body {
            top_level.visit_stmt(statement);
        }
        if !top_level.calls.is_empty() {
            project.funcs.push(Func {
                module: index,
                owner: None,
                name: "<module>".into(),
                params: vec![],
                cfg: flow
                    .as_ref()
                    .and_then(|fl| cfg::build(body, top_level.calls.len(), fl, true)),
                calls: top_level.calls,
                locals: top_level.locals,
            });
        }
    }
    project
}

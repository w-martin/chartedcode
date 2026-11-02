//! Resolving names to classes and functions through imports.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::model::{Binding, Class, Func, Module};
use crate::modules::join;

pub(crate) struct Found {
    pub(crate) class: usize,
    pub(crate) reexport: bool,
    pub(crate) other_root: bool,
}

pub(crate) enum Res {
    Class(usize, &'static str),
    External,
    Unresolved,
}

pub(crate) struct Resolver<'a> {
    /// Answers to top-level symbol lookups, which repeat once per call site.
    pub(crate) symbol_cache: RefCell<HashMap<(usize, bool, String), Option<FoundSym>>>,
    pub(crate) mods: &'a [Module],
    pub(crate) by_mod: HashMap<(PathBuf, String), usize>,
    pub(crate) by_dotted: HashMap<String, Vec<usize>>,
    pub(crate) class_by_name: HashMap<(usize, String), usize>,
    pub(crate) func_by_name: HashMap<(usize, String), usize>,
}

#[derive(Clone, Copy)]
pub(crate) enum Sym {
    Class(usize),
    Func(usize),
}

#[derive(Clone, Copy)]
pub(crate) struct FoundSym {
    pub(crate) sym: Sym,
    pub(crate) reexport: bool,
    pub(crate) other_root: bool,
    pub(crate) local: bool,
}

pub(crate) enum SymRes {
    Sym(FoundSym, usize),
    External,
    Unresolved,
    Unbound,
}

impl Resolver<'_> {
    pub(crate) fn find_module(&self, scope: &Path, dotted: &str) -> Option<(usize, bool)> {
        if let Some(&i) = self.by_mod.get(&(scope.to_path_buf(), dotted.to_string())) {
            return Some((i, false));
        }
        match self.by_dotted.get(dotted).map(Vec::as_slice) {
            Some([i]) => Some((*i, true)),
            _ => None,
        }
    }

    /// Find the class or function `name` in `module`, following re-exports and star imports.
    pub(crate) fn lookup_symbol(
        &self,
        scope: &Path,
        module: &str,
        name: &str,
        depth: u8,
    ) -> Option<FoundSym> {
        // Only a top-level answer is complete; deeper ones may have been cut short by the depth limit.
        if depth > 0 {
            return self.lookup_symbol_uncached(scope, module, name, depth);
        }
        let (index, other) = self.find_module(scope, module)?;
        let key = (index, other, name.to_string());
        if let Some(found) = self.symbol_cache.borrow().get(&key) {
            return *found;
        }
        let found = self.lookup_symbol_uncached(scope, module, name, depth);
        self.symbol_cache.borrow_mut().insert(key, found);
        found
    }

    fn lookup_symbol_uncached(
        &self,
        scope: &Path,
        module: &str,
        name: &str,
        depth: u8,
    ) -> Option<FoundSym> {
        if depth > 8 {
            return None;
        }
        let (mi, other) = self.find_module(scope, module)?;
        let m = &self.mods[mi];
        let key = (mi, name.to_string());
        if let Some(&c) = self.class_by_name.get(&key) {
            return Some(FoundSym {
                sym: Sym::Class(c),
                reexport: false,
                other_root: other,
                local: false,
            });
        }
        if let Some(&f) = self.func_by_name.get(&key) {
            return Some(FoundSym {
                sym: Sym::Func(f),
                reexport: false,
                other_root: other,
                local: false,
            });
        }
        let via = |f: Option<FoundSym>| {
            f.map(|f| FoundSym {
                sym: f.sym,
                reexport: true,
                other_root: f.other_root || other,
                local: false,
            })
        };
        if let Some(Binding::Sym(m2, n2)) = m.bindings.get(name) {
            return via(self.lookup_symbol(&m.scope, m2, n2, depth + 1));
        }
        for s in &m.stars {
            if let Some(f) = via(self.lookup_symbol(&m.scope, s, name, depth + 1)) {
                return Some(f);
            }
        }
        None
    }

    pub(crate) fn lookup_class(
        &self,
        scope: &Path,
        module: &str,
        name: &str,
        depth: u8,
    ) -> Option<Found> {
        let f = self.lookup_symbol(scope, module, name, depth)?;
        match f.sym {
            Sym::Class(c) => Some(Found {
                class: c,
                reexport: f.reexport,
                other_root: f.other_root,
            }),
            Sym::Func(_) => None,
        }
    }

    // Resolves the leading dotted names of a callee; `consumed` says how many of `parts` the symbol covers.
    pub(crate) fn resolve_symbol_path(&self, from_module: usize, parts: &[&str]) -> SymRes {
        let m = &self.mods[from_module];
        let key = (from_module, parts[0].to_string());
        if let Some(&c) = self.class_by_name.get(&key) {
            return SymRes::Sym(
                FoundSym {
                    sym: Sym::Class(c),
                    reexport: false,
                    other_root: false,
                    local: true,
                },
                1,
            );
        }
        if let Some(&f) = self.func_by_name.get(&key) {
            return SymRes::Sym(
                FoundSym {
                    sym: Sym::Func(f),
                    reexport: false,
                    other_root: false,
                    local: true,
                },
                1,
            );
        }
        let through = |base: &str| -> SymRes {
            for k in 1..parts.len() {
                let path = join(base, &parts[1..k].join("."));
                if let Some(f) = self.lookup_symbol(&m.scope, &path, parts[k], 0) {
                    return SymRes::Sym(f, k + 1);
                }
            }
            if self.find_module(&m.scope, base).is_none() {
                SymRes::External
            } else {
                SymRes::Unresolved
            }
        };
        match m.bindings.get(parts[0]) {
            Some(Binding::Mod(md)) => through(md),
            Some(Binding::Sym(sm, sn)) => {
                if let Some(f) = self.lookup_symbol(&m.scope, sm, sn, 0) {
                    return SymRes::Sym(f, 1);
                }
                let base = join(sm, sn);
                if self.find_module(&m.scope, &base).is_some() {
                    through(&base)
                } else if self.find_module(&m.scope, sm).is_none() {
                    SymRes::External
                } else {
                    SymRes::Unresolved
                }
            }
            None => {
                if parts.len() == 1 {
                    for s in &m.stars {
                        if let Some(f) = self.lookup_symbol(&m.scope, s, parts[0], 0) {
                            return SymRes::Sym(f, 1);
                        }
                    }
                }
                SymRes::Unbound
            }
        }
    }

    pub(crate) fn evidence(f: &Found) -> &'static str {
        if f.other_root {
            "other-root"
        } else if f.reexport {
            "re-export"
        } else {
            "import"
        }
    }

    pub(crate) fn through_module(&self, scope: &Path, module_path: &str, class: &str) -> Res {
        match self.lookup_class(scope, module_path, class, 0) {
            Some(f) => Res::Class(f.class, Self::evidence(&f)),
            None if self.find_module(scope, module_path).is_none() => Res::External,
            None => Res::Unresolved,
        }
    }

    pub(crate) fn resolve(
        &self,
        from_module: usize,
        reference: &str,
        exclude: Option<usize>,
    ) -> Res {
        let m = &self.mods[from_module];
        let parts: Vec<&str> = reference.split('.').collect();
        if parts.len() == 1 {
            if let Some(&c) = self.class_by_name.get(&(from_module, parts[0].to_string())) {
                if Some(c) != exclude {
                    return Res::Class(c, "same-module");
                }
            }
        }
        let tail = |from: usize| parts[from..parts.len() - 1].join(".");
        match m.bindings.get(parts[0]) {
            Some(Binding::Mod(md)) if parts.len() > 1 => {
                self.through_module(&m.scope, &join(md, &tail(1)), parts[parts.len() - 1])
            }
            Some(Binding::Mod(_)) => Res::Unresolved,
            Some(Binding::Sym(sm, sn)) if parts.len() == 1 => {
                match self.lookup_class(&m.scope, sm, sn, 0) {
                    Some(f) => Res::Class(f.class, Self::evidence(&f)),
                    None if self.find_module(&m.scope, sm).is_none() => Res::External,
                    None => Res::Unresolved,
                }
            }
            Some(Binding::Sym(sm, sn)) => {
                let base = join(&join(sm, sn), &tail(1));
                self.through_module(&m.scope, &base, parts[parts.len() - 1])
            }
            None if parts.len() == 1 => {
                for s in &m.stars {
                    if let Some(f) = self.lookup_class(&m.scope, s, parts[0], 0) {
                        return Res::Class(f.class, Self::evidence(&f));
                    }
                }
                Res::External
            }
            None => Res::External,
        }
    }
}

impl<'a> Resolver<'a> {
    /// Index `mods`, `classes` and `funcs` for lookup by module and name.
    pub(crate) fn new(mods: &'a [Module], classes: &[Class], funcs: &[Func]) -> Self {
        let mut by_mod = HashMap::new();
        let mut by_dotted: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, m) in mods.iter().enumerate() {
            by_mod.insert((m.scope.clone(), m.dotted.clone()), i);
            by_dotted.entry(m.dotted.clone()).or_default().push(i);
        }
        let mut class_by_name = HashMap::new();
        for (i, c) in classes.iter().enumerate() {
            class_by_name.entry((c.module, c.name.clone())).or_insert(i);
        }
        let mut func_by_name = HashMap::new();
        for (i, f) in funcs.iter().enumerate() {
            if f.owner.is_none() && f.name != "<module>" {
                func_by_name.insert((f.module, f.name.clone()), i);
            }
        }
        Self {
            mods,
            by_mod,
            by_dotted,
            class_by_name,
            func_by_name,
            symbol_cache: RefCell::new(HashMap::new()),
        }
    }
}

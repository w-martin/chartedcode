//! Control-flow graphs: the basic blocks of a function and the branches between them.
//!
//! Statements are visited in the same order as the call collector walks them, so each block can say which
//! of the function's call sites it holds; a mismatch means a bug, and the graph is dropped instead of
//! being linked to the wrong calls.
//!
//! Approximations, stated here so the diagram can be read correctly: any statement in a `try` body may
//! raise (one `exception` edge from the body's first block to each handler), context managers do not
//! branch, and boolean operators, conditional expressions and comprehensions do not split a block.

use std::collections::BTreeSet;

use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::{ExceptHandler, Expr, Pattern, Stmt};
use ruff_source_file::LineIndex;
use ruff_text_size::{Ranged, TextRange};

use crate::model::{BlockKind, Cfg, CfgBlock, CfgEdge, CfgLine};
use crate::parse::Collector;

/// Where a function's source text is, for line numbers and the text shown in blocks.
pub(crate) struct Flow<'a> {
    pub(crate) src: &'a str,
    pub(crate) index: &'a LineIndex,
    /// Keep the statements' text; without it blocks show only line numbers.
    pub(crate) text: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dest {
    Return,
    Raise,
    Break(usize),
    Continue(usize),
}

struct LoopCtx {
    header: usize,
    after: usize,
    /// How many `try` contexts were open when the loop began.
    tries: usize,
}

struct TryCtx {
    handlers: Vec<usize>,
    finally: Option<usize>,
    /// Exits routed through the `finally` block, to be continued after it.
    pending: Vec<(Dest, String)>,
}

struct Builder<'a> {
    flow: &'a Flow<'a>,
    skip_defs: bool,
    blocks: Vec<CfgBlock>,
    edges: Vec<CfgEdge>,
    exit: usize,
    cursor: usize,
    loops: Vec<LoopCtx>,
    tries: Vec<TryCtx>,
    decisions: usize,
}

/// Build the graph of `body`, whose call sites number `calls`. `skip_defs` leaves the insides of nested
/// definitions out of the blocks (as for a module's top-level code).
pub(crate) fn build(body: &[Stmt], calls: usize, flow: &Flow, skip_defs: bool) -> Option<Cfg> {
    let mut b = Builder {
        flow,
        skip_defs,
        blocks: vec![],
        edges: vec![],
        exit: 1,
        cursor: 0,
        loops: vec![],
        tries: vec![],
        decisions: 0,
    };
    let entry = b.new_block(BlockKind::Entry);
    let exit = b.new_block(BlockKind::Exit);
    b.exit = exit;
    let first = b.new_block(BlockKind::Plain);
    b.edge(entry, first, "");
    if let Some(end) = b.stmts(body, first) {
        b.edge(end, exit, "");
    }
    if b.cursor != calls {
        return None;
    }
    b.mark_unreachable(entry);
    Some(Cfg {
        blocks: b.blocks,
        edges: b.edges,
        entry,
        exit,
        complexity: b.decisions + 1,
    })
}

impl Builder<'_> {
    fn new_block(&mut self, kind: BlockKind) -> usize {
        self.blocks.push(CfgBlock {
            kind,
            lines: vec![],
            calls: vec![],
            unreachable: false,
        });
        self.blocks.len() - 1
    }

    fn edge(&mut self, from: usize, to: usize, label: &str) {
        self.edges.push(CfgEdge {
            from,
            to,
            label: label.into(),
        });
    }

    fn line_of(&self, range: TextRange) -> usize {
        self.flow.index.line_index(range.start()).get()
    }

    /// The first line of the source at `range`, trimmed and shortened; "…" marks that more follows.
    fn snippet(&self, range: TextRange) -> String {
        if !self.flow.text {
            return String::new();
        }
        let text = self
            .flow
            .src
            .get(range.start().to_usize()..range.end().to_usize())
            .unwrap_or("");
        let mut lines = text.lines();
        let first = lines.next().unwrap_or("").trim();
        let more = lines.next().is_some();
        let mut shown: String = first.chars().take(100).collect();
        if more || first.chars().count() > 100 {
            shown.push('…');
        }
        shown
    }

    fn add_line(&mut self, block: usize, range: TextRange, text: String) {
        let line = self.line_of(range);
        self.blocks[block].lines.push(CfgLine { line, text });
    }

    /// Run `visit` with a fresh call collector and give the call sites it finds to `block`.
    fn collect(&mut self, block: usize, visit: impl FnOnce(&mut Collector)) {
        let mut collector = Collector {
            calls: vec![],
            locals: std::collections::HashMap::new(),
            skip_defs: self.skip_defs,
        };
        visit(&mut collector);
        for _ in &collector.calls {
            self.blocks[block].calls.push(self.cursor);
            self.cursor += 1;
        }
    }

    fn stmts(&mut self, body: &[Stmt], first: usize) -> Option<usize> {
        let mut cur = Some(first);
        for s in body {
            // Code after a return, raise, break or continue still counts its calls, in a block nothing reaches.
            let block = cur.unwrap_or_else(|| self.new_block(BlockKind::Plain));
            cur = self.stmt(s, block);
        }
        cur
    }

    /// Send control from `from` to `dest`, through any `finally` block or handler in between.
    fn route(&mut self, from: usize, dest: Dest, label: &str) {
        let lowest = match dest {
            Dest::Break(l) | Dest::Continue(l) => self.loops[l].tries,
            Dest::Return | Dest::Raise => 0,
        };
        for i in (lowest..self.tries.len()).rev() {
            if dest == Dest::Raise && !self.tries[i].handlers.is_empty() {
                for handler in self.tries[i].handlers.clone() {
                    self.edge(from, handler, label);
                }
                return;
            }
            if let Some(finally) = self.tries[i].finally {
                self.edge(from, finally, label);
                if !self.tries[i].pending.iter().any(|(d, _)| *d == dest) {
                    self.tries[i].pending.push((dest, label.to_string()));
                }
                return;
            }
        }
        let target = match dest {
            Dest::Return | Dest::Raise => self.exit,
            Dest::Break(l) => self.loops[l].after,
            Dest::Continue(l) => self.loops[l].header,
        };
        self.edge(from, target, label);
    }

    fn stmt(&mut self, s: &Stmt, cur: usize) -> Option<usize> {
        match s {
            Stmt::If(i) => self.if_stmt(i, cur),
            Stmt::While(w) => Some(self.while_stmt(w, cur)),
            Stmt::For(f) => Some(self.for_stmt(f, cur)),
            Stmt::Try(t) => self.try_stmt(t, cur),
            Stmt::With(w) => {
                let header = if self.flow.text {
                    format!(
                        "{}with {}",
                        if w.is_async { "async " } else { "" },
                        self.with_items(w)
                    )
                } else {
                    String::new()
                };
                self.add_line(cur, w.range(), header);
                for item in &w.items {
                    self.collect(cur, |c| {
                        c.visit_with_item(item);
                    });
                }
                self.stmts(&w.body, cur)
            }
            Stmt::Match(m) => self.match_stmt(m, cur),
            Stmt::Return(_) => {
                self.simple(s, cur);
                self.route(cur, Dest::Return, "return");
                None
            }
            Stmt::Raise(_) => {
                self.simple(s, cur);
                self.route(cur, Dest::Raise, "raise");
                None
            }
            Stmt::Assert(_) => {
                self.simple(s, cur);
                self.decisions += 1;
                self.route(cur, Dest::Raise, "assertion fails");
                Some(cur)
            }
            Stmt::Break(_) => {
                self.simple(s, cur);
                if self.loops.is_empty() {
                    return None;
                }
                self.route(cur, Dest::Break(self.loops.len() - 1), "break");
                None
            }
            Stmt::Continue(_) => {
                self.simple(s, cur);
                if self.loops.is_empty() {
                    return None;
                }
                self.route(cur, Dest::Continue(self.loops.len() - 1), "continue");
                None
            }
            _ => {
                self.simple(s, cur);
                Some(cur)
            }
        }
    }

    fn simple(&mut self, s: &Stmt, block: usize) {
        let text = self.snippet(s.range());
        self.add_line(block, s.range(), text);
        self.collect(block, |c| c.visit_stmt(s));
    }

    fn with_items(&self, w: &ruff_python_ast::StmtWith) -> String {
        w.items
            .iter()
            .map(|item| self.snippet(item.range()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn header(&mut self, block: usize, range: TextRange, keyword: &str, expr: &Expr) {
        let text = if self.flow.text {
            format!("{keyword} {}", self.snippet(expr.range()))
        } else {
            String::new()
        };
        self.add_line(block, range, text);
    }

    fn if_stmt(&mut self, i: &ruff_python_ast::StmtIf, cur: usize) -> Option<usize> {
        self.header(cur, i.range(), "if", &i.test);
        self.collect(cur, |c| c.visit_expr(&i.test));
        self.decisions += 1;
        let mut ends = vec![];
        let then = self.new_block(BlockKind::Plain);
        self.edge(cur, then, "true");
        ends.extend(self.stmts(&i.body, then));
        let mut header = cur;
        let mut has_else = false;
        for clause in &i.elif_else_clauses {
            if let Some(test) = &clause.test {
                let h = self.new_block(BlockKind::Branch);
                self.edge(header, h, "false");
                self.header(h, clause.range(), "elif", test);
                self.collect(h, |c| c.visit_expr(test));
                self.decisions += 1;
                let body = self.new_block(BlockKind::Plain);
                self.edge(h, body, "true");
                ends.extend(self.stmts(&clause.body, body));
                header = h;
            } else {
                has_else = true;
                let body = self.new_block(BlockKind::Plain);
                self.edge(header, body, "false");
                ends.extend(self.stmts(&clause.body, body));
            }
        }
        if ends.is_empty() && has_else {
            return None;
        }
        let join = self.new_block(BlockKind::Plain);
        for end in ends {
            self.edge(end, join, "");
        }
        if !has_else {
            self.edge(header, join, "false");
        }
        Some(join)
    }

    fn while_stmt(&mut self, w: &ruff_python_ast::StmtWhile, cur: usize) -> usize {
        let header = self.new_block(BlockKind::Loop);
        self.edge(cur, header, "");
        self.header(header, w.range(), "while", &w.test);
        self.collect(header, |c| c.visit_expr(&w.test));
        let infinite = matches!(&*w.test, Expr::BooleanLiteral(l) if l.value);
        if !infinite {
            self.decisions += 1;
        }
        let body = self.new_block(BlockKind::Plain);
        let after = self.new_block(BlockKind::Plain);
        self.edge(header, body, "true");
        self.loop_body(header, after, &w.body, body);
        self.loop_exit(header, after, &w.orelse, infinite, "false");
        after
    }

    fn for_stmt(&mut self, f: &ruff_python_ast::StmtFor, cur: usize) -> usize {
        let header = self.new_block(BlockKind::Loop);
        self.edge(cur, header, "");
        let text = if self.flow.text {
            format!(
                "{}for {} in {}",
                if f.is_async { "async " } else { "" },
                self.snippet(f.target.range()),
                self.snippet(f.iter.range())
            )
        } else {
            String::new()
        };
        self.add_line(header, f.range(), text);
        self.collect(header, |c| {
            c.visit_expr(&f.iter);
            c.visit_expr(&f.target);
        });
        self.decisions += 1;
        let body = self.new_block(BlockKind::Plain);
        let after = self.new_block(BlockKind::Plain);
        self.edge(header, body, "next item");
        self.loop_body(header, after, &f.body, body);
        self.loop_exit(header, after, &f.orelse, false, "no more items");
        after
    }

    fn loop_body(&mut self, header: usize, after: usize, body: &[Stmt], first: usize) {
        self.loops.push(LoopCtx {
            header,
            after,
            tries: self.tries.len(),
        });
        if let Some(end) = self.stmts(body, first) {
            self.edge(end, header, "loop");
        }
        self.loops.pop();
    }

    /// The edges that leave a loop when it ends without `break`: to its `else` clause if it has one.
    fn loop_exit(
        &mut self,
        header: usize,
        after: usize,
        orelse: &[Stmt],
        infinite: bool,
        label: &str,
    ) {
        if orelse.is_empty() {
            if !infinite {
                self.edge(header, after, label);
            }
            return;
        }
        let else_block = self.new_block(BlockKind::Plain);
        if !infinite {
            self.edge(header, else_block, label);
        }
        if let Some(end) = self.stmts(orelse, else_block) {
            self.edge(end, after, "");
        }
    }

    fn try_stmt(&mut self, t: &ruff_python_ast::StmtTry, cur: usize) -> Option<usize> {
        self.add_line(
            cur,
            t.range(),
            if self.flow.text {
                "try".into()
            } else {
                String::new()
            },
        );
        let body = self.new_block(BlockKind::Plain);
        self.edge(cur, body, "");
        let handlers: Vec<usize> = t
            .handlers
            .iter()
            .map(|_| self.new_block(BlockKind::Handler))
            .collect();
        let finally = (!t.finalbody.is_empty()).then(|| self.new_block(BlockKind::Finally));
        for &h in &handlers {
            self.edge(body, h, "exception");
        }
        self.tries.push(TryCtx {
            handlers: handlers.clone(),
            finally,
            pending: vec![],
        });
        let body_end = self.stmts(&t.body, body);
        // Past the body, a raise is no longer caught by this try's handlers.
        if let Some(ctx) = self.tries.last_mut() {
            ctx.handlers.clear();
        }
        let mut ends: Vec<usize> = vec![];
        for (handler, &block) in t.handlers.iter().zip(&handlers) {
            let ExceptHandler::ExceptHandler(h) = handler;
            let text = if self.flow.text {
                let kind = h
                    .type_
                    .as_ref()
                    .map(|e| self.snippet(e.range()))
                    .unwrap_or_default();
                let name = h
                    .name
                    .as_ref()
                    .map(|n| format!(" as {}", n.as_str()))
                    .unwrap_or_default();
                format!("except {kind}{name}").replace("except  ", "except ")
            } else {
                String::new()
            };
            self.add_line(block, h.range(), text);
            if let Some(kind) = &h.type_ {
                self.collect(block, |c| c.visit_expr(kind));
            }
            self.decisions += 1;
            ends.extend(self.stmts(&h.body, block));
        }
        if let Some(end) = body_end {
            if t.orelse.is_empty() {
                ends.push(end);
            } else {
                let else_block = self.new_block(BlockKind::Plain);
                self.edge(end, else_block, "no exception");
                ends.extend(self.stmts(&t.orelse, else_block));
            }
        }
        let ctx = self.tries.pop()?;
        let Some(finally) = ctx.finally else {
            if ends.is_empty() {
                return None;
            }
            let join = self.new_block(BlockKind::Plain);
            for end in ends {
                self.edge(end, join, "");
            }
            return Some(join);
        };
        for &end in &ends {
            self.edge(end, finally, "");
        }
        let end = self.stmts(&t.finalbody, finally)?;
        for (dest, label) in ctx.pending {
            self.route(end, dest, &format!("{label}, after finally"));
        }
        (!ends.is_empty()).then_some(end)
    }

    fn match_stmt(&mut self, m: &ruff_python_ast::StmtMatch, cur: usize) -> Option<usize> {
        self.header(cur, m.range(), "match", &m.subject);
        self.collect(cur, |c| c.visit_expr(&m.subject));
        let mut ends = vec![];
        let mut exhaustive = false;
        for case in &m.cases {
            let block = self.new_block(BlockKind::Plain);
            self.edge(cur, block, "case");
            let irrefutable = matches!(&case.pattern, Pattern::MatchAs(p) if p.pattern.is_none())
                && case.guard.is_none();
            let text = if self.flow.text {
                let guard = case
                    .guard
                    .as_ref()
                    .map(|g| format!(" if {}", self.snippet(g.range())))
                    .unwrap_or_default();
                format!("case {}{guard}", self.snippet(case.pattern.range()))
            } else {
                String::new()
            };
            self.add_line(block, case.range(), text);
            self.collect(block, |c| {
                c.visit_pattern(&case.pattern);
                if let Some(guard) = &case.guard {
                    c.visit_expr(guard);
                }
            });
            if irrefutable {
                exhaustive = true;
            } else {
                self.decisions += 1;
            }
            ends.extend(self.stmts(&case.body, block));
        }
        if ends.is_empty() && exhaustive {
            return None;
        }
        let join = self.new_block(BlockKind::Plain);
        for end in ends {
            self.edge(end, join, "");
        }
        if !exhaustive {
            self.edge(cur, join, "no case matches");
        }
        Some(join)
    }

    /// Mark the blocks that no path from `entry` reaches.
    fn mark_unreachable(&mut self, entry: usize) {
        let mut reached: BTreeSet<usize> = BTreeSet::from([entry]);
        let mut stack = vec![entry];
        while let Some(b) = stack.pop() {
            for e in self.edges.iter().filter(|e| e.from == b) {
                if reached.insert(e.to) {
                    stack.push(e.to);
                }
            }
        }
        for (i, block) in self.blocks.iter_mut().enumerate() {
            block.unreachable = !reached.contains(&i);
        }
    }
}

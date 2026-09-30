use super::*;
use crate::ast::{BinOp as AOp, ExprKind as A, ForIter, StmtKind as S, UnOp as AUn};
use crate::hir::{BinOp, Cmp, UnOp};
use std::collections::HashSet;

#[derive(Default, Clone)]
pub struct Facts {
    pub t: Vec<(u32, TyId)>,
    pub f: Vec<(u32, TyId)>,
}

fn stmt_diverges(s: &Stmt) -> bool {
    match s {
        Stmt::Return(_) | Stmt::Break | Stmt::Continue => true,
        Stmt::If(_, a, b) => diverges(a) && diverges(b),
        Stmt::Expr(e) => matches!(e.kind, ExprKind::Rt(RtFn::ExitNow | RtFn::Panic | RtFn::ErrReturn, _)),
        Stmt::Loop { cond, body, .. } => {
            let infinite = match cond {
                None => true,
                Some(c) => matches!(c.kind, ExprKind::Bool(true)),
            };
            infinite && !has_break(body)
        }
    }
}

fn has_break(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Break => true,
        Stmt::If(_, a, b) => has_break(a) || has_break(b),
        _ => false,
    })
}

pub fn diverges(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_diverges)
}

fn collect_assigned(stmts: &[ast::Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        collect_stmt(s, out);
    }
}

fn collect_stmt(s: &ast::Stmt, out: &mut HashSet<String>) {
    match &s.kind {
        S::Var { init: Some(e), .. } => collect_expr(e, out),
        S::Expr(e) => collect_expr(e, out),
        S::If { cond, then, els } => {
            collect_expr(cond, out);
            collect_assigned(&then.stmts, out);
            if let Some(b) = els {
                collect_assigned(&b.stmts, out);
            }
        }
        S::While { cond, body } => {
            collect_expr(cond, out);
            collect_assigned(&body.stmts, out);
        }
        S::For { init, cond, step, body } => {
            if let Some(i) = init {
                collect_stmt(i, out);
            }
            if let Some(c) = cond {
                collect_expr(c, out);
            }
            if let Some(st) = step {
                collect_expr(st, out);
            }
            collect_assigned(&body.stmts, out);
        }
        S::ForIn { var, index, iter, body } => {
            out.insert(var.name.clone());
            if let Some(i) = index {
                out.insert(i.name.clone());
            }
            match iter {
                ForIter::Range(a, b, _) => {
                    collect_expr(a, out);
                    collect_expr(b, out);
                }
                ForIter::Expr(e) => collect_expr(e, out),
            }
            collect_assigned(&body.stmts, out);
        }
        S::Return(Some(e)) => collect_expr(e, out),
        S::Block(b) => collect_assigned(&b.stmts, out),
        _ => {}
    }
}

fn collect_expr(e: &ast::Expr, out: &mut HashSet<String>) {
    match &e.kind {
        A::Assign { target, value, .. } => {
            if let A::Ident(n) = &target.kind {
                out.insert(n.clone());
            } else {
                collect_expr(target, out);
            }
            collect_expr(value, out);
        }
        A::Unary(_, x) | A::Await(x) | A::NotNull(x) | A::Is(x, _) | A::As(x, _) | A::SafeAs(x, _) => collect_expr(x, out),
        A::Coalesce(a, b) => {
            collect_expr(a, out);
            collect_expr(b, out);
        }
        A::SafeGet { obj, args, .. } => {
            collect_expr(obj, out);
            for a in args.iter().flatten() {
                collect_expr(a, out);
            }
        }
        A::Binary(_, a, b) => {
            collect_expr(a, out);
            collect_expr(b, out);
        }
        A::Call { callee, args } => {
            collect_expr(callee, out);
            for a in args {
                collect_expr(a, out);
            }
        }
        A::Field { obj, .. } => collect_expr(obj, out),
        A::Index { obj, index } => {
            collect_expr(obj, out);
            collect_expr(index, out);
        }
        A::Array(xs) => xs.iter().for_each(|x| collect_expr(x, out)),
        A::StructLit { fields, .. } => fields.iter().for_each(|(_, x)| collect_expr(x, out)),
        A::MapLit(ps) => ps.iter().for_each(|(a, b)| {
            collect_expr(a, out);
            collect_expr(b, out)
        }),
        A::Template(parts) => {
            for p in parts {
                if let ast::TplExpr::Expr(x) = p {
                    collect_expr(x, out)
                }
            }
        }
        _ => {}
    }
}

impl<'a> Checker<'a> {
    pub fn apply(&mut self, facts: &[(u32, TyId)]) {
        for (s, t) in facts {
            self.ctx().narrow.insert(*s, *t);
        }
    }

    fn forget_assigned(&mut self, names: &HashSet<String>) {
        for n in names {
            if let Some(l) = self.lookup_local(n) {
                self.ctx().narrow.remove(&l.slot);
                self.clear_facts(l.slot);
            } else {
                let m = self.cur_module();
                if let Some(Entry { sym: ValSym::Global(g), .. }) = self.lookup_value_entry(m, n) {
                    self.ctx().narrow.remove(&(GLOBAL_KEY + g));
                    self.clear_facts(GLOBAL_KEY + g);
                }
            }
        }
    }

    pub fn local_of(&self, e: &ast::Expr) -> Option<u32> {
        if let A::Ident(n) = &e.kind {
            if let Some(l) = self.lookup_local(n) {
                return Some(l.slot);
            }
            if self.self_field(n).is_some() {
                return None;
            }
            let m = self.cur_module();
            if let Some(Entry { sym: ValSym::Global(g), .. }) = self.lookup_value_entry(m, n) {
                if self.globals[g as usize].ty.is_some() {
                    return Some(GLOBAL_KEY + g);
                }
            }
        }
        None
    }

    pub fn cond(&mut self, e: &ast::Expr) -> (Expr, Facts) {
        match &e.kind {
            A::Unary(AUn::Not, inner) => {
                let (c, f) = self.cond(inner);
                let c = if let ExprKind::Bool(b) = c.kind {
                    Expr::new(ExprKind::Bool(!b), T_BOOL)
                } else {
                    Expr::new(ExprKind::Unary(UnOp::Not, Box::new(c)), T_BOOL)
                };
                (c, Facts { t: f.f, f: f.t })
            }
            A::Binary(AOp::And, a, b) => {
                let (ca, fa) = self.cond(a);
                let saved = self.ctx().narrow.clone();
                self.apply(&fa.t);
                let (cb, fb) = self.cond(b);
                self.ctx().narrow = saved;
                let mut t = fa.t;
                t.extend(fb.t);
                (Expr::new(ExprKind::And(Box::new(ca), Box::new(cb)), T_BOOL), Facts { t, f: vec![] })
            }
            A::Binary(AOp::Or, a, b) => {
                let (ca, fa) = self.cond(a);
                let saved = self.ctx().narrow.clone();
                self.apply(&fa.f);
                let (cb, fb) = self.cond(b);
                self.ctx().narrow = saved;
                let mut f = fa.f;
                f.extend(fb.f);
                (Expr::new(ExprKind::Or(Box::new(ca), Box::new(cb)), T_BOOL), Facts { t: vec![], f })
            }
            A::Is(x, te) => {
                let h = self.expr_raw(x);
                let t = self.resolve_type(te);
                let mut facts = Facts::default();
                if let Some(slot) = self.local_of(x) {
                    let declared = self.declared_of(slot);
                    if t == T_NULL {
                        let u = self.types.unwrap_optional(declared);
                        if u != declared && self.narrowable(declared, u) {
                            facts.f.push((slot, u));
                        }
                    } else if self.narrowable(declared, t) {
                        facts.t.push((slot, t));
                    }
                }
                (self.is_check(h, t, e.span), facts)
            }
            A::Binary(op @ (AOp::Eq | AOp::Ne), l, r) => {
                let null_side = if matches!(r.kind, A::Null) {
                    Some(l)
                } else if matches!(l.kind, A::Null) {
                    Some(r)
                } else {
                    None
                };
                let h = self.expr(e, None);
                let mut facts = Facts::default();
                if let Some(x) = null_side {
                    if let Some(slot) = self.local_of(x) {
                        let declared = self.declared_of(slot);
                        let u = self.types.unwrap_optional(declared);
                        if u != declared && self.narrowable(declared, u) {
                            if *op == AOp::Ne {
                                facts.t.push((slot, u));
                            } else {
                                facts.f.push((slot, u));
                            }
                        }
                    }
                }
                (h, facts)
            }
            _ => {
                let h = self.expr(e, Some(T_BOOL));
                (self.coerce(h, T_BOOL, e.span), Facts::default())
            }
        }
    }

    pub fn block_stmts(&mut self, stmts: &[ast::Stmt]) -> Vec<Stmt> {
        self.ctx().scopes.push(HashMap::new());
        let mut out = Vec::new();
        let mut dead_warned = false;
        for s in stmts {
            if diverges(&out) && !dead_warned {
                self.warn(s.span, "unreachable code");
                dead_warned = true;
            }
            out.extend(self.stmt(s));
        }
        self.ctx().scopes.pop();
        out
    }

    fn merge(&mut self, a: HashMap<u32, TyId>, b: HashMap<u32, TyId>) -> HashMap<u32, TyId> {
        a.into_iter().filter(|(k, v)| b.get(k) == Some(v)).collect()
    }

    pub fn stmt(&mut self, s: &ast::Stmt) -> Vec<Stmt> {
        match &s.kind {
            S::Var { name, ty, init, is_const } => self.var_decl(name, ty.as_ref(), init.as_ref(), *is_const, s.span),
            S::Expr(e) => {
                let h = self.expr(e, None);
                if let ExprKind::Binary(BinOp::ICmp(Cmp::Eq), ..) | ExprKind::Rt(RtFn::StrEq | RtFn::Eq, _) = h.kind {
                    if let A::Binary(AOp::Eq, l, r) = &e.kind {
                        let between = Span::new(e.span.file, l.span.end as usize, r.span.start as usize);
                        let text = self.src_text(between);
                        let mut d = Diagnostic::warning(e.span, "comparison result is unused");
                        if let Some(i) = text.find("==") {
                            let op = Span::new(e.span.file, between.start as usize + i, between.start as usize + i + 2);
                            d = d.fix("to assign a value, use `=`", op, "=");
                        }
                        self.emit(d);
                    }
                }
                vec![Stmt::Expr(h)]
            }
            S::If { cond, then, els } => {
                let (c, facts) = self.cond(cond);
                let before = self.ctx().narrow.clone();
                self.apply(&facts.t);
                let tb = self.block_stmts(&then.stmts);
                let after_then = std::mem::take(&mut self.ctx().narrow);
                self.ctx().narrow = before.clone();
                self.apply(&facts.f);
                let eb = match els {
                    Some(b) => self.block_stmts(&b.stmts),
                    None => Vec::new(),
                };
                let after_else = std::mem::take(&mut self.ctx().narrow);
                let (td, ed) = (diverges(&tb), diverges(&eb));
                self.ctx().narrow = match (td, ed) {
                    (true, true) => before,
                    (true, false) => after_else,
                    (false, true) => after_then,
                    (false, false) => self.merge(after_then, after_else),
                };
                if let ExprKind::Bool(b) = c.kind {
                    return if b { tb } else { eb };
                }
                vec![Stmt::If(c, tb, eb)]
            }
            S::While { cond, body } => {
                let mut assigned = HashSet::new();
                collect_expr(cond, &mut assigned);
                collect_assigned(&body.stmts, &mut assigned);
                self.forget_assigned(&assigned);
                let (c, facts) = self.cond(cond);
                let before = self.ctx().narrow.clone();
                self.apply(&facts.t);
                self.ctx().loops += 1;
                let b = self.block_stmts(&body.stmts);
                self.ctx().loops -= 1;
                self.ctx().narrow = before;
                let has_brk = has_break(&b);
                if !has_brk {
                    self.apply(&facts.f);
                }
                let cond = if matches!(c.kind, ExprKind::Bool(true)) { None } else { Some(c) };
                vec![Stmt::Loop { cond, body: b, step: vec![] }]
            }
            S::For { init, cond, step, body } => {
                self.ctx().scopes.push(HashMap::new());
                let mut out = Vec::new();
                if let Some(i) = init {
                    out.extend(self.stmt(i));
                }
                let mut assigned = HashSet::new();
                if let Some(c) = cond {
                    collect_expr(c, &mut assigned);
                }
                if let Some(st) = step {
                    collect_expr(st, &mut assigned);
                }
                collect_assigned(&body.stmts, &mut assigned);
                self.forget_assigned(&assigned);
                let before = self.ctx().narrow.clone();
                let (c, facts) = match cond {
                    Some(c) => {
                        let (h, f) = self.cond(c);
                        (Some(h), f)
                    }
                    None => (None, Facts::default()),
                };
                self.apply(&facts.t);
                self.ctx().loops += 1;
                let b = self.block_stmts(&body.stmts);
                self.ctx().loops -= 1;
                let st = step.as_ref().map(|e| vec![Stmt::Expr(self.expr(e, None))]).unwrap_or_default();
                self.ctx().narrow = before;
                self.ctx().scopes.pop();
                out.push(Stmt::Loop { cond: c, body: b, step: st });
                out
            }
            S::ForIn { var, index, iter, body } => self.for_in(var, index.as_ref(), iter, body, s.span),
            S::Return(v) => {
                let ctx_init = self.fx.last().map(|c| c.is_init).unwrap_or(true);
                if ctx_init {
                    self.error(s.span, "`return` can only be used inside a function");
                    return vec![];
                }
                let ret = self.ctx().ret;
                match (ret, v) {
                    (Some(rt), None) => {
                        if rt != T_VOID && rt != T_ERROR {
                            let r = self.show(rt);
                            self.error(s.span, format!("this function must return a value of type {}", r));
                        }
                        vec![Stmt::Return(None)]
                    }
                    (Some(rt), Some(e)) => {
                        if rt == T_VOID {
                            let h = self.expr(e, None);
                            if h.ty != T_VOID && h.ty != T_ERROR {
                                let fi = self.ctx().func as usize;
                                let fname = self.funcs[fi].name.clone();
                                let ts = self.show(h.ty);
                                let fspan = self.funcs[fi].span;
                                let mut d = Diagnostic::error(e.span, format!("`{}` does not declare a return type, but returns {}", fname, ts));
                                match self.params_close(fspan) {
                                    Some(at) => d = d.fix(format!("declare that `{}` returns {}", fname, ts), at, format!(": {}", ts)),
                                    None => d = d.help(format!("add `: {}` after the parameter list", ts)),
                                }
                                self.emit(d);
                            }
                            return vec![Stmt::Expr(h), Stmt::Return(None)];
                        }
                        let h = self.expr_to(e, rt);
                        vec![Stmt::Return(Some(h))]
                    }
                    (None, None) => {
                        self.ctx().returns.push(T_VOID);
                        vec![Stmt::Return(None)]
                    }
                    (None, Some(e)) => {
                        let h = self.expr(e, None);
                        let t = h.ty;
                        self.ctx().returns.push(t);
                        vec![Stmt::Return(Some(h))]
                    }
                }
            }
            S::Break | S::Continue => {
                if self.fx.last().map(|c| c.loops).unwrap_or(0) == 0 {
                    self.error(
                        s.span,
                        format!(
                            "`{}` can only be used inside a loop",
                            if matches!(s.kind, S::Break) { "break" } else { "continue" }
                        ),
                    );
                    return vec![];
                }
                vec![if matches!(s.kind, S::Break) { Stmt::Break } else { Stmt::Continue }]
            }
            S::Block(b) => self.block_stmts(&b.stmts),
            S::Extend { target, func } => self.extend_stmt(target, func),
            S::Destroy { target, args } => self.destroy_stmt(target, args, s.span),
        }
    }

    fn var_decl(&mut self, name: &ast::Ident, ty: Option<&TypeExpr>, init: Option<&ast::Expr>, is_const: bool, span: Span) -> Vec<Stmt> {
        let declared = ty.map(|t| self.resolve_type(t));
        if declared == Some(T_VOID) {
            self.error(name.span, "variables cannot have type void");
        }
        let (value, vty) = match init {
            Some(e) => {
                let h = self.expr(e, declared);
                let vt = h.ty;
                if vt == T_VOID {
                    self.error(e.span, "cannot assign the result of a function that returns nothing");
                }
                let target = match declared {
                    Some(d) => d,
                    None => {
                        if vt == T_NULL {
                            self.error_note(
                                e.span,
                                format!("cannot infer the type of `{}` from `null`", name.name),
                                format!("write `{}? {} = null` or `var {}: T? = null`", "T", name.name, name.name),
                            );
                            T_ERROR
                        } else {
                            vt
                        }
                    }
                };
                (self.coerce(h, target, e.span), vt)
            }
            None => {
                let d = declared.unwrap_or(T_ERROR);
                match self.zero(d) {
                    Some(z) => (z, d),
                    None => {
                        let ts = self.show(d);
                        self.error(name.span, format!("`{}` of type {} must be initialized", name.name, ts));
                        (Self::err_expr(), d)
                    }
                }
            }
        };
        let t = declared.unwrap_or(value.ty);
        let is_global = {
            let c = self.fx.last().unwrap();
            c.is_init && c.scopes.len() == 1
        };
        let ts = self.show(t);
        self.hover(name.span, format!("{} {}: {}", if is_const { "const" } else { "var" }, name.name, ts));
        if is_global {
            let m = self.cur_module();
            let g = match self.mods[m].values.get(&name.name) {
                Some(Entry {
                    sym: ValSym::Global(g),
                    span: gs,
                    ..
                }) if *gs == name.span => Some(*g),
                _ => None,
            };
            if let Some(g) = g {
                self.globals[g as usize].ty = Some(t);
                if self.narrowable(t, vty) {
                    self.ctx().narrow.insert(GLOBAL_KEY + g, vty);
                }
                return vec![Stmt::Expr(Expr::new(ExprKind::SetGlobal(g, Box::new(value)), t))];
            }
            return vec![Stmt::Expr(value)];
        }
        if let Some(prev) = self.ctx().scopes.last().unwrap().get(&name.name).cloned() {
            let (l, _) = self.sm.file(prev.span.file).line_col(prev.span.start as usize);
            self.warn(
                name.span,
                format!("`{}` shadows a variable declared on line {} in the same block", name.name, l),
            );
        }
        let scope = Span {
            file: span.file,
            start: span.start,
            end: u32::MAX,
        };
        let slot = self.declare_local(&name.name, t, name.span, is_const, scope);
        if self.narrowable(t, vty) {
            self.ctx().narrow.insert(slot, vty);
        }
        vec![Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(value)), t))]
    }

    fn for_in(&mut self, var: &ast::Ident, index: Option<&ast::Ident>, iter: &ForIter, body: &ast::Block, span: Span) -> Vec<Stmt> {
        self.ctx().scopes.push(HashMap::new());
        let mut assigned = HashSet::new();
        collect_assigned(&body.stmts, &mut assigned);
        self.forget_assigned(&assigned);
        let before = self.ctx().narrow.clone();
        let scope = Span::new(span.file, span.start as usize, span.end as usize);
        let mut out = Vec::new();
        let result = match iter {
            ForIter::Range(a, b, inclusive) => {
                if let Some(i) = index {
                    self.error(i.span, "a range loop has only one loop variable");
                }
                let ha = self.expr_to(a, T_INT);
                let hb = self.expr_to(b, T_INT);
                let ctr = self.new_local(T_INT);
                let end = self.new_local(T_INT);
                out.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(ctr, Box::new(ha)), T_INT)));
                out.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(end, Box::new(hb)), T_INT)));
                let v = self.declare_local(&var.name, T_INT, var.span, false, scope);
                self.hover(var.span, format!("var {}: int", var.name));
                let cmp = if *inclusive { Cmp::Le } else { Cmp::Lt };
                let cond = Expr::new(
                    ExprKind::Binary(
                        BinOp::ICmp(cmp),
                        Box::new(Expr::new(ExprKind::Local(ctr), T_INT)),
                        Box::new(Expr::new(ExprKind::Local(end), T_INT)),
                    ),
                    T_BOOL,
                );
                self.ctx().loops += 1;
                let mut b = vec![Stmt::Expr(Expr::new(
                    ExprKind::SetLocal(v, Box::new(Expr::new(ExprKind::Local(ctr), T_INT))),
                    T_INT,
                ))];
                b.extend(self.block_stmts(&body.stmts));
                self.ctx().loops -= 1;
                let step = vec![Stmt::Expr(Expr::new(
                    ExprKind::SetLocal(
                        ctr,
                        Box::new(Expr::new(
                            ExprKind::Binary(BinOp::IAdd(u32::MAX), Box::new(Expr::new(ExprKind::Local(ctr), T_INT)), Box::new(Expr::int(1))),
                            T_INT,
                        )),
                    ),
                    T_INT,
                ))];
                out.push(Stmt::Loop {
                    cond: Some(cond),
                    body: b,
                    step,
                });
                out
            }
            ForIter::Expr(e) => {
                let h = self.expr(e, None);
                let t = h.ty;
                let (arr, elem, map) = match self.types.get(t).clone() {
                    Ty::Array(el) => (h, el, None),
                    Ty::Str => (Expr::new(ExprKind::Rt(RtFn::StrChars, vec![h]), T_ARR_STR), T_STR, None),
                    Ty::Map(k, v) => {
                        let at = self.types.array(k);
                        let slot = self.new_local(t);
                        out.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(h)), t)));
                        (
                            Expr::new(ExprKind::Rt(RtFn::MapKeys, vec![Expr::new(ExprKind::Local(slot), t), Self::tid(at)]), at),
                            k,
                            Some((slot, t, v)),
                        )
                    }
                    Ty::Error => (h, T_ERROR, None),
                    Ty::Any => {
                        self.error_note(
                            e.span,
                            "cannot iterate over a value of type any",
                            "cast it first, e.g. `for x in value as [any]`",
                        );
                        (h, T_ERROR, None)
                    }
                    _ => {
                        let s = self.show(t);
                        self.error(e.span, format!("cannot iterate over a value of type {}", s));
                        (h, T_ERROR, None)
                    }
                };
                let at = arr.ty;
                let arr_slot = self.new_local(at);
                let ctr = self.new_local(T_INT);
                out.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(arr_slot, Box::new(arr)), at)));
                out.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(ctr, Box::new(Expr::int(0))), T_INT)));
                let arr_e = Expr::new(ExprKind::Local(arr_slot), at);
                let ctr_e = Expr::new(ExprKind::Local(ctr), T_INT);
                let cond = Expr::new(
                    ExprKind::Binary(
                        BinOp::ICmp(Cmp::Lt),
                        Box::new(ctr_e.clone()),
                        Box::new(Expr::new(ExprKind::ArrLen(Box::new(arr_e.clone())), T_INT)),
                    ),
                    T_BOOL,
                );
                let mut b = Vec::new();
                let l = self.loc(span);
                let item = Expr::new(ExprKind::Index(Box::new(arr_e), Box::new(ctr_e.clone()), l), elem);
                match (index, map) {
                    (Some(first), Some((mslot, mt, vt))) => {
                        let k = self.declare_local(&first.name, elem, first.span, false, scope);
                        b.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(k, Box::new(item)), elem)));
                        let v = self.declare_local(&var.name, vt, var.span, false, scope);
                        let lx = self.loc_expr(span);
                        let get = Expr::new(
                            ExprKind::Rt(
                                RtFn::MapGet,
                                vec![Expr::new(ExprKind::Local(mslot), mt), Expr::new(ExprKind::Local(k), elem), lx],
                            ),
                            vt,
                        );
                        b.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(v, Box::new(get)), vt)));
                        let (ks, vs) = (self.show(elem), self.show(vt));
                        self.hover(first.span, format!("var {}: {}", first.name, ks));
                        self.hover(var.span, format!("var {}: {}", var.name, vs));
                    }
                    (Some(first), None) => {
                        let i = self.declare_local(&first.name, T_INT, first.span, false, scope);
                        b.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(i, Box::new(ctr_e.clone())), T_INT)));
                        let v = self.declare_local(&var.name, elem, var.span, false, scope);
                        b.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(v, Box::new(item)), elem)));
                        let es = self.show(elem);
                        self.hover(first.span, format!("var {}: int", first.name));
                        self.hover(var.span, format!("var {}: {}", var.name, es));
                    }
                    (None, _) => {
                        let v = self.declare_local(&var.name, elem, var.span, false, scope);
                        b.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(v, Box::new(item)), elem)));
                        let es = self.show(elem);
                        self.hover(var.span, format!("var {}: {}", var.name, es));
                    }
                }
                self.ctx().loops += 1;
                b.extend(self.block_stmts(&body.stmts));
                self.ctx().loops -= 1;
                let step = vec![Stmt::Expr(Expr::new(
                    ExprKind::SetLocal(
                        ctr,
                        Box::new(Expr::new(
                            ExprKind::Binary(BinOp::IAdd(u32::MAX), Box::new(ctr_e), Box::new(Expr::int(1))),
                            T_INT,
                        )),
                    ),
                    T_INT,
                ))];
                out.push(Stmt::Loop {
                    cond: Some(cond),
                    body: b,
                    step,
                });
                out
            }
        };
        self.ctx().narrow = before;
        self.ctx().scopes.pop();
        result
    }
}

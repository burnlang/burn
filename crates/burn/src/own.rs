use crate::hir::{Expr, ExprKind, Func, Program, Stmt, UnOp};
use crate::types::*;
use burn_runtime::RtFn;

pub fn lower(p: &Program) -> Program {
    let mut out = p.clone();
    for f in out.funcs.iter_mut() {
        if f.external.is_some() {
            continue;
        }
        Pass::run(&p.types, f);
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Want {
    Owned,
    Borrowed,
    Discard,
}

struct Pass<'a> {
    types: &'a Types,
    locals: Vec<TyId>,
    owned: Vec<bool>,
    scopes: Vec<(Vec<u32>, Vec<u32>)>,
    free_managed: Vec<u32>,
    free_raw: Vec<u32>,
}

pub fn managed(types: &Types, t: TyId) -> bool {
    matches!(
        types.get(t),
        Ty::Str | Ty::Any | Ty::Array(_) | Ty::Map(..) | Ty::Optional(_) | Ty::Func(..) | Ty::Future(_) | Ty::Record(_) | Ty::Interface(_)
    )
}

fn local(s: u32, t: TyId) -> Expr {
    Expr::new(ExprKind::Local(s), t)
}

fn set_local(s: u32, v: Expr) -> Expr {
    let t = v.ty;
    Expr::new(ExprKind::SetLocal(s, Box::new(v)), t)
}

fn release(e: Expr) -> Expr {
    Expr::new(ExprKind::Release(Box::new(e)), T_VOID)
}

fn retain(e: Expr) -> Expr {
    let t = e.ty;
    Expr::new(ExprKind::Retain(Box::new(e)), t)
}

fn seq(stmts: Vec<Stmt>, last: Expr) -> Expr {
    if stmts.is_empty() {
        return last;
    }
    let t = last.ty;
    Expr::new(ExprKind::Seq(stmts, Box::new(last)), t)
}

fn writes_local(e: &Expr, slot: u32) -> bool {
    let mut found = false;
    visit(e, &mut |x| {
        if let ExprKind::SetLocal(s, _) = x.kind {
            if s == slot {
                found = true;
            }
        }
    });
    found
}

fn visit(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::SetLocal(_, x)
        | ExprKind::SetGlobal(_, x)
        | ExprKind::Unary(_, x)
        | ExprKind::Conv(_, x)
        | ExprKind::GetField(x, _)
        | ExprKind::ArrLen(x)
        | ExprKind::BoxVal(x)
        | ExprKind::Retain(x)
        | ExprKind::Release(x) => visit(x, f),
        ExprKind::Binary(_, a, b) | ExprKind::And(a, b) | ExprKind::Or(a, b) | ExprKind::Index(a, b, _) | ExprKind::SetField(a, _, b) => {
            visit(a, f);
            visit(b, f);
        }
        ExprKind::SetIndex(a, b, c, _) => {
            visit(a, f);
            visit(b, f);
            visit(c, f);
        }
        ExprKind::Call(_, xs)
        | ExprKind::CallIface(_, xs)
        | ExprKind::Rt(_, xs)
        | ExprKind::Spawn(_, xs)
        | ExprKind::NewStruct(_, xs)
        | ExprKind::NewArray(_, xs) => xs.iter().for_each(|x| visit(x, f)),
        ExprKind::CallIndirect(c, xs) => {
            visit(c, f);
            xs.iter().for_each(|x| visit(x, f));
        }
        ExprKind::Seq(ss, x) => {
            for s in ss {
                visit_stmt(s, f);
            }
            visit(x, f);
        }
        _ => {}
    }
}

fn visit_stmt(s: &Stmt, f: &mut dyn FnMut(&Expr)) {
    match s {
        Stmt::Expr(e) | Stmt::Return(Some(e)) => visit(e, f),
        Stmt::If(c, a, b) => {
            visit(c, f);
            a.iter().for_each(|x| visit_stmt(x, f));
            b.iter().for_each(|x| visit_stmt(x, f));
        }
        Stmt::Loop { cond, body, step } => {
            if let Some(c) = cond {
                visit(c, f);
            }
            body.iter().for_each(|x| visit_stmt(x, f));
            step.iter().for_each(|x| visit_stmt(x, f));
        }
        _ => {}
    }
}

fn reads(e: &Expr, hit: &dyn Fn(&ExprKind) -> bool) -> bool {
    let mut found = false;
    visit(e, &mut |x| found |= hit(&x.kind));
    found
}

fn concat_base(e: &Expr) -> Option<&Expr> {
    match &e.kind {
        ExprKind::Rt(RtFn::StrConcat, xs) if xs.len() == 2 => Some(concat_base(&xs[0]).unwrap_or(&xs[0])),
        _ => None,
    }
}

fn concat_parts(e: Expr, out: &mut Vec<Expr>) {
    if let ExprKind::Rt(RtFn::StrConcat, xs) = e.kind {
        let mut it = xs.into_iter();
        if let (Some(a), Some(b)) = (it.next(), it.next()) {
            concat_parts(a, out);
            out.push(b);
        }
    }
}

fn concat_refs<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) {
    if let ExprKind::Rt(RtFn::StrConcat, xs) = &e.kind {
        if xs.len() == 2 {
            concat_refs(&xs[0], out);
            out.push(&xs[1]);
        }
    }
}

fn append(a: Expr, b: Expr) -> Expr {
    Expr::new(ExprKind::Rt(RtFn::StrAppend, vec![a, b]), T_STR)
}

fn is_fresh(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Call(..)
        | ExprKind::CallIndirect(..)
        | ExprKind::CallIface(..)
        | ExprKind::Rt(..)
        | ExprKind::Spawn(..)
        | ExprKind::NewStruct(..)
        | ExprKind::NewArray(..) => true,
        ExprKind::Seq(_, x) => is_fresh(x),
        _ => false,
    }
}

fn is_static(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Str(_) | ExprKind::Null)
}

fn diverges(stmts: &[Stmt]) -> bool {
    matches!(stmts.last(), Some(Stmt::Return(_)))
}

impl<'a> Pass<'a> {
    fn run(types: &'a Types, f: &mut Func) {
        let nparams = f.params as usize;
        let mut assigned = vec![false; f.locals.len()];
        for s in &f.body {
            visit_stmt(s, &mut |x| {
                if let ExprKind::SetLocal(slot, _) = x.kind {
                    if (slot as usize) < assigned.len() {
                        assigned[slot as usize] = true;
                    }
                }
            });
        }
        let owned: Vec<bool> = f
            .locals
            .iter()
            .enumerate()
            .map(|(i, t)| managed(types, *t) && (i >= nparams || assigned[i] || f.is_async))
            .collect();
        let mut p = Pass {
            types,
            locals: f.locals.clone(),
            owned,
            scopes: Vec::new(),
            free_managed: Vec::new(),
            free_raw: Vec::new(),
        };
        let body = std::mem::take(&mut f.body);
        let mut out = p.block(body);
        if !diverges(&out) {
            out.extend(p.release_all());
        }
        let mut pre = Vec::new();
        for (i, o) in p.owned.iter().enumerate() {
            if !*o {
                continue;
            }
            if i < nparams {
                if !f.is_async {
                    pre.push(Stmt::Expr(retain(local(i as u32, p.locals[i]))));
                }
            } else {
                let t = p.locals[i];
                pre.push(Stmt::Expr(set_local(i as u32, Expr::new(ExprKind::Null, t))));
            }
        }
        pre.extend(out);
        f.body = pre;
        f.locals = p.locals;
    }

    fn may_write(&self, e: &Expr) -> bool {
        reads(e, &|k| match k {
            ExprKind::Call(..)
            | ExprKind::CallIndirect(..)
            | ExprKind::CallIface(..)
            | ExprKind::Spawn(..)
            | ExprKind::SetField(..)
            | ExprKind::SetGlobal(..)
            | ExprKind::SetIndex(..) => true,
            ExprKind::Rt(_, xs) => xs.iter().any(|x| matches!(self.types.get(x.ty), Ty::Func(..))),
            _ => false,
        })
    }

    fn appendable(&self, v: &Expr, base: &dyn Fn(&ExprKind) -> bool, conflict: &dyn Fn(&ExprKind) -> bool, heap: bool) -> bool {
        if !concat_base(v).is_some_and(|b| base(&b.kind)) {
            return false;
        }
        let mut parts = Vec::new();
        concat_refs(v, &mut parts);
        if parts.len() > 1 && parts.iter().any(|p| reads(p, conflict)) {
            return false;
        }
        !heap || !parts.iter().any(|p| self.may_write(p))
    }

    fn managed(&self, t: TyId) -> bool {
        managed(self.types, t)
    }

    fn new_slot(&mut self, t: TyId, owned: bool) -> u32 {
        self.locals.push(t);
        self.owned.push(owned);
        (self.locals.len() - 1) as u32
    }

    fn temp(&mut self, t: TyId) -> u32 {
        let s = match self.free_managed.pop() {
            Some(s) => {
                self.locals[s as usize] = t;
                s
            }
            None => self.new_slot(t, true),
        };
        if let Some(sc) = self.scopes.last_mut() {
            sc.0.push(s);
        }
        s
    }

    fn raw(&mut self, t: TyId) -> u32 {
        let s = match self.free_raw.pop() {
            Some(s) => {
                self.locals[s as usize] = t;
                s
            }
            None => self.new_slot(t, false),
        };
        if let Some(sc) = self.scopes.last_mut() {
            sc.1.push(s);
        }
        s
    }

    fn push_scope(&mut self) {
        self.scopes.push((Vec::new(), Vec::new()));
    }

    fn pop_scope(&mut self, out: &mut Vec<Stmt>) {
        let (managed, raw) = self.scopes.pop().unwrap_or_default();
        for t in managed {
            let ty = self.locals[t as usize];
            out.push(Stmt::Expr(release(local(t, ty))));
            out.push(Stmt::Expr(set_local(t, Expr::new(ExprKind::Null, ty))));
            self.free_managed.push(t);
        }
        self.free_raw.extend(raw);
    }

    fn scope_has_temps(&self) -> bool {
        self.scopes.last().map(|s| !s.0.is_empty()).unwrap_or(false)
    }

    fn release_all(&self) -> Vec<Stmt> {
        self.owned
            .iter()
            .enumerate()
            .filter(|(_, o)| **o)
            .map(|(i, _)| Stmt::Expr(release(local(i as u32, self.locals[i]))))
            .collect()
    }

    fn block(&mut self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        let mut out = Vec::with_capacity(stmts.len());
        for s in stmts {
            self.stmt(s, &mut out);
        }
        out
    }

    fn stmt(&mut self, s: Stmt, out: &mut Vec<Stmt>) {
        match s {
            Stmt::Expr(e) => {
                self.push_scope();
                self.discard(e, out);
                self.pop_scope(out);
            }
            Stmt::If(c, a, b) => {
                self.push_scope();
                let mut c2 = self.expr(c, Want::Borrowed);
                if self.scope_has_temps() {
                    let tc = self.new_slot(T_BOOL, false);
                    out.push(Stmt::Expr(set_local(tc, c2)));
                    c2 = local(tc, T_BOOL);
                }
                self.pop_scope(out);
                let a2 = self.block(a);
                let b2 = self.block(b);
                out.push(Stmt::If(c2, a2, b2));
            }
            Stmt::Loop { cond, body, step } => {
                let mut prefix = Vec::new();
                let mut cond2 = None;
                if let Some(c) = cond {
                    self.push_scope();
                    let c2 = self.expr(c, Want::Borrowed);
                    if self.scope_has_temps() {
                        let tc = self.new_slot(T_BOOL, false);
                        prefix.push(Stmt::Expr(set_local(tc, c2)));
                        self.pop_scope(&mut prefix);
                        let not = Expr::new(ExprKind::Unary(UnOp::Not, Box::new(local(tc, T_BOOL))), T_BOOL);
                        prefix.push(Stmt::If(not, vec![Stmt::Break], vec![]));
                    } else {
                        self.pop_scope(&mut prefix);
                        cond2 = Some(c2);
                    }
                }
                let mut b2 = prefix;
                b2.extend(self.block(body));
                let s2 = self.block(step);
                out.push(Stmt::Loop {
                    cond: cond2,
                    body: b2,
                    step: s2,
                });
            }
            Stmt::Return(None) => {
                out.extend(self.release_all());
                out.push(Stmt::Return(None));
            }
            Stmt::Return(Some(v)) => {
                let has_owned = self.owned.iter().any(|o| *o);
                if !self.managed(v.ty) {
                    self.push_scope();
                    let v2 = self.expr(v, Want::Borrowed);
                    let t = v2.ty;
                    let tr = self.new_slot(t, false);
                    out.push(Stmt::Expr(set_local(tr, v2)));
                    self.pop_scope(out);
                    out.extend(self.release_all());
                    out.push(Stmt::Return(Some(local(tr, t))));
                    return;
                }
                if let ExprKind::Local(s) = v.kind {
                    if self.owned[s as usize] {
                        let t = v.ty;
                        let tr = self.new_slot(t, false);
                        out.push(Stmt::Expr(set_local(tr, local(s, t))));
                        out.push(Stmt::Expr(set_local(s, Expr::new(ExprKind::Null, t))));
                        out.extend(self.release_all());
                        out.push(Stmt::Return(Some(local(tr, t))));
                        return;
                    }
                }
                self.push_scope();
                let v2 = self.expr(v, Want::Owned);
                if !has_owned && !self.scope_has_temps() {
                    self.pop_scope(out);
                    out.push(Stmt::Return(Some(v2)));
                    return;
                }
                let t = v2.ty;
                let tr = self.new_slot(t, false);
                out.push(Stmt::Expr(set_local(tr, v2)));
                self.pop_scope(out);
                out.extend(self.release_all());
                out.push(Stmt::Return(Some(local(tr, t))));
            }
            Stmt::Break => out.push(Stmt::Break),
            Stmt::Continue => out.push(Stmt::Continue),
        }
    }

    fn discard(&mut self, e: Expr, out: &mut Vec<Stmt>) {
        match e.kind {
            ExprKind::SetLocal(s, v) if self.owned.get(s as usize).copied().unwrap_or(false) => {
                let ty = self.locals[s as usize];
                self.assign_local(s, ty, *v, out);
            }
            ExprKind::SetGlobal(g, v) if self.managed(e.ty) => self.assign_global(g, e.ty, *v, out),
            ExprKind::SetField(o, i, v) if self.managed(v.ty) => {
                self.assign_field(*o, i, *v, out);
            }
            ExprKind::SetIndex(a, i, v, l) if self.managed(v.ty) => {
                self.assign_index(*a, *i, *v, l, out);
            }
            ExprKind::Seq(ss, x) => {
                for s in ss {
                    self.stmt(s, out);
                }
                self.discard(*x, out);
            }
            kind => {
                let e = Expr { kind, ty: e.ty };
                let e2 = self.expr(e, Want::Discard);
                if !matches!(
                    e2.kind,
                    ExprKind::Int(_) | ExprKind::Null | ExprKind::Local(_) | ExprKind::Global(_) | ExprKind::Str(_)
                ) {
                    out.push(Stmt::Expr(e2));
                }
            }
        }
    }

    fn assign_local(&mut self, s: u32, ty: TyId, v: Expr, out: &mut Vec<Stmt>) {
        let base = |k: &ExprKind| matches!(k, ExprKind::Local(x) if *x == s);
        if ty == T_STR && !writes_local(&v, s) && self.appendable(&v, &base, &base, false) {
            let mut parts = Vec::new();
            concat_parts(v, &mut parts);
            for b in parts {
                let b2 = self.expr(b, Want::Borrowed);
                out.push(Stmt::Expr(set_local(s, append(local(s, ty), b2))));
            }
            return;
        }
        let safe = !writes_local(&v, s);
        let old = self.raw(ty);
        let v2 = self.expr(v, Want::Owned);
        if safe {
            out.push(Stmt::Expr(set_local(old, local(s, ty))));
            out.push(Stmt::Expr(set_local(s, v2)));
        } else {
            let tv = self.raw(ty);
            out.push(Stmt::Expr(set_local(tv, v2)));
            out.push(Stmt::Expr(set_local(old, local(s, ty))));
            out.push(Stmt::Expr(set_local(s, local(tv, ty))));
        }
        out.push(Stmt::Expr(release(local(old, ty))));
    }

    fn assign_global(&mut self, g: u32, ty: TyId, v: Expr, out: &mut Vec<Stmt>) {
        let base = |k: &ExprKind| matches!(k, ExprKind::Global(x) if *x == g);
        if ty == T_STR && self.appendable(&v, &base, &base, true) {
            let mut parts = Vec::new();
            concat_parts(v, &mut parts);
            for b in parts {
                let b2 = self.expr(b, Want::Borrowed);
                let cur = Expr::new(ExprKind::Global(g), ty);
                out.push(Stmt::Expr(Expr::new(ExprKind::SetGlobal(g, Box::new(append(cur, b2))), ty)));
            }
            return;
        }
        let effects = v.has_side_effects();
        let old = self.raw(ty);
        let v2 = self.expr(v, Want::Owned);
        let gl = Expr::new(ExprKind::Global(g), ty);
        if effects {
            let tv = self.raw(ty);
            out.push(Stmt::Expr(set_local(tv, v2)));
            out.push(Stmt::Expr(set_local(old, gl)));
            out.push(Stmt::Expr(Expr::new(ExprKind::SetGlobal(g, Box::new(local(tv, ty))), ty)));
        } else {
            out.push(Stmt::Expr(set_local(old, gl)));
            out.push(Stmt::Expr(Expr::new(ExprKind::SetGlobal(g, Box::new(v2)), ty)));
        }
        out.push(Stmt::Expr(release(local(old, ty))));
    }

    fn stash(&mut self, e: Expr, out: &mut Vec<Stmt>) -> Expr {
        if matches!(e.kind, ExprKind::Local(_) | ExprKind::Int(_) | ExprKind::Null | ExprKind::Str(_)) {
            return e;
        }
        let t = e.ty;
        let s = self.raw(t);
        out.push(Stmt::Expr(set_local(s, e)));
        local(s, t)
    }

    fn assign_field(&mut self, o: Expr, i: u32, v: Expr, out: &mut Vec<Stmt>) -> Expr {
        let ty = v.ty;
        if let (ExprKind::Local(slot), true) = (&o.kind, ty == T_STR) {
            let slot = *slot;
            let field = |k: &ExprKind| matches!(k, ExprKind::GetField(x, j) if *j == i && matches!(x.kind, ExprKind::Local(y) if y == slot));
            let any = |k: &ExprKind| matches!(k, ExprKind::GetField(_, j) if *j == i);
            if !writes_local(&v, slot) && self.appendable(&v, &field, &any, true) {
                let obj = self.expr(o, Want::Borrowed);
                let mut parts = Vec::new();
                concat_parts(v, &mut parts);
                for b in parts {
                    let b2 = self.expr(b, Want::Borrowed);
                    let cur = Expr::new(ExprKind::GetField(Box::new(obj.clone()), i), ty);
                    out.push(Stmt::Expr(Expr::new(
                        ExprKind::SetField(Box::new(obj.clone()), i, Box::new(append(cur, b2))),
                        ty,
                    )));
                }
                return Expr::new(ExprKind::GetField(Box::new(obj), i), ty);
            }
        }
        let effects = v.has_side_effects();
        let o2 = self.expr(o, Want::Borrowed);
        let obj = self.stash(o2, out);
        let v2 = self.expr(v, Want::Owned);
        let val = if effects { self.stash(v2, out) } else { v2 };
        let old = self.raw(ty);
        let get = Expr::new(ExprKind::GetField(Box::new(obj.clone()), i), ty);
        out.push(Stmt::Expr(set_local(old, get)));
        out.push(Stmt::Expr(Expr::new(ExprKind::SetField(Box::new(obj.clone()), i, Box::new(val)), ty)));
        out.push(Stmt::Expr(release(local(old, ty))));
        Expr::new(ExprKind::GetField(Box::new(obj), i), ty)
    }

    fn assign_index(&mut self, a: Expr, i: Expr, v: Expr, l: u32, out: &mut Vec<Stmt>) -> Expr {
        let ty = v.ty;
        let effects = v.has_side_effects();
        let a2 = self.expr(a, Want::Borrowed);
        let arr = self.stash(a2, out);
        let i2 = self.expr(i, Want::Borrowed);
        let idx = self.stash(i2, out);
        let v2 = self.expr(v, Want::Owned);
        let val = if effects { self.stash(v2, out) } else { v2 };
        let old = self.raw(ty);
        let get = Expr::new(ExprKind::Index(Box::new(arr.clone()), Box::new(idx.clone()), l), ty);
        out.push(Stmt::Expr(set_local(old, get)));
        out.push(Stmt::Expr(Expr::new(
            ExprKind::SetIndex(Box::new(arr.clone()), Box::new(idx.clone()), Box::new(val), l),
            ty,
        )));
        out.push(Stmt::Expr(release(local(old, ty))));
        Expr::new(ExprKind::Index(Box::new(arr), Box::new(idx), l), ty)
    }

    fn fresh(&mut self, e: Expr, want: Want) -> Expr {
        if !self.managed(e.ty) {
            return e;
        }
        match want {
            Want::Owned => e,
            Want::Discard => release(e),
            Want::Borrowed => {
                let t = e.ty;
                let s = self.temp(t);
                set_local(s, e)
            }
        }
    }

    fn borrowed(&mut self, e: Expr, want: Want) -> Expr {
        if want == Want::Owned && self.managed(e.ty) {
            retain(e)
        } else {
            e
        }
    }

    fn args(&mut self, xs: Vec<Expr>, user: bool) -> Vec<Expr> {
        let n = xs.len();
        let effects: Vec<bool> = (0..n).map(|i| xs[i + 1..].iter().any(|x| x.has_side_effects())).collect();
        let mut out = Vec::with_capacity(n);
        for (i, x) in xs.into_iter().enumerate() {
            let stable = !self.managed(x.ty) || is_fresh(&x) || is_static(&x) || matches!(x.kind, ExprKind::Local(_));
            if stable || (!user && !effects[i]) {
                out.push(self.expr(x, Want::Borrowed));
            } else {
                let t = x.ty;
                let x2 = self.expr(x, Want::Owned);
                let s = self.temp(t);
                out.push(set_local(s, x2));
            }
        }
        out
    }

    fn expr(&mut self, e: Expr, want: Want) -> Expr {
        let ty = e.ty;
        match e.kind {
            ExprKind::Int(_)
            | ExprKind::TypeId(_)
            | ExprKind::LocId(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Str(_)
            | ExprKind::Null
            | ExprKind::FuncRef(_) => Expr { kind: e.kind, ty },
            ExprKind::Local(_) | ExprKind::Global(_) => self.borrowed(Expr { kind: e.kind, ty }, want),
            ExprKind::SetLocal(s, v) => {
                if self.owned.get(s as usize).copied().unwrap_or(false) {
                    let st = self.locals[s as usize];
                    let mut out = Vec::new();
                    self.assign_local(s, st, *v, &mut out);
                    let r = seq(out, local(s, st));
                    return self.borrowed(r, want);
                }
                let v2 = self.expr(*v, Want::Borrowed);
                Expr::new(ExprKind::SetLocal(s, Box::new(v2)), ty)
            }
            ExprKind::SetGlobal(g, v) => {
                if self.managed(ty) {
                    let mut out = Vec::new();
                    self.assign_global(g, ty, *v, &mut out);
                    let r = seq(out, Expr::new(ExprKind::Global(g), ty));
                    return self.borrowed(r, want);
                }
                let v2 = self.expr(*v, Want::Borrowed);
                Expr::new(ExprKind::SetGlobal(g, Box::new(v2)), ty)
            }
            ExprKind::SetField(o, i, v) => {
                if self.managed(v.ty) {
                    let mut out = Vec::new();
                    let r = self.assign_field(*o, i, *v, &mut out);
                    let r = seq(out, r);
                    return self.borrowed(r, want);
                }
                let o2 = self.expr(*o, Want::Borrowed);
                let v2 = self.expr(*v, Want::Borrowed);
                Expr::new(ExprKind::SetField(Box::new(o2), i, Box::new(v2)), ty)
            }
            ExprKind::SetIndex(a, i, v, l) => {
                if self.managed(v.ty) {
                    let mut out = Vec::new();
                    let r = self.assign_index(*a, *i, *v, l, &mut out);
                    let r = seq(out, r);
                    return self.borrowed(r, want);
                }
                let a2 = self.expr(*a, Want::Borrowed);
                let i2 = self.expr(*i, Want::Borrowed);
                let v2 = self.expr(*v, Want::Borrowed);
                Expr::new(ExprKind::SetIndex(Box::new(a2), Box::new(i2), Box::new(v2), l), ty)
            }
            ExprKind::Unary(op, x) => Expr::new(ExprKind::Unary(op, Box::new(self.expr(*x, Want::Borrowed))), ty),
            ExprKind::Conv(c, x) => Expr::new(ExprKind::Conv(c, Box::new(self.expr(*x, Want::Borrowed))), ty),
            ExprKind::Binary(op, a, b) => {
                let a2 = self.expr(*a, Want::Borrowed);
                let b2 = self.expr(*b, Want::Borrowed);
                Expr::new(ExprKind::Binary(op, Box::new(a2), Box::new(b2)), ty)
            }
            ExprKind::And(a, b) => {
                let a2 = self.expr(*a, Want::Borrowed);
                let b2 = self.expr(*b, Want::Borrowed);
                Expr::new(ExprKind::And(Box::new(a2), Box::new(b2)), ty)
            }
            ExprKind::Or(a, b) => {
                let a2 = self.expr(*a, Want::Borrowed);
                let b2 = self.expr(*b, Want::Borrowed);
                Expr::new(ExprKind::Or(Box::new(a2), Box::new(b2)), ty)
            }
            ExprKind::ArrLen(a) => Expr::new(ExprKind::ArrLen(Box::new(self.expr(*a, Want::Borrowed))), ty),
            ExprKind::Call(f, xs) => {
                let xs = self.args(xs, true);
                self.fresh(Expr::new(ExprKind::Call(f, xs), ty), want)
            }
            ExprKind::CallIface(slot, xs) => {
                let xs = self.args(xs, true);
                self.fresh(Expr::new(ExprKind::CallIface(slot, xs), ty), want)
            }
            ExprKind::CallIndirect(c, xs) => {
                let xs = self.args(xs, true);
                let c2 = self.expr(*c, Want::Borrowed);
                self.fresh(Expr::new(ExprKind::CallIndirect(Box::new(c2), xs), ty), want)
            }
            ExprKind::Rt(f, xs) => {
                let xs = self.args(xs, false);
                self.fresh(Expr::new(ExprKind::Rt(f, xs), ty), want)
            }
            ExprKind::Spawn(f, xs) => {
                let xs: Vec<Expr> = xs.into_iter().map(|x| self.expr(x, Want::Owned)).collect();
                self.fresh(Expr::new(ExprKind::Spawn(f, xs), ty), want)
            }
            ExprKind::NewStruct(t, xs) => {
                let xs: Vec<Expr> = xs.into_iter().map(|x| self.expr(x, Want::Owned)).collect();
                self.fresh(Expr::new(ExprKind::NewStruct(t, xs), ty), want)
            }
            ExprKind::NewArray(t, xs) => {
                let xs: Vec<Expr> = xs.into_iter().map(|x| self.expr(x, Want::Owned)).collect();
                self.fresh(Expr::new(ExprKind::NewArray(t, xs), ty), want)
            }
            ExprKind::GetField(o, i) => {
                let o2 = self.expr(*o, Want::Borrowed);
                self.borrowed(Expr::new(ExprKind::GetField(Box::new(o2), i), ty), want)
            }
            ExprKind::Index(a, i, l) => {
                let a2 = self.expr(*a, Want::Borrowed);
                let i2 = self.expr(*i, Want::Borrowed);
                self.borrowed(Expr::new(ExprKind::Index(Box::new(a2), Box::new(i2), l), ty), want)
            }
            ExprKind::BoxVal(x) => {
                let x2 = self.expr(*x, Want::Borrowed);
                self.borrowed(Expr::new(ExprKind::BoxVal(Box::new(x2)), ty), want)
            }
            ExprKind::Seq(ss, x) => {
                let mut out = Vec::new();
                for s in ss {
                    self.stmt(s, &mut out);
                }
                let x2 = self.expr(*x, want);
                seq(out, x2)
            }
            ExprKind::Retain(_) | ExprKind::Release(_) => Expr { kind: e.kind, ty },
        }
    }
}

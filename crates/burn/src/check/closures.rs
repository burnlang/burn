use super::*;
use crate::ast::{ArmBody, ExprKind as A, Pattern, StmtKind as S, TplExpr};

#[derive(Default)]
struct Names {
    in_lambda: HashSet<String>,
    assigned: HashSet<String>,
}

fn walk_block(b: &[ast::Stmt], depth: u32, n: &mut Names) {
    for s in b {
        walk_stmt(s, depth, n);
    }
}

fn walk_stmt(s: &ast::Stmt, depth: u32, n: &mut Names) {
    match &s.kind {
        S::Var { init: Some(e), .. } => walk(e, depth, n),
        S::Expr(e) => walk(e, depth, n),
        S::If { cond, then, els } => {
            walk(cond, depth, n);
            walk_block(&then.stmts, depth, n);
            if let Some(b) = els {
                walk_block(&b.stmts, depth, n);
            }
        }
        S::While { cond, body } => {
            walk(cond, depth, n);
            walk_block(&body.stmts, depth, n);
        }
        S::For { init, cond, step, body } => {
            if let Some(i) = init {
                walk_stmt(i, depth, n);
            }
            if let Some(c) = cond {
                walk(c, depth, n);
            }
            if let Some(st) = step {
                walk(st, depth, n);
            }
            walk_block(&body.stmts, depth, n);
        }
        S::ForIn { iter, body, .. } => {
            match iter {
                ast::ForIter::Range(a, b, _) => {
                    walk(a, depth, n);
                    walk(b, depth, n);
                }
                ast::ForIter::Expr(e) => walk(e, depth, n),
            }
            walk_block(&body.stmts, depth, n);
        }
        S::Return(Some(e)) => walk(e, depth, n),
        S::Block(b) => walk_block(&b.stmts, depth, n),
        S::Extend { func, .. } => walk_block(&func.body.stmts, depth + 1, n),
        S::Destroy { target, args } => {
            n.assigned.insert(target.name.clone());
            if depth > 0 {
                n.in_lambda.insert(target.name.clone());
            }
            for a in args {
                walk(a, depth, n);
            }
        }
        _ => {}
    }
}

fn walk(e: &ast::Expr, depth: u32, n: &mut Names) {
    match &e.kind {
        A::Ident(name) => {
            if depth > 0 {
                n.in_lambda.insert(name.clone());
            }
        }
        A::Template(parts) => {
            for p in parts {
                if let TplExpr::Expr(x) = p {
                    walk(x, depth, n);
                }
            }
        }
        A::Unary(_, x) | A::Await(x) | A::NotNull(x) | A::Is(x, _) | A::As(x, _) | A::SafeAs(x, _) => walk(x, depth, n),
        A::Binary(_, a, b) | A::Coalesce(a, b) | A::Index { obj: a, index: b } => {
            walk(a, depth, n);
            walk(b, depth, n);
        }
        A::Assign { target, value, .. } => {
            if let A::Ident(name) = &target.kind {
                n.assigned.insert(name.clone());
            }
            walk(target, depth, n);
            walk(value, depth, n);
        }
        A::Call { callee, args } => {
            walk(callee, depth, n);
            args.iter().for_each(|a| walk(a, depth, n));
        }
        A::Field { obj, .. } => walk(obj, depth, n),
        A::SafeGet { obj, args, .. } => {
            walk(obj, depth, n);
            for a in args.iter().flatten() {
                walk(a, depth, n);
            }
        }
        A::Array(xs) => xs.iter().for_each(|x| walk(x, depth, n)),
        A::StructLit { fields, .. } => fields.iter().for_each(|(_, x)| walk(x, depth, n)),
        A::MapLit(ps) => ps.iter().for_each(|(a, b)| {
            walk(a, depth, n);
            walk(b, depth, n)
        }),
        A::Lambda(f) => walk_block(&f.body.stmts, depth + 1, n),
        A::New { args, .. } => args.iter().for_each(|a| walk(a, depth, n)),
        A::Match { subject, arms } => {
            if let Some(x) = subject {
                walk(x, depth, n);
            }
            for arm in arms {
                for p in &arm.patterns {
                    match p {
                        Pattern::Value(x) => walk(x, depth, n),
                        Pattern::Range(a, b, _) => {
                            walk(a, depth, n);
                            walk(b, depth, n);
                        }
                        Pattern::Is(..) => {}
                    }
                }
                if let Some(g) = &arm.guard {
                    walk(g, depth, n);
                }
                match &arm.body {
                    ArmBody::Expr(x) => walk(x, depth, n),
                    ArmBody::Block(b) => walk_block(&b.stmts, depth, n),
                }
            }
        }
        _ => {}
    }
}

pub fn cell_names(body: &[ast::Stmt]) -> HashSet<String> {
    let mut n = Names::default();
    walk_block(body, 0, &mut n);
    if n.in_lambda.is_empty() {
        return HashSet::new();
    }
    n.in_lambda.intersection(&n.assigned).cloned().collect()
}

fn rewrite_stmts(ss: &mut [Stmt], cells: &HashMap<u32, TyId>, locals: &[TyId]) {
    for s in ss {
        match s {
            Stmt::Expr(e) | Stmt::Return(Some(e)) => rewrite(e, cells, locals),
            Stmt::If(c, a, b) => {
                rewrite(c, cells, locals);
                rewrite_stmts(a, cells, locals);
                rewrite_stmts(b, cells, locals);
            }
            Stmt::Loop { cond, body, step } => {
                if let Some(c) = cond {
                    rewrite(c, cells, locals);
                }
                rewrite_stmts(body, cells, locals);
                rewrite_stmts(step, cells, locals);
            }
            _ => {}
        }
    }
}

fn rewrite(e: &mut Expr, cells: &HashMap<u32, TyId>, locals: &[TyId]) {
    match &mut e.kind {
        ExprKind::Local(s) => {
            if let Some(ct) = cells.get(s) {
                if e.ty != *ct {
                    let slot = *s;
                    let t = e.ty;
                    *e = Expr::new(ExprKind::GetField(Box::new(Expr::new(ExprKind::Local(slot), *ct)), 0), t);
                }
            }
        }
        ExprKind::SetLocal(s, v) => {
            let slot = *s;
            if let Some(ct) = cells.get(&slot).copied() {
                if let ExprKind::NewStruct(t, xs) = &mut v.kind {
                    if *t == ct {
                        for x in xs.iter_mut() {
                            if !matches!(x.kind, ExprKind::Local(p) if p == slot) {
                                rewrite(x, cells, locals);
                            }
                        }
                        e.ty = ct;
                        return;
                    }
                }
                rewrite(v, cells, locals);
                let v = std::mem::replace(v, Box::new(Checker::err_expr()));
                let t = e.ty;
                *e = Expr::new(ExprKind::SetField(Box::new(Expr::new(ExprKind::Local(slot), ct)), 0, v), t);
                return;
            }
            rewrite(v, cells, locals);
        }
        ExprKind::SetGlobal(_, x) | ExprKind::Unary(_, x) | ExprKind::Conv(_, x) | ExprKind::GetField(x, _) | ExprKind::ArrLen(x) | ExprKind::BoxVal(x) => {
            rewrite(x, cells, locals)
        }
        ExprKind::Binary(_, a, b) | ExprKind::And(a, b) | ExprKind::Or(a, b) | ExprKind::Index(a, b, _) | ExprKind::SetField(a, _, b) => {
            rewrite(a, cells, locals);
            rewrite(b, cells, locals);
        }
        ExprKind::SetIndex(a, b, c, _) => {
            rewrite(a, cells, locals);
            rewrite(b, cells, locals);
            rewrite(c, cells, locals);
        }
        ExprKind::Call(_, xs)
        | ExprKind::CallIface(_, xs)
        | ExprKind::Rt(_, xs)
        | ExprKind::Spawn(_, xs)
        | ExprKind::NewStruct(_, xs)
        | ExprKind::NewArray(_, xs) => xs.iter_mut().for_each(|x| rewrite(x, cells, locals)),
        ExprKind::CallIndirect(c, xs) => {
            rewrite(c, cells, locals);
            xs.iter_mut().for_each(|x| rewrite(x, cells, locals));
        }
        ExprKind::Seq(ss, x) => {
            rewrite_stmts(ss, cells, locals);
            rewrite(x, cells, locals);
        }
        _ => {}
    }
}

pub struct ClosureInfo {
    pub ty: TyId,
    pub captures: Vec<(u32, bool, TyId)>,
}

impl<'a> Checker<'a> {
    pub fn cell_type(&mut self, t: TyId) -> TyId {
        if let Some(c) = self.cell_types.get(&t) {
            return *c;
        }
        let ri = self.types.new_record(RecordDef {
            name: "<cell>".into(),
            fields: vec![FieldDef {
                name: "<value>".into(),
                ty: t,
                default: None,
                private: false,
                span: Span::default(),
            }],
            ..Default::default()
        });
        let ct = self.types.records[ri as usize].ty;
        self.cell_types.insert(t, ct);
        ct
    }

    pub fn closure_record(&mut self, fields: Vec<TyId>) -> TyId {
        let mut fs = vec![FieldDef {
            name: "<fn>".into(),
            ty: T_INT,
            default: None,
            private: false,
            span: Span::default(),
        }];
        for (i, t) in fields.into_iter().enumerate() {
            fs.push(FieldDef {
                name: format!("<{}>", i),
                ty: t,
                default: None,
                private: false,
                span: Span::default(),
            });
        }
        let ri = self.types.new_record(RecordDef {
            name: "<closure>".into(),
            fields: fs,
            ..Default::default()
        });
        self.types.records[ri as usize].ty
    }

    pub fn make_cell_local(&mut self, slot: u32) -> TyId {
        let t = self.ctx().locals[slot as usize];
        let ct = self.cell_type(t);
        self.ctx().cells.insert(slot, ct);
        ct
    }

    pub fn finish_cells(&mut self, body: &mut [Stmt], locals: &mut [TyId], cells: &HashMap<u32, TyId>) {
        if cells.is_empty() {
            return;
        }
        rewrite_stmts(body, cells, locals);
        for (s, ct) in cells {
            locals[*s as usize] = *ct;
        }
    }

    pub fn capture(&mut self, name: &str) -> Option<LocalSym> {
        let n = self.fx.len();
        if n < 2 || !self.fx[n - 1].lambda {
            return None;
        }
        self.capture_at(n - 1, name)
    }

    fn capture_at(&mut self, depth: usize, name: &str) -> Option<LocalSym> {
        if depth == 0 {
            return None;
        }
        let parent = depth - 1;
        let found = self.fx[parent].scopes.iter().rev().find_map(|s| s.get(name).cloned());
        let found = match found {
            Some(l) => {
                if self.fx[parent].is_init && self.fx[parent].scopes.len() == 1 {
                    return None;
                }
                Some(l)
            }
            None if self.fx[parent].lambda => self.capture_at(parent, name),
            None => None,
        };
        let outer = found?;
        if self.fx[parent].is_init && !self.fx[parent].scopes.iter().skip(1).any(|s| s.get(name).map(|l| l.slot) == Some(outer.slot)) {
            return None;
        }
        let ty = self.fx[parent].locals[outer.slot as usize];
        let cell = self.fx[parent].cells.get(&outer.slot).copied();
        let narrowed = if cell.is_none() {
            self.fx[parent].narrow.get(&outer.slot).copied()
        } else {
            None
        };
        let c = &mut self.fx[depth];
        c.locals.push(ty);
        let slot = (c.locals.len() - 1) as u32;
        if let Some(ct) = cell {
            c.cells.insert(slot, ct);
        }
        if let Some(nt) = narrowed {
            c.narrow.insert(slot, nt);
        }
        c.captures.push((outer.slot, slot, cell.is_some()));
        let sym = LocalSym {
            slot,
            is_const: outer.is_const,
            span: outer.span,
        };
        c.scopes[0].insert(name.to_string(), sym.clone());
        Some(sym)
    }

    pub fn closure_value(&mut self, fid: FuncId, t: TyId) -> Expr {
        let info = match self.closures.get(&fid) {
            Some(i) => i,
            None => return Expr::new(ExprKind::FuncRef(fid), t),
        };
        let cty = info.ty;
        let caps = info.captures.clone();
        let mut fields = vec![Expr::new(ExprKind::FuncRef(fid), T_INT)];
        for (ps, _, ty) in caps {
            fields.push(Expr::new(ExprKind::Local(ps), ty));
        }
        Expr::new(ExprKind::NewStruct(cty, fields), t)
    }

    pub fn func_value(&mut self, f: FuncId) -> Expr {
        let t = self.func_type(f);
        let adapter = match self.adapters.get(&f) {
            Some(a) => *a,
            None => {
                let info = &self.funcs[f as usize];
                let params = info.params.clone();
                let ret = if info.is_async {
                    self.types.future(self.funcs[f as usize].ret.unwrap_or(T_VOID))
                } else {
                    info.ret.unwrap_or(T_VOID)
                };
                let name = format!("<ref {}>", info.name);
                let span = info.span;
                let module = info.module;
                let cty = self.closure_record(vec![]);
                let n = params.len() as u32;
                let args: Vec<Expr> = params.iter().enumerate().map(|(i, p)| Expr::new(ExprKind::Local(i as u32), p.1)).collect();
                let call = Expr::new(ExprKind::Call(f, args), ret);
                let body = if ret == T_VOID {
                    vec![Stmt::Expr(call), Stmt::Return(None)]
                } else {
                    vec![Stmt::Return(Some(call))]
                };
                let mut locals: Vec<TyId> = params.iter().map(|p| p.1).collect();
                locals.push(cty);
                let fid = self.funcs.len() as FuncId;
                self.funcs.push(FuncInfo {
                    name: name.clone(),
                    params: params.clone(),
                    ret: Some(ret),
                    is_async: false,
                    decl: None,
                    module,
                    self_ty: None,
                    state: FnState::Done,
                    hir: Some(hir::Func {
                        name,
                        params: n + 1,
                        locals,
                        ret,
                        body,
                        is_async: false,
                        span,
                        end_loc: 0,
                        annotations: Vec::new(),
                        external: None,
                    }),
                    span,
                    private: true,
                    is_static: true,
                    annotations: Vec::new(),
                    deprecated: None,
                    external: None,
                    tenv: None,
                });
                self.closures.insert(fid, ClosureInfo { ty: cty, captures: Vec::new() });
                self.adapters.insert(f, fid);
                fid
            }
        };
        self.closure_value(adapter, t)
    }

    pub fn call_value(&mut self, callee: Expr, mut args: Vec<Expr>, ret: TyId) -> Expr {
        let ct = callee.ty;
        let (pre, env) = match callee.kind {
            ExprKind::Local(_) => (None, callee),
            _ => {
                let slot = self.new_local(ct);
                (
                    Some(Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(callee)), ct))),
                    Expr::new(ExprKind::Local(slot), ct),
                )
            }
        };
        let fnv = Expr::new(ExprKind::GetField(Box::new(env.clone()), 0), T_INT);
        args.push(env);
        let call = Expr::new(ExprKind::CallIndirect(Box::new(fnv), args), ret);
        match pre {
            Some(p) => Expr::new(ExprKind::Seq(vec![p], Box::new(call)), ret),
            None => call,
        }
    }
}

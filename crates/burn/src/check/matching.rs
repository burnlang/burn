use super::stmt::{diverges, Facts};
use super::*;
use crate::ast::{ArmBody, BinOp as AOp, ExprKind as A, MatchArm, Pattern};

type Arm = (Option<Expr>, Vec<Stmt>, Option<(Expr, Span)>);
type Bind = (ast::Ident, TyId, Option<Expr>);

struct Prepared {
    pre: Vec<Stmt>,
    arms: Vec<Arm>,
    exhaustive: bool,
    missing: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Stmt,
    Value,
}

#[derive(PartialEq, Clone)]
enum Key {
    Int(i64),
    Str(String),
    Bool(bool),
    Null,
}

impl<'a> Checker<'a> {
    pub fn match_stmt(&mut self, subject: Option<&ast::Expr>, arms: &[MatchArm], span: Span) -> Vec<Stmt> {
        let p = self.prepare_match(subject, arms, None, Mode::Stmt);
        if !p.exhaustive && !p.missing.is_empty() {
            let list = p.missing.join(", ");
            self.emit(
                Diagnostic::warning(span, format!("`match` does not handle {}", list)).help("add an arm for each, or an `else` arm if nothing should happen"),
            );
        }
        let mut chain: Vec<Stmt> = Vec::new();
        for (cond, body, _) in p.arms.into_iter().rev() {
            chain = match cond {
                None => body,
                Some(c) => vec![Stmt::If(c, body, chain)],
            };
        }
        let mut out = p.pre;
        out.extend(chain);
        out
    }

    pub fn match_value(&mut self, subject: Option<&ast::Expr>, arms: &[MatchArm], span: Span, expected: Option<TyId>) -> Expr {
        let p = self.prepare_match(subject, arms, expected, Mode::Value);
        if !p.exhaustive {
            let mut d = Diagnostic::error(span, "this `match` produces a value, so it must handle every case");
            d = if p.missing.is_empty() {
                d.help("add an `else` arm")
            } else {
                d.help(format!("add arms for {}, or an `else` arm", p.missing.join(", ")))
            };
            self.emit(d);
            return Self::err_expr();
        }
        let values: Vec<(TyId, Span)> = p.arms.iter().filter_map(|a| a.2.as_ref().map(|(v, s)| (v.ty, *s))).collect();
        let ty = match expected.filter(|t| *t != T_VOID && *t != T_ERROR) {
            Some(t) => t,
            None => self.join_arm_types(&values),
        };
        if ty == T_ERROR {
            return Self::err_expr();
        }
        let slot = self.new_local(ty);
        let mut chain: Vec<Stmt> = Vec::new();
        for (cond, mut body, value) in p.arms.into_iter().rev() {
            if let Some((v, vspan)) = value {
                let v = self.coerce(v, ty, vspan);
                body.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(v)), ty)));
            }
            chain = match cond {
                None => body,
                Some(c) => vec![Stmt::If(c, body, chain)],
            };
        }
        let mut out = p.pre;
        out.extend(chain);
        Expr::new(ExprKind::Seq(out, Box::new(Expr::new(ExprKind::Local(slot), ty))), ty)
    }

    fn join_arm_types(&mut self, values: &[(TyId, Span)]) -> TyId {
        let mut cur: Option<TyId> = None;
        let mut nullable = false;
        for (t, s) in values {
            let t = *t;
            if t == T_ERROR {
                return T_ERROR;
            }
            if t == T_NULL {
                nullable = true;
                continue;
            }
            cur = Some(match cur {
                None => t,
                Some(c) if c == t => c,
                Some(c) if (c == T_INT && t == T_FLOAT) || (c == T_FLOAT && t == T_INT) => T_FLOAT,
                Some(c) if self.types.unwrap_optional(c) == t => c,
                Some(c) if self.types.unwrap_optional(t) == c => t,
                Some(c) => match self.common_type(c, t) {
                    Some(x) => x,
                    None => {
                        let (a, b) = (self.show(c), self.show(t));
                        self.emit(
                            Diagnostic::error(*s, format!("this arm produces {} but an earlier arm produces {}", b, a))
                                .help("every arm of a `match` that produces a value must produce the same type"),
                        );
                        return T_ERROR;
                    }
                },
            });
        }
        match cur {
            None if nullable => {
                if let Some((_, s)) = values.first() {
                    self.error(*s, "cannot tell the type of this `match`: every arm produces `null`");
                }
                T_ERROR
            }
            None => T_VOID,
            Some(t) if nullable => self.types.optional(t),
            Some(t) => t,
        }
    }

    fn prepare_match(&mut self, subject: Option<&ast::Expr>, arms: &[MatchArm], expected: Option<TyId>, mode: Mode) -> Prepared {
        let mut pre = Vec::new();
        let mut subj: Option<(Expr, Option<u32>, Option<u32>)> = None;
        if let Some(x) = subject {
            let raw = self.expr_raw(x);
            let key = self.local_of(x);
            let (val, tmp) = match raw.kind {
                ExprKind::Local(s) if key == Some(s) => (raw, Some(s)),
                ExprKind::Global(_) => (raw, None),
                _ => {
                    let t = raw.ty;
                    let s = self.new_local(t);
                    pre.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(s, Box::new(raw)), t)));
                    (Expr::new(ExprKind::Local(s), t), Some(s))
                }
            };
            subj = Some((val, key.or(tmp), tmp));
        }
        let sty = subj.as_ref().map(|s| s.0.ty).unwrap_or(T_BOOL);
        let base = self.types.unwrap_optional(sty);
        let enum_variants: Option<(String, Vec<String>)> = match self.types.get(base).clone() {
            Ty::Enum(ei) => {
                let e = &self.types.enums[ei as usize];
                Some((e.name.clone(), e.variants.iter().map(|v| v.0.clone()).collect()))
            }
            Ty::Interface(_) => self
                .types
                .variants_of(base)
                .map(|e| (e.name.clone(), e.variants.iter().map(|v| v.name.clone()).collect())),
            _ => None,
        };
        let before = self.ctx().narrow.clone();
        let mut ends: Vec<HashMap<u32, TyId>> = Vec::new();
        let mut out = Vec::new();
        let mut seen: Vec<(Key, Span)> = Vec::new();
        let mut has_else = false;
        let mut else_span: Option<Span> = None;
        for arm in arms {
            if let Some(es) = else_span {
                let (l, _) = self.sm.file(es.file).line_col(es.start as usize);
                self.emit(Diagnostic::warning(
                    arm.span,
                    format!("unreachable arm: the `else` on line {} already handles everything", l),
                ));
            }
            let start = self.ctx().narrow.clone();
            self.ctx().scopes.push(HashMap::new());
            let mut conds: Vec<Expr> = Vec::new();
            let mut facts = Facts::default();
            let single = arm.patterns.len() == 1;
            let mut binding: Vec<Bind> = Vec::new();
            for pat in &arm.patterns {
                let (c, f) = match &subj {
                    None => match pat {
                        Pattern::Value(e) => self.cond(e),
                        _ => unreachable!(),
                    },
                    Some((val, key, tmp)) => {
                        let (c, f, k, bind) = self.pattern_cond(pat, val.clone(), *key, *tmp, enum_variants.as_ref());
                        if let Some(k) = k {
                            if arm.guard.is_none() {
                                let pspan = pattern_span(pat);
                                if let Some((_, prev)) = seen.iter().find(|(x, _)| *x == k) {
                                    let (l, _) = self.sm.file(prev.file).line_col(prev.start as usize);
                                    self.emit(Diagnostic::warning(pspan, format!("this case is already handled on line {}", l)));
                                } else {
                                    seen.push((k, pspan));
                                }
                            }
                        }
                        if !bind.is_empty() {
                            binding = bind;
                        }
                        (c, f)
                    }
                };
                if single {
                    facts = f;
                }
                conds.push(c);
            }
            let mut cond = conds.into_iter().reduce(|a, b| Expr::new(ExprKind::Or(Box::new(a), Box::new(b)), T_BOOL));
            if cond.is_none() {
                has_else = arm.guard.is_none();
                if has_else {
                    else_span = Some(arm.span);
                }
            }
            self.apply(&facts.t);
            let mut body = Vec::new();
            for (name, t, field) in binding {
                if let Some((val, _, Some(tmp))) = &subj {
                    let declared = val.ty;
                    let read = Expr::new(ExprKind::Local(*tmp), declared);
                    let v = match field {
                        Some(path) => path,
                        None if t == declared => read,
                        None => self.payload_conv(read, declared, t),
                    };
                    let slot = self.declare_local(&name.name, t, name.span, false, arm.span);
                    self.hover(name.span, format!("{}: {}", name.name, self.show(t)));
                    body.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(v)), t)));
                } else {
                    self.error(name.span, "a binding needs a subject stored in a variable");
                }
            }
            if let Some(g) = &arm.guard {
                let (gc, gf) = self.cond(g);
                self.apply(&gf.t);
                cond = Some(match cond {
                    None => gc,
                    Some(c) => {
                        let pre_bind: Vec<Stmt> = body.clone();
                        let gc = if pre_bind.is_empty() {
                            gc
                        } else {
                            Expr::new(ExprKind::Seq(pre_bind, Box::new(gc)), T_BOOL)
                        };
                        Expr::new(ExprKind::And(Box::new(c), Box::new(gc)), T_BOOL)
                    }
                });
            }
            let mut value = None;
            match &arm.body {
                ArmBody::Block(b) => {
                    let stmts = self.block_stmts(&b.stmts);
                    if mode == Mode::Value && !diverges(&stmts) {
                        self.emit(
                            Diagnostic::error(b.span, "an arm of a `match` that produces a value must be an expression")
                                .help("write the value after `=>` directly, or use `match` as a statement"),
                        );
                    }
                    body.extend(stmts);
                }
                ArmBody::Expr(e) => match mode {
                    Mode::Stmt => {
                        let h = self.expr(e, None);
                        body.push(Stmt::Expr(h));
                    }
                    Mode::Value => {
                        let h = self.expr(e, expected);
                        if h.ty == T_VOID && super::stmt::diverges(&[Stmt::Expr(h.clone())]) {
                            body.push(Stmt::Expr(h));
                        } else {
                            if h.ty == T_VOID {
                                self.error(e.span, "this arm produces no value");
                            }
                            value = Some((h, e.span));
                        }
                    }
                },
            }
            self.ctx().scopes.pop();
            let end = std::mem::replace(&mut self.ctx().narrow, start);
            if !diverges(&body) {
                ends.push(end);
            }
            if arm.guard.is_none() {
                self.apply(&facts.f);
            }
            out.push((cond, body, value));
        }
        let mut missing = Vec::new();
        let mut exhaustive = has_else;
        if !exhaustive && subj.is_some() {
            let covered = |k: &Key| seen.iter().any(|(x, _)| x == k);
            let needs_null = self.types.is_nullable(sty) && sty != T_ANY;
            if let Some((ename, vs)) = &enum_variants {
                for (i, v) in vs.iter().enumerate() {
                    if !covered(&Key::Int(i as i64)) {
                        missing.push(format!("{}.{}", ename, v));
                    }
                }
                if needs_null && !covered(&Key::Null) {
                    missing.push("null".into());
                }
                exhaustive = missing.is_empty();
            } else if base == T_BOOL {
                for b in [true, false] {
                    if !covered(&Key::Bool(b)) {
                        missing.push(b.to_string());
                    }
                }
                if needs_null && !covered(&Key::Null) {
                    missing.push("null".into());
                }
                exhaustive = missing.is_empty();
            }
        }
        if !exhaustive {
            ends.push(self.ctx().narrow.clone());
        }
        self.ctx().narrow = before.clone();
        if let Some(first) = ends.pop() {
            let merged = ends
                .into_iter()
                .fold(first, |a, b| a.into_iter().filter(|(k, v)| b.get(k) == Some(v)).collect());
            self.ctx().narrow = merged;
        }
        if exhaustive && !has_else {
            if let Some(last) = out.last_mut() {
                if !arms.last().map(|a| a.guard.is_some()).unwrap_or(false) {
                    last.0 = None;
                }
            }
        }
        Prepared {
            pre,
            arms: out,
            exhaustive,
            missing,
        }
    }

    fn pattern_cond(
        &mut self,
        pat: &Pattern,
        val: Expr,
        key: Option<u32>,
        tmp: Option<u32>,
        variants: Option<&(String, Vec<String>)>,
    ) -> (Expr, Facts, Option<Key>, Vec<Bind>) {
        let sty = val.ty;
        let base = self.types.unwrap_optional(sty);
        let mut facts = Facts::default();
        match pat {
            Pattern::Value(e) => {
                if let Some(en) = self.types.variants_of(base).cloned() {
                    if let Some((i, name, args)) = self.variant_pattern(e, &en) {
                        return self.variant_cond(val, &en, i, &name, args, e.span, key, tmp);
                    }
                }
                if let (A::Ident(n), Some((_, vs))) = (&e.kind, variants) {
                    if self.peek_local(n).is_none() {
                        if let Some(i) = vs.iter().position(|v| v == n) {
                            let lit = Expr::new(ExprKind::Int(i as i64), base);
                            let c = self.equality(true, val, lit, e.span);
                            return (c, facts, Some(Key::Int(i as i64)), Vec::new());
                        }
                    }
                }
                if matches!(e.kind, A::Null) {
                    let c = self.is_check(val, T_NULL, e.span);
                    if let Some(k) = key {
                        let declared = self.declared_of(k);
                        let u = self.types.unwrap_optional(declared);
                        if u != declared && self.narrowable(declared, u) {
                            facts.f.push((k, u));
                        }
                    }
                    return (c, facts, Some(Key::Null), Vec::new());
                }
                let h = self.expr(e, Some(base));
                let k = match (&h.kind, self.types.get(h.ty)) {
                    (ExprKind::Int(v), Ty::Int | Ty::Enum(_)) => Some(Key::Int(*v)),
                    (ExprKind::Bool(b), _) => Some(Key::Bool(*b)),
                    (ExprKind::Str(i), _) => Some(Key::Str(self.strings[*i as usize].clone())),
                    _ => None,
                };
                if h.ty != T_ERROR
                    && sty != T_ERROR
                    && self.types.get(base) != self.types.get(h.ty)
                    && !(self.types.is_numeric(base) && self.types.is_numeric(h.ty))
                    && base != T_ANY
                {
                    let (a, b) = (self.show(sty), self.show(h.ty));
                    self.error(e.span, format!("this pattern is {} but the value being matched is {}", b, a));
                    return (Expr::new(ExprKind::Bool(false), T_BOOL), facts, None, Vec::new());
                }
                let val = if base != sty && self.types.is_nullable(sty) {
                    let lit = h.clone();
                    let nn = self.is_check(val.clone(), T_NULL, e.span);
                    let not_null = Expr::new(ExprKind::Unary(crate::hir::UnOp::Not, Box::new(nn)), T_BOOL);
                    let inner = self.payload_conv(val, sty, base);
                    let eq = self.equality(true, inner, lit, e.span);
                    return (Expr::new(ExprKind::And(Box::new(not_null), Box::new(eq)), T_BOOL), facts, k, Vec::new());
                } else {
                    val
                };
                (self.equality(true, val, h, e.span), facts, k, Vec::new())
            }
            Pattern::Range(a, b, inclusive) => {
                let ha = self.expr(a, Some(base));
                let hb = self.expr(b, Some(base));
                if !(self.types.is_numeric(base) || base == T_STR) {
                    let s = self.show(sty);
                    self.error(a.span.to(b.span), format!("ranges only match numbers and strings, not {}", s));
                    return (Expr::new(ExprKind::Bool(false), T_BOOL), facts, None, Vec::new());
                }
                let span = a.span.to(b.span);
                let lo = self.comparison(AOp::Ge, val.clone(), ha, span);
                let hi = self.comparison(if *inclusive { AOp::Le } else { AOp::Lt }, val, hb, span);
                (Expr::new(ExprKind::And(Box::new(lo), Box::new(hi)), T_BOOL), facts, None, Vec::new())
            }
            Pattern::Is(te, bind) => {
                let t = self.resolve_type(te);
                let c = self.is_check(val, t, te.span);
                if let Some(k) = key {
                    let declared = self.declared_of(k);
                    if t == T_NULL {
                        let u = self.types.unwrap_optional(declared);
                        if u != declared && self.narrowable(declared, u) {
                            facts.f.push((k, u));
                        }
                    } else if self.narrowable(declared, t) {
                        facts.t.push((k, t));
                    }
                }
                if let Some(tk) = tmp {
                    if Some(tk) != key {
                        let declared = self.declared_of(tk);
                        if self.narrowable(declared, t) {
                            facts.t.push((tk, t));
                        }
                    }
                }
                let k = if t == T_NULL { Some(Key::Null) } else { None };
                (c, facts, k, bind.clone().map(|b| (b, t, None)).into_iter().collect())
            }
        }
    }

    fn variant_pattern<'e>(&mut self, e: &'e ast::Expr, en: &IfaceDef) -> Option<(usize, ast::Ident, Option<&'e [ast::Expr]>)> {
        let (head, args) = match &e.kind {
            A::Call { callee, args } => (&**callee, Some(args.as_slice())),
            _ => (e, None),
        };
        let name = match &head.kind {
            A::Ident(n) if self.peek_local(n).is_none() => ast::Ident {
                name: n.clone(),
                span: head.span,
            },
            A::Field { obj, name } if self.type_ident(obj) == Some(en.ty) => name.clone(),
            _ => return None,
        };
        let i = en.variants.iter().position(|v| v.name == name.name)?;
        Some((i, name, args))
    }

    #[allow(clippy::too_many_arguments)]
    fn variant_cond(
        &mut self,
        val: Expr,
        en: &IfaceDef,
        i: usize,
        name: &ast::Ident,
        args: Option<&[ast::Expr]>,
        span: Span,
        key: Option<u32>,
        tmp: Option<u32>,
    ) -> (Expr, Facts, Option<Key>, Vec<Bind>) {
        let mut facts = Facts::default();
        let v = &en.variants[i];
        let rec = self.types.records[v.record as usize].clone();
        self.def_link(name.span, v.span);
        let rt = rec.ty;
        let sty = val.ty;
        let mut cond = self.is_check(val.clone(), rt, span);
        for k in [key, tmp].into_iter().flatten() {
            let declared = self.declared_of(k);
            if self.narrowable(declared, rt) && !facts.t.iter().any(|(x, _)| *x == k) {
                facts.t.push((k, rt));
            }
        }
        let mut binds = Vec::new();
        let mut total = true;
        match args {
            None => {}
            Some(args) if args.len() != rec.fields.len() => {
                let shape: Vec<String> = rec.fields.iter().map(|f| f.name.clone()).collect();
                self.emit(
                    Diagnostic::error(
                        span,
                        format!(
                            "`{}.{}` has {} field{} but the pattern lists {}",
                            en.name,
                            v.name,
                            rec.fields.len(),
                            if rec.fields.len() == 1 { "" } else { "s" },
                            args.len()
                        ),
                    )
                    .help(if shape.is_empty() {
                        format!("write `{}.{}`", en.name, v.name)
                    } else {
                        format!("write `{}.{}({})`, using `_` for fields you do not need", en.name, v.name, shape.join(", "))
                    }),
                );
                for a in args {
                    if let A::Ident(n) = &a.kind {
                        if n != "_" && self.peek_local(n).is_none() {
                            binds.push((ast::Ident { name: n.clone(), span: a.span }, T_ERROR, None));
                        }
                    }
                }
                return (Expr::new(ExprKind::Bool(false), T_BOOL), facts, None, binds);
            }
            Some(args) => {
                for (fi, (a, f)) in args.iter().zip(rec.fields.iter()).enumerate() {
                    let r = self.payload_conv(val.clone(), sty, rt);
                    let got = Expr::new(ExprKind::GetField(Box::new(r), fi as u32), f.ty);
                    let inner = self.types.variants_of(self.types.unwrap_optional(f.ty)).cloned();
                    if let Some(sub) = inner.as_ref().and_then(|en| self.variant_pattern(a, en).map(|p| (en.clone(), p))) {
                        let (en, (k, kname, kargs)) = sub;
                        let (c, _, _, b) = self.variant_cond(got, &en, k, &kname, kargs, a.span, None, None);
                        total = false;
                        cond = Expr::new(ExprKind::And(Box::new(cond), Box::new(c)), T_BOOL);
                        binds.extend(b);
                        continue;
                    }
                    match &a.kind {
                        A::Ident(n) if n == "_" => {}
                        A::Ident(n)
                            if self.peek_local(n).is_none()
                                && !matches!(self.lookup_value_entry(self.cur_module(), n), Some(e) if !matches!(e.sym, ValSym::Func(_))) =>
                        {
                            binds.push((ast::Ident { name: n.clone(), span: a.span }, f.ty, Some(got)));
                        }
                        _ => {
                            total = false;
                            let want = self.expr(a, Some(f.ty));
                            let eq = self.equality(true, got, want, a.span);
                            cond = Expr::new(ExprKind::And(Box::new(cond), Box::new(eq)), T_BOOL);
                        }
                    }
                }
            }
        }
        let k = if total { Some(Key::Int(i as i64)) } else { None };
        (cond, facts, k, binds)
    }
}

fn pattern_span(p: &Pattern) -> Span {
    match p {
        Pattern::Value(e) => e.span,
        Pattern::Range(a, b, _) => a.span.to(b.span),
        Pattern::Is(t, b) => match b {
            Some(b) => t.span.to(b.span),
            None => t.span,
        },
    }
}

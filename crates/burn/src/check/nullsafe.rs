use super::*;
use crate::diag::Diagnostic;

impl<'a> Checker<'a> {
    fn bind_temp(&mut self, h: Expr, narrowed: Option<TyId>, span: Span) -> (u32, ast::Expr, Stmt) {
        let t = h.ty;
        let slot = self.new_local(t);
        let name = format!(" tmp{}", slot);
        let c = self.ctx();
        c.scopes.last_mut().unwrap().insert(name.clone(), LocalSym { slot, is_const: true, span });
        if let Some(n) = narrowed {
            c.narrow.insert(slot, n);
        }
        let set = Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(h)), t));
        (
            slot,
            ast::Expr {
                kind: ast::ExprKind::Ident(name),
                span,
            },
            set,
        )
    }

    fn is_null(slot: u32, t: TyId) -> Expr {
        Expr::new(
            ExprKind::Binary(
                hir::BinOp::ICmp(hir::Cmp::Eq),
                Box::new(Expr::new(ExprKind::Local(slot), t)),
                Box::new(Expr::new(ExprKind::Null, t)),
            ),
            T_BOOL,
        )
    }

    fn op_span(&self, from: Span, to: Span, op: &str) -> Option<Span> {
        let between = Span::new(from.file, from.end as usize, to.start as usize);
        let text = self.src_text(between);
        text.find(op)
            .map(|i| Span::new(from.file, between.start as usize + i, between.start as usize + i + op.len()))
    }

    pub fn safe_get(&mut self, obj: &ast::Expr, name: &ast::Ident, args: Option<&[ast::Expr]>, span: Span) -> Expr {
        let o = self.expr(obj, None);
        let t = o.ty;
        let inner = match self.types.get(t).clone() {
            Ty::Optional(i) => Some(i),
            Ty::Any | Ty::Null => None,
            Ty::Error => {
                if let Some(a) = args {
                    for x in a {
                        self.expr(x, None);
                    }
                }
                return Self::err_expr();
            }
            _ => {
                let s = self.show(t);
                let mut d = Diagnostic::warning(span, format!("`?.` is not needed: a value of type {} is never null", s));
                if let Some(op) = self.op_span(obj.span, name.span, "?.") {
                    d = d.fix("use `.`", op, ".");
                }
                self.emit(d);
                Some(t)
            }
        };
        let nullable = inner != Some(t);
        let (slot, tmp, set) = self.bind_temp(o, inner.filter(|i| *i != t), obj.span);
        let access = match args {
            None => self.field(&tmp, name, span),
            Some(a) => self.method_call(&tmp, name, a, span, None),
        };
        if !nullable {
            return Expr::new(ExprKind::Seq(vec![set], Box::new(access.clone())), access.ty);
        }
        if access.ty == T_ERROR {
            return Self::err_expr();
        }
        if access.ty == T_VOID {
            let run = Stmt::If(Self::is_null(slot, t), vec![], vec![Stmt::Expr(access)]);
            return Expr::new(ExprKind::Seq(vec![set, run], Box::new(Expr::new(ExprKind::Int(0), T_VOID))), T_VOID);
        }
        let rt = self.types.optional(access.ty);
        let res = self.new_local(rt);
        let value = self.coerce(access, rt, span);
        let pick = Stmt::If(
            Self::is_null(slot, t),
            vec![Stmt::Expr(Expr::new(ExprKind::SetLocal(res, Box::new(Expr::new(ExprKind::Null, rt))), rt))],
            vec![Stmt::Expr(Expr::new(ExprKind::SetLocal(res, Box::new(value)), rt))],
        );
        Expr::new(ExprKind::Seq(vec![set, pick], Box::new(Expr::new(ExprKind::Local(res), rt))), rt)
    }

    pub fn coalesce(&mut self, a: &ast::Expr, b: &ast::Expr, span: Span, expected: Option<TyId>) -> Expr {
        let hint = expected.map(|t| self.types.optional(t));
        let l = self.expr(a, hint);
        let t = l.ty;
        let inner = match self.types.get(t).clone() {
            Ty::Optional(i) => i,
            Ty::Any => T_ANY,
            Ty::Null => return self.expr(b, expected),
            Ty::Error => {
                self.expr(b, None);
                return Self::err_expr();
            }
            _ => {
                let s = self.show(t);
                let left = self.src_text(a.span);
                self.emit(Diagnostic::warning(span, format!("`??` is not needed: a value of type {} is never null", s)).fix("remove the fallback", span, left));
                self.expr(b, Some(t));
                return l;
            }
        };
        let r = self.expr(b, Some(inner));
        let rt = if r.ty == T_NULL || self.types.is_nullable(r.ty) && r.ty != T_ANY && inner != T_ANY {
            t
        } else {
            inner
        };
        let (slot, tmp, set) = self.bind_temp(l, (inner != T_ANY).then_some(inner), a.span);
        let value = self.expr(&tmp, None);
        let value = self.coerce(value, rt, a.span);
        let fallback = self.coerce(r, rt, b.span);
        let res = self.new_local(rt);
        let pick = Stmt::If(
            Self::is_null(slot, t),
            vec![Stmt::Expr(Expr::new(ExprKind::SetLocal(res, Box::new(fallback)), rt))],
            vec![Stmt::Expr(Expr::new(ExprKind::SetLocal(res, Box::new(value)), rt))],
        );
        Expr::new(ExprKind::Seq(vec![set, pick], Box::new(Expr::new(ExprKind::Local(res), rt))), rt)
    }

    pub fn safe_as(&mut self, x: &ast::Expr, te: &TypeExpr, span: Span) -> Expr {
        let target = self.resolve_type(te);
        let h = self.expr(x, None);
        let from = h.ty;
        if from == T_ERROR || target == T_ERROR {
            return Self::err_expr();
        }
        let rt = self.types.optional(target);
        if from == target || (!self.types.is_nullable(from) && self.types.implements(from, target)) {
            let s = self.show(from);
            let mut d = Diagnostic::warning(span, format!("`as?` always succeeds here: the value is already {}", s));
            if let Some(op) = self.op_span(x.span, te.span, "as?") {
                d = d.fix("use `as`", op, "as");
            }
            self.emit(d);
            return self.coerce(h, rt, span);
        }
        let (slot, _, set) = self.bind_temp(h, None, x.span);
        let raw = Expr::new(ExprKind::Local(slot), from);
        let check = self.is_check(raw.clone(), target, span);
        if check.ty == T_ERROR {
            return Self::err_expr();
        }
        let l = self.loc_expr(span);
        let cast = Expr::new(ExprKind::Rt(RtFn::Cast, vec![raw, Self::tid(from), Self::tid(target), l]), target);
        let value = self.coerce(cast, rt, span);
        let res = self.new_local(rt);
        let pick = Stmt::If(
            check,
            vec![Stmt::Expr(Expr::new(ExprKind::SetLocal(res, Box::new(value)), rt))],
            vec![Stmt::Expr(Expr::new(ExprKind::SetLocal(res, Box::new(Expr::new(ExprKind::Null, rt))), rt))],
        );
        Expr::new(ExprKind::Seq(vec![set, pick], Box::new(Expr::new(ExprKind::Local(res), rt))), rt)
    }
}

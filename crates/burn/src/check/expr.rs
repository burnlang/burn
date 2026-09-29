use super::*;
use crate::ast::{BinOp as AOp, ExprKind as A, UnOp as AUn};
use crate::hir::{BinOp, Cmp, Conv, UnOp};

pub enum Place {
    Local(u32, TyId),
    Global(u32, TyId),
    Field(Expr, u32, TyId),
    Index(Expr, Expr, TyId, u32),
    MapKey(Expr, Expr, TyId),
    Error,
}

impl<'a> Checker<'a> {
    pub fn err_expr() -> Expr {
        Expr::new(ExprKind::Int(0), T_ERROR)
    }

    pub fn expr_to(&mut self, e: &ast::Expr, target: TyId) -> Expr {
        let h = self.expr(e, Some(target));
        self.coerce(h, target, e.span)
    }

    pub fn coerce(&mut self, h: Expr, target: TyId, span: Span) -> Expr {
        match self.try_coerce(h, target) {
            Ok(x) => x,
            Err(h) => {
                let (from, to) = (self.show(h.ty), self.show(target));
                if h.ty == T_VOID {
                    self.error(span, format!("expected a value of type {} but this expression returns nothing", to));
                } else if h.ty == T_NULL {
                    self.error_note(span, format!("`null` is not allowed for type {}", to), format!("use `{}?` to make the type nullable", to));
                } else if self.types.is_nullable(h.ty) && self.types.unwrap_optional(h.ty) == target {
                    self.error_note(span, format!("expected {} but found {}", to, from), "the value may be null: check it with `if (x != null)` first, or use `x!!`");
                } else if h.ty == T_ANY {
                    self.error_note(span, format!("expected {} but found any", to), format!("narrow it with `if (x is {})` or cast it with `x as {}`", to, to));
                } else if h.ty == T_FLOAT && target == T_INT {
                    self.error_note(span, "expected int but found float", "convert it explicitly with `x as int` or `round(x)`");
                } else {
                    self.error(span, format!("expected {} but found {}", to, from));
                }
                Self::err_expr()
            }
        }
    }

    pub fn retype(mut h: Expr, t: TyId) -> Expr {
        h.ty = t;
        h
    }

    pub fn try_coerce(&mut self, h: Expr, to: TyId) -> Result<Expr, Expr> {
        let from = h.ty;
        if from == to || from == T_ERROR || to == T_ERROR {
            return Ok(h);
        }
        let fk = self.types.get(from).clone();
        let tk = self.types.get(to).clone();
        match (&fk, &tk) {
            (Ty::Int, Ty::Float) => {
                if let ExprKind::Int(v) = h.kind {
                    return Ok(Expr::new(ExprKind::Float(v as f64), T_FLOAT));
                }
                Ok(Expr::new(ExprKind::Conv(Conv::IntToFloat, Box::new(h)), T_FLOAT))
            }
            (Ty::Null, Ty::Optional(_) | Ty::Any) => Ok(Expr::new(ExprKind::Null, to)),
            (Ty::Void, _) => Err(h),
            (_, Ty::Any) => Ok(Expr::new(ExprKind::Rt(RtFn::Box, vec![h, Self::tid(from)]), T_ANY)),
            (Ty::Optional(x), Ty::Optional(y)) => {
                let (x, y) = (*x, *y);
                if !self.types.is_unboxed(x) && !self.types.is_unboxed(y) && (self.types.implements(x, y)) {
                    return Ok(Self::retype(h, to));
                }
                Err(h)
            }
            (_, Ty::Optional(inner)) => {
                let inner = *inner;
                let h2 = self.try_coerce(h, inner)?;
                if self.types.is_unboxed(inner) {
                    Ok(Expr::new(ExprKind::Rt(RtFn::Box, vec![h2, Self::tid(inner)]), to))
                } else {
                    Ok(Self::retype(h2, to))
                }
            }
            (Ty::Record(_), Ty::Interface(_)) => {
                if self.types.implements(from, to) {
                    Ok(Self::retype(h, to))
                } else {
                    Err(h)
                }
            }
            (Ty::Record(a), Ty::Record(b)) => {
                let (a, b) = (*a, *b);
                let ra = self.types.records[a as usize].clone();
                let rb = self.types.records[b as usize].clone();
                if !ra.anon || rb.is_class || ra.fields.len() != rb.fields.len() || !rb.fields.iter().all(|f| ra.field_index(&f.name).is_some()) {
                    return Err(h);
                }
                if let ExprKind::NewStruct(_, items) = &h.kind {
                    let mut out = Vec::new();
                    for f in &rb.fields {
                        let i = ra.field_index(&f.name).unwrap();
                        match self.try_coerce(items[i].clone(), f.ty) {
                            Ok(x) => out.push(x),
                            Err(_) => return Err(h),
                        }
                    }
                    return Ok(Expr::new(ExprKind::NewStruct(to, out), to));
                }
                let slot = self.new_local(from);
                let mut out = Vec::new();
                for f in &rb.fields {
                    let i = ra.field_index(&f.name).unwrap();
                    let get = Expr::new(ExprKind::GetField(Box::new(Expr::new(ExprKind::Local(slot), from)), i as u32), ra.fields[i].ty);
                    match self.try_coerce(get, f.ty) {
                        Ok(x) => out.push(x),
                        Err(_) => return Err(h),
                    }
                }
                let set = Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(h)), from));
                Ok(Expr::new(ExprKind::Seq(vec![set], Box::new(Expr::new(ExprKind::NewStruct(to, out), to))), to))
            }
            (Ty::Array(_), Ty::Array(y)) => {
                let y = *y;
                if let ExprKind::NewArray(_, items) = &h.kind {
                    let mut out = Vec::new();
                    for it in items {
                        match self.try_coerce(it.clone(), y) {
                            Ok(x) => out.push(x),
                            Err(_) => return Err(h),
                        }
                    }
                    return Ok(Expr::new(ExprKind::NewArray(to, out), to));
                }
                Err(h)
            }
            _ => Err(h),
        }
    }

    pub fn to_str(&mut self, h: Expr) -> Expr {
        if h.ty == T_STR || h.ty == T_ERROR {
            return h;
        }
        if let (ExprKind::Int(v), true) = (&h.kind, h.ty == T_INT) {
            return self.str_lit(&v.to_string());
        }
        let t = h.ty;
        Expr::new(ExprKind::Rt(RtFn::ToStr, vec![h, Self::tid(t)]), T_STR)
    }

    pub fn concat(&mut self, a: Expr, b: Expr) -> Expr {
        let a = self.to_str(a);
        let b = self.to_str(b);
        if let (ExprKind::Str(x), ExprKind::Str(y)) = (&a.kind, &b.kind) {
            let s = format!("{}{}", self.strings[*x as usize], self.strings[*y as usize]);
            return self.str_lit(&s);
        }
        Expr::new(ExprKind::Rt(RtFn::StrConcat, vec![a, b]), T_STR)
    }

    pub fn with_temp(&mut self, h: Expr, f: impl FnOnce(&mut Self, Expr) -> Expr) -> Expr {
        if !h.has_side_effects() {
            return f(self, h);
        }
        let t = h.ty;
        let slot = self.new_local(t);
        let set = Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(h)), t));
        let body = f(self, Expr::new(ExprKind::Local(slot), t));
        let bt = body.ty;
        Expr::new(ExprKind::Seq(vec![set], Box::new(body)), bt)
    }

    pub fn zero(&mut self, t: TyId) -> Option<Expr> {
        Some(match self.types.get(t).clone() {
            Ty::Int => Expr::int(0),
            Ty::Float => Expr::new(ExprKind::Float(0.0), T_FLOAT),
            Ty::Bool => Expr::new(ExprKind::Bool(false), T_BOOL),
            Ty::Str => self.str_lit(""),
            Ty::Optional(_) | Ty::Any | Ty::Null => Expr::new(ExprKind::Null, t),
            Ty::Array(_) => Expr::new(ExprKind::NewArray(t, vec![]), t),
            Ty::Map(..) => Expr::new(ExprKind::Rt(RtFn::MapNew, vec![Self::tid(t)]), t),
            Ty::Enum(_) => Expr::new(ExprKind::Int(0), t),
            Ty::Error => Self::err_expr(),
            _ => return None,
        })
    }

    pub fn payload_conv(&mut self, h: Expr, declared: TyId, narrowed: TyId) -> Expr {
        match self.types.get(declared).clone() {
            Ty::Any => Expr::new(ExprKind::BoxVal(Box::new(h)), narrowed),
            Ty::Optional(x) if self.types.is_unboxed(x) => Expr::new(ExprKind::BoxVal(Box::new(h)), narrowed),
            _ => Self::retype(h, narrowed),
        }
    }

    pub fn read_local(&mut self, slot: u32) -> Expr {
        let declared = self.ctx().locals[slot as usize];
        let e = Expr::new(ExprKind::Local(slot), declared);
        match self.ctx().narrow.get(&slot).copied() {
            Some(n) if n != declared => self.payload_conv(e, declared, n),
            _ => e,
        }
    }

    pub fn narrowable(&self, declared: TyId, value: TyId) -> bool {
        if declared == value {
            return false;
        }
        let dk = self.types.get(declared);
        if !matches!(dk, Ty::Any | Ty::Optional(_) | Ty::Interface(_)) {
            return false;
        }
        if matches!(self.types.get(value), Ty::Null | Ty::Any | Ty::Optional(_) | Ty::Error | Ty::Void) {
            return false;
        }
        match dk {
            Ty::Any => true,
            Ty::Optional(x) => *x == value || self.types.implements(value, *x),
            Ty::Interface(_) => self.types.implements(value, declared),
            _ => false,
        }
    }

    pub fn declared_of(&self, key: u32) -> TyId {
        if key >= GLOBAL_KEY {
            return self.globals[(key - GLOBAL_KEY) as usize].ty.unwrap_or(T_ERROR);
        }
        self.fx.last().unwrap().locals[key as usize]
    }

    pub fn invalidate_globals(&mut self) {
        if let Some(c) = self.fx.last_mut() {
            c.narrow.retain(|k, _| *k < GLOBAL_KEY);
        }
    }

    pub fn read_global(&mut self, g: u32, t: TyId) -> Expr {
        let e = Expr::new(ExprKind::Global(g), t);
        match self.fx.last().and_then(|c| c.narrow.get(&(GLOBAL_KEY + g)).copied()) {
            Some(n) if n != t => self.payload_conv(e, t, n),
            _ => e,
        }
    }

    pub fn narrow_after_assign(&mut self, slot: u32, value: TyId) {
        let declared = self.declared_of(slot);
        if self.narrowable(declared, value) {
            self.ctx().narrow.insert(slot, value);
        } else {
            self.ctx().narrow.remove(&slot);
        }
    }

    pub fn expr(&mut self, e: &ast::Expr, expected: Option<TyId>) -> Expr {
        let h = self.expr_inner(e, expected);
        if matches!(e.kind, A::Ident(_) | A::Field { .. } | A::Call { .. } | A::Index { .. }) {
            self.note_type(e.span, h.ty);
        }
        h
    }

    fn expr_inner(&mut self, e: &ast::Expr, expected: Option<TyId>) -> Expr {
        match &e.kind {
            A::Int(v) => {
                if expected == Some(T_FLOAT) {
                    Expr::new(ExprKind::Float(*v as f64), T_FLOAT)
                } else {
                    Expr::int(*v)
                }
            }
            A::Float(v) => Expr::new(ExprKind::Float(*v), T_FLOAT),
            A::Str(s) => self.str_lit(s),
            A::Template(parts) => {
                let mut acc: Option<Expr> = None;
                for p in parts {
                    let piece = match p {
                        ast::TplExpr::Lit(s) => self.str_lit(s),
                        ast::TplExpr::Expr(x) => {
                            let h = self.expr(x, None);
                            if h.ty == T_VOID {
                                self.error(x.span, "cannot put a value of type void into a string");
                            }
                            self.to_str(h)
                        }
                    };
                    acc = Some(match acc {
                        None => piece,
                        Some(a) => self.concat(a, piece),
                    });
                }
                acc.unwrap_or_else(|| self.str_lit(""))
            }
            A::Bool(b) => Expr::new(ExprKind::Bool(*b), T_BOOL),
            A::Null => {
                let t = match expected {
                    Some(t) if self.types.is_nullable(t) => t,
                    _ => T_NULL,
                };
                Expr::new(ExprKind::Null, t)
            }
            A::Ident(name) => self.ident_expr(name, e.span),
            A::Unary(AUn::Neg, x) => {
                let h = self.expr(x, expected.filter(|t| self.types.is_numeric(*t)));
                match self.types.get(h.ty) {
                    Ty::Int => Expr::new(ExprKind::Unary(UnOp::INeg, Box::new(h)), T_INT),
                    Ty::Float => Expr::new(ExprKind::Unary(UnOp::FNeg, Box::new(h)), T_FLOAT),
                    Ty::Error => h,
                    _ => {
                        let s = self.show(h.ty);
                        self.error(e.span, format!("cannot negate a value of type {}", s));
                        Self::err_expr()
                    }
                }
            }
            A::Unary(AUn::Not, _) | A::Binary(AOp::And | AOp::Or, _, _) => self.cond(e).0,
            A::Binary(op, l, r) => self.binary(*op, l, r, e.span, expected),
            A::Assign { target, op, value } => self.assign(target, *op, value, e.span),
            A::Call { callee, args } => self.call(callee, args, e.span, expected),
            A::Field { obj, name } => self.field(obj, name, e.span),
            A::Index { obj, index } => self.index(obj, index, e.span),
            A::Array(items) => self.array_lit(items, expected, e.span),
            A::StructLit { ty, fields } => self.struct_lit(ty.as_ref(), fields, expected, e.span),
            A::MapLit(pairs) => self.map_lit(pairs, expected, e.span),
            A::Is(x, te) => self.is_expr(x, te, e.span),
            A::As(x, te) => self.as_expr(x, te, e.span),
            A::Await(x) => {
                let h = self.expr(x, None);
                match self.types.get(h.ty).clone() {
                    Ty::Future(t) => Expr::new(ExprKind::Rt(RtFn::Await, vec![h]), t),
                    Ty::Error => h,
                    _ => {
                        let s = self.show(h.ty);
                        self.error_note(e.span, format!("`await` needs a Future, but this is {}", s), "only calls to `async fun` functions produce futures");
                        h
                    }
                }
            }
            A::Lambda(f) => self.lambda(f),
            A::NotNull(x) => {
                let h = self.expr(x, None);
                match self.types.get(h.ty).clone() {
                    Ty::Optional(inner) => {
                        let l = self.loc_expr(e.span);
                        let t = h.ty;
                        Expr::new(ExprKind::Rt(RtFn::Unwrap, vec![h, Self::tid(t), l]), inner)
                    }
                    Ty::Any => {
                        self.error(e.span, "`!!` cannot be used on `any`; cast it with `as` instead");
                        Self::err_expr()
                    }
                    Ty::Error => h,
                    _ => {
                        let s = self.show(h.ty);
                        self.warn(e.span, format!("unnecessary `!!`: a value of type {} is never null", s));
                        h
                    }
                }
            }
        }
    }

    fn outer_local_exists(&self, name: &str) -> bool {
        let n = self.fx.len();
        if n < 2 {
            return false;
        }
        self.fx[..n - 1].iter().any(|c| !c.is_init && c.scopes.iter().any(|s| s.contains_key(name)))
    }

    pub fn self_field(&self, name: &str) -> Option<(TyId, usize, TyId)> {
        let st = self.fx.last()?.self_ty?;
        let rec = self.types.record_of(st)?;
        let i = rec.field_index(name)?;
        Some((st, i, rec.fields[i].ty))
    }

    pub fn ident_expr(&mut self, name: &str, span: Span) -> Expr {
        if let Some(l) = self.lookup_local(name) {
            self.def_link(span, l.span);
            let h = self.read_local(l.slot);
            let t = self.show(h.ty);
            self.hover(span, format!("{}: {}", name, t));
            return h;
        }
        if let Some((st, i, ft)) = self.self_field(name) {
            let sp = self.types.record_of(st).unwrap().fields[i].span;
            self.def_link(span, sp);
            let t = self.show(ft);
            self.hover(span, format!("(field) {}: {}", name, t));
            let s = Expr::new(ExprKind::Local(0), st);
            return Expr::new(ExprKind::GetField(Box::new(s), i as u32), ft);
        }
        let m = self.cur_module();
        match self.lookup_value(name, span) {
            Some(ValSym::Global(g)) => {
                let gi = &self.globals[g as usize];
                let (gspan, gty, gconst) = (gi.span, gi.ty, gi.is_const);
                match gty {
                    Some(t) => {
                        self.def_link(span, gspan);
                        let ts = self.show(t);
                        self.hover(span, format!("{} {}: {}", if gconst { "const" } else { "var" }, name, ts));
                        self.read_global(g, t)
                    }
                    None => {
                        self.error(span, format!("`{}` is used before it is declared", name));
                        Self::err_expr()
                    }
                }
            }
            Some(ValSym::Func(f)) => {
                let fspan = self.funcs[f as usize].span;
                self.def_link(span, fspan);
                let t = self.func_type(f);
                if self.funcs[f as usize].is_async {
                    self.error(span, "async functions cannot be used as values yet; call them directly");
                }
                Expr::new(ExprKind::FuncRef(f), t)
            }
            None => {
                if self.lookup_type_name(m, name, span).is_some() {
                    self.error(span, format!("`{}` is a type, not a value", name));
                } else if name == "self" {
                    self.error(span, "`self` can only be used inside class methods");
                } else if self.outer_local_exists(name) {
                    self.error(span, format!("lambdas cannot capture the local variable `{}`; pass it as a parameter instead", name));
                } else if builtins::is_builtin(name) {
                    self.error(span, format!("built-in function `{}` must be called, e.g. `{}(...)`", name, name));
                } else if self.is_private_elsewhere(m, name) {
                    self.error(span, format!("`{}` is private to its module", name));
                } else {
                    let mut cands: Vec<String> = Vec::new();
                    if let Some(c) = self.fx.last() {
                        for s in &c.scopes {
                            cands.extend(s.keys().cloned());
                        }
                    }
                    cands.extend(self.mods[m].values.keys().cloned());
                    for imp in self.mods[m].imports.clone() {
                        cands.extend(self.mods[imp].values.iter().filter(|(_, e)| e.vis != Vis::Priv).map(|(k, _)| k.clone()));
                    }
                    cands.extend(builtins::BUILTINS.iter().map(|s| s.to_string()));
                    match suggest(name, cands.iter().map(|s| s.as_str())) {
                        Some(s) => self.error_note(span, format!("cannot find `{}` in this scope", name), format!("did you mean `{}`?", s)),
                        None => self.error(span, format!("cannot find `{}` in this scope", name)),
                    }
                }
                Self::err_expr()
            }
        }
    }

    pub fn arith(&mut self, op: AOp, l: Expr, r: Expr, span: Span) -> Expr {
        let (lt, rt) = (l.ty, r.ty);
        if lt == T_ERROR || rt == T_ERROR {
            return Self::err_expr();
        }
        if op == AOp::Add && (lt == T_STR || rt == T_STR) {
            if lt == T_VOID || rt == T_VOID {
                self.error(span, "cannot concatenate a value of type void");
                return Self::err_expr();
            }
            return self.concat(l, r);
        }
        if op == AOp::Add {
            if let (Ty::Array(a), Ty::Array(b)) = (self.types.get(lt).clone(), self.types.get(rt).clone()) {
                if a == b {
                    return Expr::new(ExprKind::Rt(RtFn::ArrConcat, vec![l, r]), lt);
                }
            }
        }
        let numeric = self.types.is_numeric(lt) && self.types.is_numeric(rt);
        if !numeric {
            let (a, b) = (self.show(lt), self.show(rt));
            let hint = if lt == T_ANY || rt == T_ANY {
                Some("values of type `any` must be narrowed with `is` or cast with `as` before doing math")
            } else if self.types.is_nullable(lt) || self.types.is_nullable(rt) {
                Some("one side may be null; check it with `!= null` first")
            } else {
                None
            };
            let msg = format!("cannot apply `{}` to {} and {}", op.symbol(), a, b);
            match hint {
                Some(h) => self.error_note(span, msg, h),
                None => self.error(span, msg),
            }
            return Self::err_expr();
        }
        if lt == T_INT && rt == T_INT {
            if let (ExprKind::Int(a), ExprKind::Int(b)) = (&l.kind, &r.kind) {
                let (a, b) = (*a, *b);
                let v = match op {
                    AOp::Add => Some(a.wrapping_add(b)),
                    AOp::Sub => Some(a.wrapping_sub(b)),
                    AOp::Mul => Some(a.wrapping_mul(b)),
                    AOp::Div if b != 0 => Some(a.wrapping_div(b)),
                    AOp::Mod if b != 0 => Some(a.wrapping_rem(b)),
                    _ => None,
                };
                if let Some(v) = v {
                    return Expr::int(v);
                }
            }
            let bop = match op {
                AOp::Add => BinOp::IAdd,
                AOp::Sub => BinOp::ISub,
                AOp::Mul => BinOp::IMul,
                AOp::Div => BinOp::IDiv(self.loc(span)),
                _ => BinOp::IMod(self.loc(span)),
            };
            return Expr::new(ExprKind::Binary(bop, Box::new(l), Box::new(r)), T_INT);
        }
        let l = self.coerce(l, T_FLOAT, span);
        let r = self.coerce(r, T_FLOAT, span);
        let bop = match op {
            AOp::Add => BinOp::FAdd,
            AOp::Sub => BinOp::FSub,
            AOp::Mul => BinOp::FMul,
            AOp::Div => BinOp::FDiv,
            _ => return Expr::new(ExprKind::Rt(RtFn::FMod, vec![l, r]), T_FLOAT),
        };
        Expr::new(ExprKind::Binary(bop, Box::new(l), Box::new(r)), T_FLOAT)
    }

    fn binary(&mut self, op: AOp, l: &ast::Expr, r: &ast::Expr, span: Span, expected: Option<TyId>) -> Expr {
        match op {
            AOp::Eq | AOp::Ne => {
                let (a, b) = (self.expr(l, None), None::<Expr>);
                let _ = b;
                let rexp = if a.ty == T_NULL || a.ty == T_ERROR { None } else { Some(a.ty) };
                let b = self.expr(r, rexp);
                self.equality(op == AOp::Eq, a, b, span)
            }
            AOp::Lt | AOp::Gt | AOp::Le | AOp::Ge => {
                let a = self.expr(l, None);
                let rexp = if self.types.is_numeric(a.ty) { Some(a.ty) } else { None };
                let b = self.expr(r, rexp);
                self.comparison(op, a, b, span)
            }
            AOp::And | AOp::Or => unreachable!(),
            _ => {
                let lexp = expected.filter(|t| self.types.is_numeric(*t));
                let a = self.expr(l, lexp);
                let rexp = if self.types.is_numeric(a.ty) { Some(a.ty) } else { None };
                let b = self.expr(r, rexp);
                self.arith(op, a, b, span)
            }
        }
    }

    pub fn equality(&mut self, eq: bool, a: Expr, b: Expr, span: Span) -> Expr {
        let cmp = if eq { Cmp::Eq } else { Cmp::Ne };
        let (at, bt) = (a.ty, b.ty);
        if at == T_ERROR || bt == T_ERROR {
            return Expr::new(ExprKind::Bool(false), T_BOOL);
        }
        if at == T_NULL || bt == T_NULL || matches!(a.kind, ExprKind::Null) || matches!(b.kind, ExprKind::Null) {
            let (other, ot) = if at == T_NULL || matches!(a.kind, ExprKind::Null) { (b, bt) } else { (a, at) };
            if !self.types.is_nullable(ot) && !matches!(self.types.get(ot), Ty::Interface(_)) {
                let s = self.show(ot);
                self.error(span, format!("a value of type {} can never be null", s));
            }
            return Expr::new(ExprKind::Binary(BinOp::ICmp(cmp), Box::new(other), Box::new(Expr::new(ExprKind::Null, ot))), T_BOOL);
        }
        let (ak, bk) = (self.types.get(at).clone(), self.types.get(bt).clone());
        if self.types.is_numeric(at) && self.types.is_numeric(bt) {
            if at == T_INT && bt == T_INT {
                return Expr::new(ExprKind::Binary(BinOp::ICmp(cmp), Box::new(a), Box::new(b)), T_BOOL);
            }
            let a = self.coerce(a, T_FLOAT, span);
            let b = self.coerce(b, T_FLOAT, span);
            return Expr::new(ExprKind::Binary(BinOp::FCmp(cmp), Box::new(a), Box::new(b)), T_BOOL);
        }
        if at == bt && matches!(ak, Ty::Bool | Ty::Enum(_) | Ty::Func(..)) {
            return Expr::new(ExprKind::Binary(BinOp::ICmp(cmp), Box::new(a), Box::new(b)), T_BOOL);
        }
        if at == T_STR && bt == T_STR {
            let e = Expr::new(ExprKind::Rt(RtFn::StrEq, vec![a, b]), T_BOOL);
            return if eq { e } else { Expr::new(ExprKind::Unary(UnOp::Not, Box::new(e)), T_BOOL) };
        }
        let _ = bk;
        let (a, b, t) = if at == bt {
            (a, b, at)
        } else {
            match self.try_coerce(b, at) {
                Ok(b2) => (a, b2, at),
                Err(b) => match self.try_coerce(a, bt) {
                    Ok(a2) => (a2, b, bt),
                    Err(a) => {
                        let (x, y) = (self.show(a.ty), self.show(b.ty));
                        self.error(span, format!("cannot compare {} with {}", x, y));
                        return Expr::new(ExprKind::Bool(false), T_BOOL);
                    }
                },
            }
        };
        let e = Expr::new(ExprKind::Rt(RtFn::Eq, vec![a, b, Self::tid(t)]), T_BOOL);
        if eq {
            e
        } else {
            Expr::new(ExprKind::Unary(UnOp::Not, Box::new(e)), T_BOOL)
        }
    }

    fn comparison(&mut self, op: AOp, a: Expr, b: Expr, span: Span) -> Expr {
        let cmp = match op {
            AOp::Lt => Cmp::Lt,
            AOp::Gt => Cmp::Gt,
            AOp::Le => Cmp::Le,
            _ => Cmp::Ge,
        };
        let (at, bt) = (a.ty, b.ty);
        if at == T_ERROR || bt == T_ERROR {
            return Expr::new(ExprKind::Bool(false), T_BOOL);
        }
        if self.types.is_numeric(at) && self.types.is_numeric(bt) {
            if at == T_INT && bt == T_INT {
                return Expr::new(ExprKind::Binary(BinOp::ICmp(cmp), Box::new(a), Box::new(b)), T_BOOL);
            }
            let a = self.coerce(a, T_FLOAT, span);
            let b = self.coerce(b, T_FLOAT, span);
            return Expr::new(ExprKind::Binary(BinOp::FCmp(cmp), Box::new(a), Box::new(b)), T_BOOL);
        }
        if at == T_STR && bt == T_STR {
            let c = Expr::new(ExprKind::Rt(RtFn::StrCmp, vec![a, b]), T_INT);
            return Expr::new(ExprKind::Binary(BinOp::ICmp(cmp), Box::new(c), Box::new(Expr::int(0))), T_BOOL);
        }
        if at == bt && matches!(self.types.get(at), Ty::Enum(_)) {
            return Expr::new(ExprKind::Binary(BinOp::ICmp(cmp), Box::new(a), Box::new(b)), T_BOOL);
        }
        let (x, y) = (self.show(at), self.show(bt));
        self.error(span, format!("cannot order {} and {} with `{}`", x, y, op.symbol()));
        Expr::new(ExprKind::Bool(false), T_BOOL)
    }

    pub fn place(&mut self, target: &ast::Expr, hoist: bool, pre: &mut Vec<Stmt>) -> Place {
        match &target.kind {
            A::Ident(name) => {
                if let Some(l) = self.lookup_local(name) {
                    if l.is_const {
                        self.error(target.span, format!("cannot assign to constant `{}`", name));
                    }
                    self.def_link(target.span, l.span);
                    let t = self.ctx().locals[l.slot as usize];
                    return Place::Local(l.slot, t);
                }
                if let Some((st, i, ft)) = self.self_field(name) {
                    return Place::Field(Expr::new(ExprKind::Local(0), st), i as u32, ft);
                }
                match self.lookup_value(name, target.span) {
                    Some(ValSym::Global(g)) => {
                        let gi = &self.globals[g as usize];
                        let (is_const, ty, gspan) = (gi.is_const, gi.ty, gi.span);
                        if is_const {
                            self.error(target.span, format!("cannot assign to constant `{}`", name));
                        }
                        self.def_link(target.span, gspan);
                        match ty {
                            Some(t) => Place::Global(g, t),
                            None => {
                                self.error(target.span, format!("`{}` is used before it is declared", name));
                                Place::Error
                            }
                        }
                    }
                    Some(ValSym::Func(_)) => {
                        self.error(target.span, format!("cannot assign to function `{}`", name));
                        Place::Error
                    }
                    None => {
                        self.ident_expr(name, target.span);
                        Place::Error
                    }
                }
            }
            A::Field { obj, name } => {
                let o = self.expr(obj, None);
                let o = if hoist { self.hoist(o, pre) } else { o };
                match self.types.get(o.ty).clone() {
                    Ty::Record(ri) => {
                        let rec = self.types.records[ri as usize].clone();
                        match rec.field_index(&name.name) {
                            Some(i) => {
                                self.check_field_access(&rec, i, name.span);
                                self.def_link(name.span, rec.fields[i].span);
                                Place::Field(o, i as u32, rec.fields[i].ty)
                            }
                            None => {
                                self.error(name.span, format!("{} has no field `{}`", rec.name, name.name));
                                Place::Error
                            }
                        }
                    }
                    Ty::Error => Place::Error,
                    Ty::Optional(_) => {
                        self.error_note(obj.span, "cannot assign a field of a value that may be null", "check it with `if (x != null)` first");
                        Place::Error
                    }
                    _ => {
                        let s = self.show(o.ty);
                        self.error(target.span, format!("cannot assign field `{}` on a value of type {}", name.name, s));
                        Place::Error
                    }
                }
            }
            A::Index { obj, index } => {
                let o = self.expr(obj, None);
                match self.types.get(o.ty).clone() {
                    Ty::Array(e) => {
                        let o = if hoist { self.hoist(o, pre) } else { o };
                        let i = self.expr_to(index, T_INT);
                        let i = if hoist { self.hoist(i, pre) } else { i };
                        let l = self.loc(target.span);
                        Place::Index(o, i, e, l)
                    }
                    Ty::Map(k, v) => {
                        let o = if hoist { self.hoist(o, pre) } else { o };
                        let key = self.expr_to(index, k);
                        let key = if hoist { self.hoist(key, pre) } else { key };
                        Place::MapKey(o, key, v)
                    }
                    Ty::Str => {
                        self.error(target.span, "strings are immutable; build a new string instead");
                        Place::Error
                    }
                    Ty::Error => Place::Error,
                    _ => {
                        let s = self.show(o.ty);
                        self.error(target.span, format!("cannot assign by index into a value of type {}", s));
                        Place::Error
                    }
                }
            }
            _ => {
                self.error(target.span, "invalid assignment target");
                Place::Error
            }
        }
    }

    fn hoist(&mut self, h: Expr, pre: &mut Vec<Stmt>) -> Expr {
        if !h.has_side_effects() {
            return h;
        }
        let t = h.ty;
        let slot = self.new_local(t);
        pre.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(h)), t)));
        Expr::new(ExprKind::Local(slot), t)
    }

    pub fn read_place(&mut self, p: &Place) -> Expr {
        match p {
            Place::Local(s, _) => self.read_local(*s),
            Place::Global(g, t) => self.read_global(*g, *t),
            Place::Field(o, i, t) => Expr::new(ExprKind::GetField(Box::new(o.clone()), *i), *t),
            Place::Index(o, i, t, l) => Expr::new(ExprKind::Index(Box::new(o.clone()), Box::new(i.clone()), *l), *t),
            Place::MapKey(m, k, t) => Expr::new(ExprKind::Rt(RtFn::MapGet, vec![m.clone(), k.clone(), Expr::int(u32::MAX as i64)]), *t),
            Place::Error => Self::err_expr(),
        }
    }

    pub fn write_place(&mut self, p: Place, v: Expr, span: Span) -> Expr {
        match p {
            Place::Local(s, t) => {
                let vt = v.ty;
                let v = self.coerce(v, t, span);
                self.narrow_after_assign(s, vt);
                Expr::new(ExprKind::SetLocal(s, Box::new(v)), t)
            }
            Place::Global(g, t) => {
                let vt = v.ty;
                let v = self.coerce(v, t, span);
                self.narrow_after_assign(GLOBAL_KEY + g, vt);
                Expr::new(ExprKind::SetGlobal(g, Box::new(v)), t)
            }
            Place::Field(o, i, t) => {
                let v = self.coerce(v, t, span);
                Expr::new(ExprKind::SetField(Box::new(o), i, Box::new(v)), t)
            }
            Place::Index(o, i, t, l) => {
                let v = self.coerce(v, t, span);
                Expr::new(ExprKind::SetIndex(Box::new(o), Box::new(i), Box::new(v), l), t)
            }
            Place::MapKey(m, k, t) => {
                let v = self.coerce(v, t, span);
                Expr::new(ExprKind::Rt(RtFn::MapSet, vec![m, k, v]), t)
            }
            Place::Error => Self::err_expr(),
        }
    }

    fn place_ty(p: &Place) -> Option<TyId> {
        match p {
            Place::Local(_, t) | Place::Global(_, t) | Place::Field(_, _, t) | Place::Index(_, _, t, _) | Place::MapKey(_, _, t) => Some(*t),
            Place::Error => None,
        }
    }

    fn assign(&mut self, target: &ast::Expr, op: Option<AOp>, value: &ast::Expr, span: Span) -> Expr {
        let mut pre = Vec::new();
        let p = self.place(target, op.is_some(), &mut pre);
        let pt = Self::place_ty(&p);
        let v = match op {
            None => {
                let v = self.expr(value, pt);
                if v.ty == T_VOID {
                    self.error(value.span, "cannot assign the result of a function that returns nothing");
                }
                v
            }
            Some(op) => {
                let cur = self.read_place(&p);
                let rexp = if self.types.is_numeric(cur.ty) { Some(cur.ty) } else { None };
                let r = self.expr(value, rexp);
                self.arith(op, cur, r, span)
            }
        };
        let w = self.write_place(p, v, value.span);
        if pre.is_empty() {
            w
        } else {
            let t = w.ty;
            Expr::new(ExprKind::Seq(pre, Box::new(w)), t)
        }
    }

    pub fn check_args(&mut self, params: &[TyId], args: &[ast::Expr], recv: Option<Expr>, span: Span, what: &str) -> Vec<Expr> {
        let mut out = Vec::new();
        let offset = if recv.is_some() { 1 } else { 0 };
        if let Some(r) = recv {
            let rs = span;
            let r = self.coerce(r, params[0], rs);
            out.push(r);
        }
        let expected = params.len() - offset.min(params.len());
        if args.len() != expected {
            self.error(span, format!("{} expects {} argument{} but got {}", what, expected, if expected == 1 { "" } else { "s" }, args.len()));
        }
        for (i, a) in args.iter().enumerate() {
            match params.get(i + offset) {
                Some(t) => out.push(self.expr_to(a, *t)),
                None => {
                    self.expr(a, None);
                }
            }
        }
        while out.len() < params.len() {
            out.push(Self::err_expr());
        }
        out
    }

    pub fn direct_call(&mut self, fid: FuncId, recv: Option<Expr>, args: &[ast::Expr], span: Span, name_span: Span) -> Expr {
        let ret = self.func_ret(fid);
        let info = &self.funcs[fid as usize];
        let params: Vec<TyId> = info.params.iter().map(|p| p.1).collect();
        let name = info.name.clone();
        let is_async = info.is_async;
        let private = info.private;
        let owner = info.self_ty.or_else(|| {
            let n = info.name.split('.').next().unwrap_or("");
            if info.name.contains('.') {
                self.mods[info.module].types.get(n).map(|e| e.sym)
            } else {
                None
            }
        });
        let fspan = info.span;
        if private && owner.is_some() && self.fx.last().and_then(|c| c.self_ty).or_else(|| self.static_owner()) != owner {
            self.error(name_span, format!("method `{}` is private", name));
        }
        self.def_link(name_span, fspan);
        let sig = {
            let info = &self.funcs[fid as usize];
            let ps: Vec<String> = info.params.iter().filter(|p| p.0 != "self").map(|p| format!("{}: {}", p.0, self.types.display(p.1))).collect();
            let r = if ret == T_VOID { String::new() } else { format!(": {}", self.types.display(ret)) };
            format!("{}fun {}({}){}", if is_async { "async " } else { "" }, name, ps.join(", "), r)
        };
        self.hover(name_span, sig);
        let hargs = self.check_args(&params, args, recv, span, &format!("`{}`", name));
        self.invalidate_globals();
        if is_async {
            let ft = self.types.future(ret);
            Expr::new(ExprKind::Spawn(fid, hargs), ft)
        } else {
            Expr::new(ExprKind::Call(fid, hargs), ret)
        }
    }

    fn static_owner(&self) -> Option<TyId> {
        let c = self.fx.last()?;
        let info = &self.funcs[c.func as usize];
        if info.name.contains('.') {
            let n = info.name.split('.').next().unwrap_or("");
            return self.mods[info.module].types.get(n).map(|e| e.sym);
        }
        None
    }

    fn indirect_call(&mut self, callee: Expr, args: &[ast::Expr], span: Span) -> Expr {
        match self.types.get(callee.ty).clone() {
            Ty::Func(ps, r) => {
                let hargs = self.check_args(&ps, args, None, span, "this function");
                self.invalidate_globals();
                Expr::new(ExprKind::CallIndirect(Box::new(callee), hargs), r)
            }
            Ty::Error => {
                for a in args {
                    self.expr(a, None);
                }
                Self::err_expr()
            }
            Ty::Optional(_) => {
                self.error_note(span, "this function value may be null", "check it with `!= null` first");
                Self::err_expr()
            }
            _ => {
                let s = self.show(callee.ty);
                self.error(span, format!("a value of type {} cannot be called", s));
                Self::err_expr()
            }
        }
    }

    fn type_ident(&mut self, e: &ast::Expr) -> Option<TyId> {
        if let A::Ident(n) = &e.kind {
            if self.lookup_local(n).is_some() || self.self_field(n).is_some() {
                return None;
            }
            let m = self.cur_module();
            if self.lookup_value_entry(m, n).is_some() {
                return None;
            }
            return self.lookup_type_name(m, n, e.span);
        }
        None
    }

    fn call(&mut self, callee: &ast::Expr, args: &[ast::Expr], span: Span, expected: Option<TyId>) -> Expr {
        match &callee.kind {
            A::Ident(name) => {
                if let Some(l) = self.lookup_local(name) {
                    self.def_link(callee.span, l.span);
                    let c = self.read_local(l.slot);
                    return self.indirect_call(c, args, span);
                }
                if let Some(st) = self.fx.last().and_then(|c| c.self_ty) {
                    if let Some(rec) = self.types.record_of(st) {
                        if let Some(fid) = rec.methods.get(name).copied() {
                            let s = Expr::new(ExprKind::Local(0), st);
                            return self.direct_call(fid, Some(s), args, span, callee.span);
                        }
                        if let Some(fid) = rec.statics.get(name).copied() {
                            return self.direct_call(fid, None, args, span, callee.span);
                        }
                    }
                    if self.self_field(name).is_some() {
                        let c = self.ident_expr(name, callee.span);
                        return self.indirect_call(c, args, span);
                    }
                }
                if let Some(owner) = self.static_owner() {
                    if let Some(rec) = self.types.record_of(owner) {
                        if let Some(fid) = rec.statics.get(name).copied() {
                            return self.direct_call(fid, None, args, span, callee.span);
                        }
                    }
                }
                let m = self.cur_module();
                match self.lookup_value_entry(m, name).map(|e| e.sym) {
                    Some(ValSym::Func(f)) => return self.direct_call(f, None, args, span, callee.span),
                    Some(ValSym::Global(_)) => {
                        let c = self.ident_expr(name, callee.span);
                        return self.indirect_call(c, args, span);
                    }
                    None => {}
                }
                if let Some(t) = self.lookup_type_name(m, name, callee.span) {
                    return self.construct(t, args, span, callee.span);
                }
                if let Some(e) = self.builtin(name, None, args, span, expected) {
                    return e;
                }
                self.ident_expr(name, callee.span);
                for a in args {
                    self.expr(a, None);
                }
                Self::err_expr()
            }
            A::Field { obj, name } => self.method_call(obj, name, args, span, expected),
            _ => {
                let c = self.expr(callee, None);
                self.indirect_call(c, args, span)
            }
        }
    }

    fn construct(&mut self, t: TyId, args: &[ast::Expr], span: Span, name_span: Span) -> Expr {
        let ri = match self.types.get(t) {
            Ty::Record(r) => *r,
            Ty::Error => return Self::err_expr(),
            _ => {
                let s = self.show(t);
                self.error(name_span, format!("`{}` cannot be constructed like a function", s));
                return Self::err_expr();
            }
        };
        let rec = self.types.records[ri as usize].clone();
        self.hover(name_span, format!("{} {}", if rec.is_class { "class" } else { "type" }, rec.name));
        if let Some(init) = rec.init {
            let mut fields = Vec::new();
            for f in &rec.fields {
                let v = match &f.default {
                    Some(d) => self.default_expr(d, f.ty, rec.module as usize),
                    None => self.zero(f.ty).unwrap_or(Expr::new(ExprKind::Null, f.ty)),
                };
                fields.push(v);
            }
            let slot = self.new_local(t);
            let alloc = Expr::new(ExprKind::SetLocal(slot, Box::new(Expr::new(ExprKind::NewStruct(t, fields), t))), t);
            let call = self.direct_call(init, Some(Expr::new(ExprKind::Local(slot), t)), args, span, name_span);
            return Expr::new(ExprKind::Seq(vec![Stmt::Expr(alloc), Stmt::Expr(call)], Box::new(Expr::new(ExprKind::Local(slot), t))), t);
        }
        if args.len() > rec.fields.len() {
            self.error(span, format!("`{}` has {} field{} but {} arguments were given", rec.name, rec.fields.len(), if rec.fields.len() == 1 { "" } else { "s" }, args.len()));
        }
        let mut vals = Vec::new();
        for (i, f) in rec.fields.iter().enumerate() {
            if let Some(a) = args.get(i) {
                vals.push(self.expr_to(a, f.ty));
            } else if let Some(d) = &f.default {
                let d = d.clone();
                vals.push(self.default_expr(&d, f.ty, rec.module as usize));
            } else if let Some(z) = self.zero(f.ty) {
                vals.push(z);
            } else {
                self.error(span, format!("missing value for field `{}` of `{}`", f.name, rec.name));
                vals.push(Self::err_expr());
            }
        }
        Expr::new(ExprKind::NewStruct(t, vals), t)
    }

    fn default_expr(&mut self, d: &ast::Expr, t: TyId, module: usize) -> Expr {
        let saved = self.ctx().module;
        self.ctx().module = module;
        let h = self.expr_to(d, t);
        self.ctx().module = saved;
        h
    }

    fn method_call(&mut self, obj: &ast::Expr, name: &ast::Ident, args: &[ast::Expr], span: Span, expected: Option<TyId>) -> Expr {
        if let Some(t) = self.type_ident(obj) {
            match self.types.get(t).clone() {
                Ty::Record(ri) => {
                    let rec = self.types.records[ri as usize].clone();
                    if let Some(fid) = rec.statics.get(&name.name) {
                        return self.direct_call(*fid, None, args, span, name.span);
                    }
                    if let Some(fid) = rec.methods.get(&name.name) {
                        return self.direct_call(*fid, None, args, span, name.span);
                    }
                    if let Some(i) = rec.field_index(&name.name) {
                        let _ = i;
                        self.error(name.span, format!("`{}` is a field; it needs an instance of {}", name.name, rec.name));
                    } else {
                        let cands: Vec<&str> = rec.statics.keys().chain(rec.methods.keys()).map(|s| s.as_str()).collect();
                        match suggest(&name.name, cands.into_iter()) {
                            Some(s) => self.error_note(name.span, format!("{} has no method `{}`", rec.name, name.name), format!("did you mean `{}`?", s)),
                            None => self.error(name.span, format!("{} has no method `{}`", rec.name, name.name)),
                        }
                    }
                }
                Ty::Error => {}
                _ => {
                    let s = self.show(t);
                    self.error(name.span, format!("{} has no method `{}`", s, name.name));
                }
            }
            for a in args {
                self.expr(a, None);
            }
            return Self::err_expr();
        }
        let o = self.expr(obj, None);
        let t = o.ty;
        match self.types.get(t).clone() {
            Ty::Record(ri) => {
                let rec = self.types.records[ri as usize].clone();
                if let Some(fid) = rec.methods.get(&name.name) {
                    return self.direct_call(*fid, Some(o), args, span, name.span);
                }
                if let Some(i) = rec.field_index(&name.name) {
                    if matches!(self.types.get(rec.fields[i].ty), Ty::Func(..)) {
                        self.check_field_access(&rec, i, name.span);
                        let f = Expr::new(ExprKind::GetField(Box::new(o), i as u32), rec.fields[i].ty);
                        return self.indirect_call(f, args, span);
                    }
                }
                if rec.statics.contains_key(&name.name) {
                    self.error(name.span, format!("`{}` is a static method; call it as `{}.{}(...)`", name.name, rec.name, name.name));
                    return Self::err_expr();
                }
            }
            Ty::Interface(ii) => {
                let iface = self.types.ifaces[ii as usize].clone();
                if let Some(m) = iface.methods.iter().find(|m| m.name == name.name) {
                    if m.is_async {
                        self.error(name.span, "async interface methods cannot be called through the interface yet");
                    }
                    let mut ps = vec![t];
                    ps.extend(m.params.iter().copied());
                    self.def_link(name.span, m.span);
                    let sig = self.method_sig_str(&m.params, m.ret, m.is_async);
                    self.hover(name.span, format!("{}.{}: {}", iface.name, m.name, sig));
                    let hargs = self.check_args(&ps, args, Some(o), span, &format!("`{}`", m.name));
                    self.invalidate_globals();
                    return Expr::new(ExprKind::CallIface(m.slot, hargs), m.ret);
                }
                let s = self.show(t);
                self.error(name.span, format!("interface {} has no method `{}`", s, name.name));
                return Self::err_expr();
            }
            Ty::Optional(_) => {
                self.error_note(obj.span, format!("cannot call `{}` on a value that may be null", name.name), "check it with `if (x != null)` first, or use `x!!`");
                for a in args {
                    self.expr(a, None);
                }
                return Self::err_expr();
            }
            Ty::Error => {
                for a in args {
                    self.expr(a, None);
                }
                return Self::err_expr();
            }
            _ => {}
        }
        if let Some(e) = self.builtin(&name.name, Some((o.clone(), obj.span)), args, span, expected) {
            return e;
        }
        let m = self.cur_module();
        if let Some(Entry { sym: ValSym::Func(f), .. }) = self.lookup_value_entry(m, &name.name) {
            let first = self.funcs[f as usize].params.first().map(|p| p.1);
            if let Some(pt) = first {
                if pt == t || self.types.implements(t, pt) || pt == T_ANY {
                    return self.direct_call(f, Some(o), args, span, name.span);
                }
            }
        }
        let s = self.show(t);
        if t == T_ANY {
            self.error_note(name.span, format!("cannot call `{}` on a value of type any", name.name), "narrow it with `is` or cast it with `as` first");
        } else {
            self.error(name.span, format!("{} has no method `{}`", s, name.name));
        }
        for a in args {
            self.expr(a, None);
        }
        Self::err_expr()
    }

    pub fn check_field_access(&mut self, rec: &RecordDef, i: usize, span: Span) {
        if rec.fields[i].private {
            let inside = self.fx.last().and_then(|c| c.self_ty) == Some(rec.ty) || self.static_owner() == Some(rec.ty);
            if !inside {
                self.error(span, format!("field `{}` of {} is private", rec.fields[i].name, rec.name));
            }
        }
    }

    fn field(&mut self, obj: &ast::Expr, name: &ast::Ident, span: Span) -> Expr {
        if let Some(t) = self.type_ident(obj) {
            match self.types.get(t).clone() {
                Ty::Enum(ei) => {
                    let en = self.types.enums[ei as usize].clone();
                    if let Some(i) = en.variants.iter().position(|v| v.0 == name.name) {
                        self.def_link(name.span, en.variants[i].1);
                        self.hover(name.span, format!("{}.{} = {}", en.name, name.name, i));
                        return Expr::new(ExprKind::Int(i as i64), t);
                    }
                    let cands: Vec<&str> = en.variants.iter().map(|v| v.0.as_str()).collect();
                    match suggest(&name.name, cands.into_iter()) {
                        Some(s) => self.error_note(name.span, format!("enum {} has no variant `{}`", en.name, name.name), format!("did you mean `{}`?", s)),
                        None => self.error(name.span, format!("enum {} has no variant `{}`", en.name, name.name)),
                    }
                    return Self::err_expr();
                }
                Ty::Record(ri) => {
                    let rec = self.types.records[ri as usize].clone();
                    if let Some(fid) = rec.statics.get(&name.name) {
                        let ft = self.func_type(*fid);
                        return Expr::new(ExprKind::FuncRef(*fid), ft);
                    }
                    self.error(name.span, format!("{} has no static member `{}`", rec.name, name.name));
                    return Self::err_expr();
                }
                Ty::Error => return Self::err_expr(),
                _ => {
                    let s = self.show(t);
                    self.error(name.span, format!("{} has no member `{}`", s, name.name));
                    return Self::err_expr();
                }
            }
        }
        let o = self.expr(obj, None);
        let t = o.ty;
        let n = name.name.as_str();
        match self.types.get(t).clone() {
            Ty::Record(ri) => {
                let rec = self.types.records[ri as usize].clone();
                if let Some(i) = rec.field_index(n) {
                    self.check_field_access(&rec, i, name.span);
                    self.def_link(name.span, rec.fields[i].span);
                    let ts = self.show(rec.fields[i].ty);
                    self.hover(name.span, format!("(field) {}.{}: {}", rec.name, n, ts));
                    return Expr::new(ExprKind::GetField(Box::new(o), i as u32), rec.fields[i].ty);
                }
                if rec.methods.contains_key(n) {
                    self.error(name.span, format!("`{}` is a method; call it with `{}()`", n, n));
                    return Self::err_expr();
                }
                let cands: Vec<&str> = rec.fields.iter().map(|f| f.name.as_str()).collect();
                match suggest(n, cands.into_iter()) {
                    Some(s) => self.error_note(name.span, format!("{} has no field `{}`", self.show(t), n), format!("did you mean `{}`?", s)),
                    None => self.error(name.span, format!("{} has no field `{}`", self.show(t), n)),
                }
                Self::err_expr()
            }
            Ty::Str if matches!(n, "length" | "size" | "len") => Expr::new(ExprKind::Rt(RtFn::StrLen, vec![o]), T_INT),
            Ty::Array(_) if matches!(n, "length" | "size" | "len") => Expr::new(ExprKind::ArrLen(Box::new(o)), T_INT),
            Ty::Map(..) if matches!(n, "length" | "size" | "len") => Expr::new(ExprKind::Rt(RtFn::MapLen, vec![o]), T_INT),
            Ty::Map(k, v) if matches!(n, "keys" | "values") => {
                let et = if n == "keys" { k } else { v };
                let at = self.types.array(et);
                let f = if n == "keys" { RtFn::MapKeys } else { RtFn::MapValues };
                Expr::new(ExprKind::Rt(f, vec![o, Self::tid(at)]), at)
            }
            Ty::Any => {
                let key = self.str_lit(n);
                let l = self.loc_expr(span);
                Expr::new(ExprKind::Rt(RtFn::AnyIndex, vec![o, key, Self::tid(T_STR), l]), T_ANY)
            }
            Ty::Optional(_) => {
                self.error_note(obj.span, format!("cannot read `{}` from a value that may be null", n), "check it with `if (x != null)` first, or use `x!!`");
                Self::err_expr()
            }
            Ty::Error => Self::err_expr(),
            _ => {
                let s = self.show(t);
                self.error(name.span, format!("{} has no field `{}`", s, n));
                Self::err_expr()
            }
        }
    }

    fn index(&mut self, obj: &ast::Expr, index: &ast::Expr, span: Span) -> Expr {
        let o = self.expr(obj, None);
        match self.types.get(o.ty).clone() {
            Ty::Array(e) => {
                let i = self.expr_to(index, T_INT);
                let l = self.loc(span);
                Expr::new(ExprKind::Index(Box::new(o), Box::new(i), l), e)
            }
            Ty::Str => {
                let i = self.expr_to(index, T_INT);
                let l = self.loc_expr(span);
                Expr::new(ExprKind::Rt(RtFn::StrIndex, vec![o, i, l]), T_STR)
            }
            Ty::Map(k, v) => {
                let key = self.expr_to(index, k);
                let l = self.loc_expr(span);
                Expr::new(ExprKind::Rt(RtFn::MapGet, vec![o, key, l]), v)
            }
            Ty::Any => {
                let i = self.expr(index, None);
                let it = i.ty;
                let l = self.loc_expr(span);
                Expr::new(ExprKind::Rt(RtFn::AnyIndex, vec![o, i, Self::tid(it), l]), T_ANY)
            }
            Ty::Error => {
                self.expr(index, None);
                Self::err_expr()
            }
            Ty::Optional(_) => {
                self.error_note(obj.span, "cannot index a value that may be null", "check it with `if (x != null)` first");
                Self::err_expr()
            }
            _ => {
                let s = self.show(o.ty);
                self.error(span, format!("a value of type {} cannot be indexed", s));
                Self::err_expr()
            }
        }
    }

    pub fn join_types(&mut self, ts: &[TyId]) -> TyId {
        let mut cur: Option<TyId> = None;
        let mut nullable = false;
        for &t in ts {
            if t == T_NULL {
                nullable = true;
                continue;
            }
            if t == T_ERROR {
                continue;
            }
            cur = Some(match cur {
                None => t,
                Some(c) if c == t => c,
                Some(c) if self.types.is_numeric(c) && self.types.is_numeric(t) => T_FLOAT,
                Some(c) => {
                    let common = self.common_iface(c, t);
                    match common {
                        Some(i) => i,
                        None => {
                            if self.types.unwrap_optional(c) == t {
                                c
                            } else if self.types.unwrap_optional(t) == c {
                                t
                            } else {
                                T_ANY
                            }
                        }
                    }
                }
            });
        }
        match cur {
            None => T_ANY,
            Some(t) if nullable => self.types.optional(t),
            Some(t) => t,
        }
    }

    fn common_iface(&self, a: TyId, b: TyId) -> Option<TyId> {
        let ia: Vec<u32> = match self.types.get(a) {
            Ty::Record(r) => self.types.records[*r as usize].implements.clone(),
            Ty::Interface(i) => vec![*i],
            _ => return None,
        };
        for i in ia {
            let it = self.types.ifaces[i as usize].ty;
            if (a == it || self.types.implements(a, it)) && (b == it || self.types.implements(b, it)) {
                return Some(it);
            }
        }
        None
    }

    fn array_lit(&mut self, items: &[ast::Expr], expected: Option<TyId>, _span: Span) -> Expr {
        let et = match expected.map(|t| self.types.get(t).clone()) {
            Some(Ty::Array(e)) => Some(e),
            Some(Ty::Optional(o)) => match self.types.get(o).clone() {
                Ty::Array(e) => Some(e),
                _ => None,
            },
            _ => None,
        };
        if let Some(et) = et {
            let elems: Vec<Expr> = items.iter().map(|i| self.expr_to(i, et)).collect();
            let at = self.types.array(et);
            return Expr::new(ExprKind::NewArray(at, elems), at);
        }
        let hs: Vec<Expr> = items.iter().map(|i| self.expr(i, None)).collect();
        let tys: Vec<TyId> = hs.iter().map(|h| h.ty).collect();
        let et = if hs.is_empty() { T_ANY } else { self.join_types(&tys) };
        if et == T_VOID {
            self.error(items[0].span, "arrays cannot contain void values");
        }
        let mut elems = Vec::new();
        for (h, it) in hs.into_iter().zip(items.iter()) {
            elems.push(self.coerce(h, et, it.span));
        }
        let at = self.types.array(et);
        Expr::new(ExprKind::NewArray(at, elems), at)
    }

    fn visible_records(&self) -> Vec<TyId> {
        let m = self.cur_module();
        let mut out = Vec::new();
        let mut mods = vec![m];
        mods.extend(self.mods[m].imports.iter().copied());
        for mi in mods {
            let mut names: Vec<(&String, &Entry<TyId>)> = self.mods[mi].types.iter().collect();
            names.sort_by_key(|(_, e)| (e.span.file, e.span.start));
            for (_, e) in names {
                if (mi == m || e.vis != Vis::Priv) && !out.contains(&e.sym) {
                    out.push(e.sym);
                }
            }
        }
        out
    }

    fn struct_lit(&mut self, ty: Option<&ast::Ident>, fields: &[(ast::Ident, ast::Expr)], expected: Option<TyId>, span: Span) -> Expr {
        let mut seen: HashMap<&str, Span> = HashMap::new();
        for (n, _) in fields {
            if seen.insert(&n.name, n.span).is_some() {
                self.error(n.span, format!("field `{}` is given twice", n.name));
            }
        }
        let target = if let Some(tn) = ty {
            let m = self.cur_module();
            match self.lookup_type_name(m, &tn.name, tn.span) {
                Some(t) => Some(t),
                None => {
                    let te = TypeExpr { kind: TypeExprKind::Named(tn.name.clone(), vec![]), span: tn.span };
                    let t = self.resolve_type(&te);
                    Some(t)
                }
            }
        } else {
            expected.map(|t| {
                let u = self.types.unwrap_optional(t);
                if matches!(self.types.get(u), Ty::Record(_) | Ty::Map(..)) {
                    u
                } else {
                    t
                }
            })
        };
        if let Some(t) = target {
            match self.types.get(t).clone() {
                Ty::Record(ri) => return self.build_record(ri, fields, span),
                Ty::Map(k, v) if ty.is_none() => {
                    if k != T_STR && k != T_ANY {
                        let s = self.show(t);
                        self.error(span, format!("cannot use field syntax for a map of type {}", s));
                    }
                    let pairs: Vec<(Expr, Expr)> = fields
                        .iter()
                        .map(|(n, e)| {
                            let key = self.str_lit(&n.name);
                            let key = self.coerce(key, k, n.span);
                            (key, self.expr_to(e, v))
                        })
                        .collect();
                    return self.build_map(t, pairs);
                }
                Ty::Error => {
                    for (_, e) in fields {
                        self.expr(e, None);
                    }
                    return Self::err_expr();
                }
                Ty::Any | Ty::Interface(_) | Ty::Optional(_) if ty.is_none() => {}
                _ => {
                    if ty.is_some() {
                        let s = self.show(t);
                        self.error(span, format!("{} is not a record type and cannot be built with `{{ ... }}`", s));
                        for (_, e) in fields {
                            self.expr(e, None);
                        }
                        return Self::err_expr();
                    }
                }
            }
        }
        if fields.is_empty() {
            self.error(span, "cannot infer the type of `{}`; add a type annotation");
            return Self::err_expr();
        }
        let hs: Vec<(String, Expr, Span)> = fields.iter().map(|(n, e)| (n.name.clone(), self.expr(e, None), e.span)).collect();
        let mut matches: Vec<u32> = Vec::new();
        for t in self.visible_records() {
            if let Ty::Record(ri) = self.types.get(t) {
                let rec = &self.types.records[*ri as usize];
                if rec.is_class || rec.anon || rec.fields.len() != hs.len() {
                    continue;
                }
                let ok = hs.iter().all(|(n, h, _)| match rec.field_index(n) {
                    Some(i) => {
                        let ft = rec.fields[i].ty;
                        h.ty == ft || h.ty == T_ERROR || (h.ty == T_INT && ft == T_FLOAT) || (h.ty == T_NULL && self.types.is_nullable(ft)) || ft == T_ANY || self.types.unwrap_optional(ft) == h.ty
                    }
                    None => false,
                });
                if ok {
                    matches.push(*ri);
                }
            }
        }
        if matches.len() == 1 {
            let ri = matches[0];
            let rec = self.types.records[ri as usize].clone();
            let mut vals: Vec<Option<Expr>> = vec![None; rec.fields.len()];
            for (n, h, sp) in hs {
                let i = rec.field_index(&n).unwrap();
                vals[i] = Some(self.coerce(h, rec.fields[i].ty, sp));
            }
            let t = rec.ty;
            self.hover(span, format!("type {}", rec.name));
            return Expr::new(ExprKind::NewStruct(t, vals.into_iter().map(|v| v.unwrap()).collect()), t);
        }
        let mut key: Vec<(String, TyId)> = Vec::new();
        let mut vals = Vec::new();
        for (n, h, sp) in hs {
            let t = if h.ty == T_NULL { T_ANY } else { h.ty };
            if t == T_VOID {
                self.error(sp, "a field cannot hold a void value");
            }
            let h = self.coerce(h, t, sp);
            key.push((n, t));
            vals.push(h);
        }
        let t = match self.anon.get(&key) {
            Some(t) => *t,
            None => {
                let m = self.cur_module();
                let ri = self.types.new_record(RecordDef {
                    name: String::new(),
                    fields: key.iter().map(|(n, t)| FieldDef { name: n.clone(), ty: *t, default: None, private: false, span }).collect(),
                    is_class: false,
                    anon: true,
                    implements: vec![],
                    methods: HashMap::new(),
                    statics: HashMap::new(),
                    init: None,
                    module: m as u32,
                    span,
                    ty: 0,
                });
                let t = self.types.records[ri as usize].ty;
                self.anon.insert(key, t);
                t
            }
        };
        Expr::new(ExprKind::NewStruct(t, vals), t)
    }

    fn build_record(&mut self, ri: u32, fields: &[(ast::Ident, ast::Expr)], span: Span) -> Expr {
        let rec = self.types.records[ri as usize].clone();
        let mut vals: Vec<Option<Expr>> = vec![None; rec.fields.len()];
        for (n, e) in fields {
            match rec.field_index(&n.name) {
                Some(i) => {
                    self.check_field_access(&rec, i, n.span);
                    self.def_link(n.span, rec.fields[i].span);
                    vals[i] = Some(self.expr_to(e, rec.fields[i].ty));
                }
                None => {
                    let cands: Vec<&str> = rec.fields.iter().map(|f| f.name.as_str()).collect();
                    let rn = self.show(rec.ty);
                    match suggest(&n.name, cands.into_iter()) {
                        Some(s) => self.error_note(n.span, format!("{} has no field `{}`", rn, n.name), format!("did you mean `{}`?", s)),
                        None => self.error(n.span, format!("{} has no field `{}`", rn, n.name)),
                    }
                    self.expr(e, None);
                }
            }
        }
        let mut out = Vec::new();
        let mut missing = Vec::new();
        for (i, v) in vals.into_iter().enumerate() {
            match v {
                Some(v) => out.push(v),
                None => {
                    let f = &rec.fields[i];
                    if let Some(d) = &f.default {
                        let d = d.clone();
                        out.push(self.default_expr(&d, f.ty, rec.module as usize));
                    } else if let Some(z) = self.zero(f.ty) {
                        out.push(z);
                    } else {
                        missing.push(f.name.clone());
                        out.push(Self::err_expr());
                    }
                }
            }
        }
        if !missing.is_empty() {
            let rn = self.show(rec.ty);
            self.error(span, format!("missing field{} {} in {}", if missing.len() == 1 { "" } else { "s" }, missing.iter().map(|m| format!("`{}`", m)).collect::<Vec<_>>().join(", "), rn));
        }
        Expr::new(ExprKind::NewStruct(rec.ty, out), rec.ty)
    }

    pub fn build_map(&mut self, t: TyId, pairs: Vec<(Expr, Expr)>) -> Expr {
        let slot = self.new_local(t);
        let mut stmts = vec![Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(Expr::new(ExprKind::Rt(RtFn::MapNew, vec![Self::tid(t)]), t))), t))];
        for (k, v) in pairs {
            let vt = v.ty;
            stmts.push(Stmt::Expr(Expr::new(ExprKind::Rt(RtFn::MapSet, vec![Expr::new(ExprKind::Local(slot), t), k, v]), vt)));
        }
        Expr::new(ExprKind::Seq(stmts, Box::new(Expr::new(ExprKind::Local(slot), t))), t)
    }

    fn map_lit(&mut self, pairs: &[(ast::Expr, ast::Expr)], expected: Option<TyId>, span: Span) -> Expr {
        let kv = match expected.map(|t| self.types.unwrap_optional(t)).map(|t| self.types.get(t).clone()) {
            Some(Ty::Map(k, v)) => Some((k, v)),
            _ => None,
        };
        let (k, v, pre): (TyId, TyId, Vec<(Expr, Expr)>) = match kv {
            Some((k, v)) => (k, v, pairs.iter().map(|(a, b)| (self.expr_to(a, k), self.expr_to(b, v))).collect()),
            None => {
                let hs: Vec<(Expr, Expr)> = pairs.iter().map(|(a, b)| (self.expr(a, None), self.expr(b, None))).collect();
                let kts: Vec<TyId> = hs.iter().map(|p| p.0.ty).collect();
                let vts: Vec<TyId> = hs.iter().map(|p| p.1.ty).collect();
                let k = self.join_types(&kts);
                let v = self.join_types(&vts);
                self.check_map_key(k, span);
                let mut out = Vec::new();
                for ((a, b), (ea, eb)) in hs.into_iter().zip(pairs.iter()) {
                    out.push((self.coerce(a, k, ea.span), self.coerce(b, v, eb.span)));
                }
                (k, v, out)
            }
        };
        let t = self.types.map_of(k, v);
        self.build_map(t, pre)
    }

    pub fn is_check(&mut self, h: Expr, t: TyId, span: Span) -> Expr {
        let from = h.ty;
        if from == T_ERROR || t == T_ERROR {
            return Expr::new(ExprKind::Bool(false), T_BOOL);
        }
        if t == T_NULL {
            if !self.types.is_nullable(from) {
                let s = self.show(from);
                self.error(span, format!("a value of type {} can never be null", s));
            }
            return Expr::new(ExprKind::Binary(BinOp::ICmp(Cmp::Eq), Box::new(h), Box::new(Expr::new(ExprKind::Null, from))), T_BOOL);
        }
        if from == t {
            self.warn(span, "this check is always true");
            return if h.has_side_effects() { Expr::new(ExprKind::Seq(vec![Stmt::Expr(h)], Box::new(Expr::new(ExprKind::Bool(true), T_BOOL))), T_BOOL) } else { Expr::new(ExprKind::Bool(true), T_BOOL) };
        }
        let ok = match self.types.get(from).clone() {
            Ty::Any => true,
            Ty::Optional(x) => x == t || self.types.implements(t, x),
            Ty::Interface(_) => self.types.implements(t, from),
            _ => false,
        };
        if !ok {
            let (a, b) = (self.show(from), self.show(t));
            self.error(span, format!("a value of type {} can never be {}", a, b));
            return Expr::new(ExprKind::Bool(false), T_BOOL);
        }
        Expr::new(ExprKind::Rt(RtFn::IsType, vec![h, Self::tid(from), Self::tid(t)]), T_BOOL)
    }

    fn is_expr(&mut self, x: &ast::Expr, te: &TypeExpr, span: Span) -> Expr {
        let h = self.expr(x, None);
        let t = self.resolve_type(te);
        self.is_check(h, t, span)
    }

    fn as_expr(&mut self, x: &ast::Expr, te: &TypeExpr, span: Span) -> Expr {
        let t = self.resolve_type(te);
        let exp = if self.types.is_numeric(t) { None } else { Some(t) };
        let h = self.expr(x, exp);
        let from = h.ty;
        if from == t || t == T_ERROR || from == T_ERROR {
            return Self::retype(h, if t == T_ERROR { from } else { t });
        }
        let fk = self.types.get(from).clone();
        let tk = self.types.get(t).clone();
        match (&fk, &tk) {
            (Ty::Float, Ty::Int) => return Expr::new(ExprKind::Conv(Conv::FloatToInt, Box::new(h)), T_INT),
            (Ty::Int, Ty::Float) => return self.coerce(h, T_FLOAT, span),
            (Ty::Bool, Ty::Int) | (Ty::Enum(_), Ty::Int) | (Ty::Int, Ty::Enum(_)) => return Self::retype(h, t),
            (_, Ty::Str) if !matches!(fk, Ty::Any | Ty::Optional(_)) => return self.to_str(h),
            _ => {}
        }
        let h = match self.try_coerce(h, t) {
            Ok(x) => return x,
            Err(h) => h,
        };
        let down = match &fk {
            Ty::Any => true,
            Ty::Optional(x) => *x == t || self.types.implements(t, *x),
            Ty::Interface(_) => self.types.implements(t, from),
            _ => false,
        };
        if down {
            let l = self.loc_expr(span);
            return Expr::new(ExprKind::Rt(RtFn::Cast, vec![h, Self::tid(from), Self::tid(t), l]), t);
        }
        let (a, b) = (self.show(from), self.show(t));
        self.error(span, format!("cannot cast {} to {}", a, b));
        Self::err_expr()
    }

    fn lambda(&mut self, f: &ast::FunDecl) -> Expr {
        let key = (f.span.file, f.span.start);
        if let Some(fid) = self.lambdas.get(&key).copied() {
            let t = self.func_type(fid);
            return Expr::new(ExprKind::FuncRef(fid), t);
        }
        if f.is_async {
            self.error(f.span, "async lambdas are not supported yet");
        }
        let m = self.cur_module();
        if self.is_dry() {
            let ps: Vec<TyId> = f.params.iter().map(|p| self.resolve_type_in(&p.ty, m)).collect();
            let r = f.ret.as_ref().map(|r| self.resolve_type_in(r, m)).unwrap_or(T_ANY);
            let t = self.types.func(ps, r);
            return Expr::new(ExprKind::Int(0), t);
        }
        let fid = self.declare_fun(m, f, None, false);
        let (line, _) = self.sm.file(f.span.file).line_col(f.span.start as usize);
        self.funcs[fid as usize].name = format!("<lambda:{}>", line);
        self.lambdas.insert(key, fid);
        self.check_func(fid);
        let t = self.func_type(fid);
        Expr::new(ExprKind::FuncRef(fid), t)
    }
}

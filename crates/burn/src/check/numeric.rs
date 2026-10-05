use super::*;
use crate::ast::BinOp as AOp;
use crate::hir::{BinOp, Cmp, Conv, UnOp};

impl<'a> Checker<'a> {
    pub fn plain_int_lit(h: &Expr) -> Option<i64> {
        match h.kind {
            ExprKind::Int(v) if h.ty == T_INT => Some(v),
            _ => None,
        }
    }

    pub fn int_widens(&self, from: TyId, to: TyId) -> bool {
        match (self.types.int_range(from), self.types.int_range(to)) {
            (Some((fb, fs)), Some((tb, ts))) => match (fs, ts) {
                (false, true) => fb < tb,
                (false, false) | (true, true) => fb <= tb,
                (true, false) => false,
            },
            _ => false,
        }
    }

    pub fn lit_value(&self, v: i64, t: TyId) -> i128 {
        if t == T_U64 {
            v as u64 as i128
        } else {
            v as i128
        }
    }

    pub fn fits_int(&self, v: i128, t: TyId) -> bool {
        match self.types.get(t) {
            Ty::Int => (i64::MIN as i128..=i64::MAX as i128).contains(&v),
            Ty::Num(n) if !n.is_float() => n.fits(v),
            _ => false,
        }
    }

    pub fn range_text(&self, t: TyId) -> String {
        match self.types.get(t) {
            Ty::Num(n) if !n.is_float() => format!("{} holds values from {} to {}", n.name(), n.min_value(), n.max_value()),
            _ => "int holds values from -9223372036854775808 to 9223372036854775807".into(),
        }
    }

    pub fn typed_lit(&mut self, v: i128, t: TyId, span: Span) -> Expr {
        if self.types.is_floating(t) {
            let f = if t == T_F32 { v as f64 as f32 as f64 } else { v as f64 };
            return Expr::new(ExprKind::Float(f), t);
        }
        if !self.fits_int(v, t) {
            let s = self.show(t);
            let help = self.range_text(t);
            self.emit(Diagnostic::error(span, format!("`{}` does not fit in {}", v, s)).help(help));
            return Self::err_expr();
        }
        Expr::new(ExprKind::Int(v as i64), t)
    }

    pub fn num_code(&self, t: TyId) -> i64 {
        match self.types.get(t) {
            Ty::Num(n) => *n as i64,
            _ => 0,
        }
    }

    pub fn fit_check(&mut self, h: Expr, t: TyId, span: Span) -> Expr {
        let loc = self.loc_expr(span);
        let code = self.num_code(t);
        Expr::new(ExprKind::Rt(RtFn::NumFit, vec![h, Expr::int(code), loc]), t)
    }

    pub fn wrap_to(&mut self, h: Expr, t: TyId) -> Expr {
        if let ExprKind::Int(v) = h.kind {
            if let Some(n) = self.types.num_of(t) {
                return Expr::new(ExprKind::Int(n.wrap(v as u64) as i64), t);
            }
        }
        let code = self.num_code(t);
        Expr::new(ExprKind::Rt(RtFn::NumWrap, vec![h, Expr::int(code)]), t)
    }

    pub fn f32_round(h: Expr) -> Expr {
        if let ExprKind::Float(f) = h.kind {
            return Expr::new(ExprKind::Float(f as f32 as f64), T_F32);
        }
        Expr::new(ExprKind::Rt(RtFn::F32Round, vec![h]), T_F32)
    }

    pub fn widen_num(&mut self, h: Expr, to: TyId) -> Option<Expr> {
        let from = h.ty;
        if self.types.is_integer(from) && self.types.is_integer(to) {
            if let Some(v) = Self::plain_int_lit(&h) {
                return self.fits_int(v as i128, to).then(|| Expr::new(ExprKind::Int(v), to));
            }
            return self.int_widens(from, to).then(|| Self::retype(h, to));
        }
        if self.types.is_integer(from) && self.types.is_floating(to) {
            let as_float = match h.kind {
                ExprKind::Int(v) => Expr::new(ExprKind::Float(self.lit_value(v, from) as f64), T_FLOAT),
                _ if from == T_U64 => Expr::new(ExprKind::Rt(RtFn::U2F, vec![h]), T_FLOAT),
                _ => Expr::new(ExprKind::Conv(Conv::IntToFloat, Box::new(h)), T_FLOAT),
            };
            return Some(if to == T_F32 { Self::f32_round(as_float) } else { as_float });
        }
        if from == T_F32 && to == T_FLOAT {
            return Some(Self::retype(h, T_FLOAT));
        }
        if from == T_FLOAT && to == T_F32 {
            if let ExprKind::Float(f) = h.kind {
                return Some(Expr::new(ExprKind::Float(f as f32 as f64), T_F32));
            }
        }
        None
    }

    pub fn numeric_mismatch(&mut self, h: &Expr, target: TyId, span: Span) -> bool {
        if !(self.types.is_numeric(h.ty) && self.types.is_numeric(target)) || h.ty == T_FLOAT && target == T_INT {
            return false;
        }
        let (from, to) = (self.show(h.ty), self.show(target));
        let fixed = self.wrap_suffix(span, &format!(" as {}", to));
        let help = if self.types.is_floating(h.ty) && self.types.is_integer(target) {
            format!("`as {}` drops the fraction and fails at runtime if the value does not fit", to)
        } else if self.types.is_floating(target) {
            format!("`as {}` rounds the value to the precision of {}", to, to)
        } else {
            format!("{} cannot hold every {} value; `as {}` checks the value at runtime", to, from, to)
        };
        self.emit(Diagnostic::error(span, format!("expected {} but found {}", to, from)).help(help).maybe_fix(
            format!("convert it with `as {}`", to),
            span,
            fixed,
        ));
        true
    }

    pub fn convert_num(&mut self, h: Expr, to: TyId, span: Span) -> Expr {
        let from = h.ty;
        if let Some(x) = self.widen_num(h.clone(), to) {
            return x;
        }
        if self.types.is_integer(from) && self.types.is_integer(to) {
            if let ExprKind::Int(v) = h.kind {
                let v = self.lit_value(v, from);
                if self.fits_int(v, to) {
                    return Expr::new(ExprKind::Int(v as i64), to);
                }
                let s = self.show(to);
                self.error(span, format!("cannot convert {} to {}", v, s));
                return Self::err_expr();
            }
            let code = self.num_code(to) | if from == T_U64 { burn_runtime::api::CONV_FROM_U64 as i64 } else { 0 };
            let loc = self.loc_expr(span);
            return Expr::new(ExprKind::Rt(RtFn::NumConv, vec![h, Expr::int(code), loc]), to);
        }
        if self.types.is_floating(from) && self.types.is_integer(to) {
            if to == T_INT {
                return Expr::new(ExprKind::Conv(Conv::FloatToInt, Box::new(Self::retype(h, T_FLOAT))), T_INT);
            }
            let code = self.num_code(to);
            let loc = self.loc_expr(span);
            return Expr::new(ExprKind::Rt(RtFn::F2Num, vec![Self::retype(h, T_FLOAT), Expr::int(code), loc]), to);
        }
        if from == T_FLOAT && to == T_F32 {
            return Self::f32_round(h);
        }
        let (a, b) = (self.show(from), self.show(to));
        self.error(span, format!("cannot convert {} to {}", a, b));
        Self::err_expr()
    }

    pub fn unify_ints(&mut self, l: Expr, r: Expr, what: &str, span: Span) -> Option<(Expr, Expr, TyId)> {
        let (lt, rt) = (l.ty, r.ty);
        if lt == rt {
            return Some((l, r, lt));
        }
        if let Some(v) = Self::plain_int_lit(&l) {
            let x = self.typed_lit(v as i128, rt, span);
            return (x.ty != T_ERROR).then_some((x, r, rt));
        }
        if let Some(v) = Self::plain_int_lit(&r) {
            let x = self.typed_lit(v as i128, lt, span);
            return (x.ty != T_ERROR).then_some((l, x, lt));
        }
        if self.int_widens(lt, rt) {
            return Some((Self::retype(l, rt), r, rt));
        }
        if self.int_widens(rt, lt) {
            return Some((l, Self::retype(r, lt), lt));
        }
        if let (Some((lb, ls)), Some((rb, _))) = (self.types.int_range(lt), self.types.int_range(rt)) {
            let need = if ls { lb.max(rb + 1) } else { rb.max(lb + 1) };
            let t = match need {
                0..=16 => Some(T_I16),
                17..=32 => Some(T_I32),
                33..=64 => Some(T_INT),
                _ => None,
            };
            if let Some(t) = t {
                return Some((Self::retype(l, t), Self::retype(r, t), t));
            }
        }
        let (a, b) = (self.show(lt), self.show(rt));
        self.emit(Diagnostic::error(span, format!("cannot {} {} and {}", what, a, b)).help(format!(
            "neither type can hold every value of the other; convert one side with `as {}` or `as {}`",
            a, b
        )));
        None
    }

    pub fn float_kind(&self, lt: TyId, rt: TyId) -> TyId {
        let small = |t: TyId| t == T_F32 || self.types.is_integer(t);
        if (lt == T_F32 || rt == T_F32) && small(lt) && small(rt) {
            T_F32
        } else {
            T_FLOAT
        }
    }

    pub fn sized_arith(&mut self, op: AOp, l: Expr, r: Expr, t: TyId, span: Span) -> Expr {
        let u64t = t == T_U64;
        let signed = self.types.int_range(t).map(|x| x.1).unwrap_or(true);
        if let (ExprKind::Int(a), ExprKind::Int(b)) = (&l.kind, &r.kind) {
            let (a, b) = (self.lit_value(*a, t), self.lit_value(*b, t));
            let v = match op {
                AOp::Add => Some(a + b),
                AOp::Sub => Some(a - b),
                AOp::Mul => a.checked_mul(b),
                AOp::Div if b != 0 => Some(if u64t { a / b } else { (a as i64).wrapping_div(b as i64) as i128 }),
                AOp::Mod if b != 0 => Some(if u64t { a % b } else { (a as i64).wrapping_rem(b as i64) as i128 }),
                _ => None,
            };
            if let Some(v) = v {
                if self.fits_int(v, t) {
                    return Expr::new(ExprKind::Int(v as i64), t);
                }
                let s = self.show(t);
                let help = self.range_text(t);
                self.emit(Diagnostic::error(span, format!("`{} {} {}` overflows {}", a, op.symbol(), b, s)).help(help));
                return Self::err_expr();
            }
        }
        let loc = self.loc(span);
        if u64t {
            let f = match op {
                AOp::Add => RtFn::UAdd,
                AOp::Sub => RtFn::USub,
                AOp::Mul => RtFn::UMul,
                AOp::Div => RtFn::UDiv,
                _ => RtFn::UMod,
            };
            return Expr::new(ExprKind::Rt(f, vec![l, r, Expr::new(ExprKind::LocId(loc), T_INT)]), t);
        }
        let bop = match op {
            AOp::Add => BinOp::IAdd(loc),
            AOp::Sub => BinOp::ISub(loc),
            AOp::Mul => BinOp::IMul(loc),
            AOp::Div => BinOp::IDiv(loc),
            _ => BinOp::IMod(loc),
        };
        let e = Expr::new(ExprKind::Binary(bop, Box::new(l), Box::new(r)), t);
        if op == AOp::Mod || (op == AOp::Div && !signed) {
            return e;
        }
        self.fit_check(e, t, span)
    }

    pub fn sized_bits(&mut self, op: AOp, l: Expr, r: Expr, span: Span) -> Expr {
        let shift = matches!(op, AOp::Shl | AOp::Shr | AOp::UShr);
        let (l, r, t) = if shift {
            let t = l.ty;
            (l, Self::retype(r, T_INT), t)
        } else {
            match self.unify_ints(l, r, &format!("apply `{}` to", op.symbol()), span) {
                Some(x) => x,
                None => return Self::err_expr(),
            }
        };
        let (bits, signed) = self.types.int_range(t).unwrap_or((64, true));
        if shift {
            if let ExprKind::Int(b) = r.kind {
                if !(0..64).contains(&b) {
                    self.emit(Diagnostic::error(span, format!("cannot shift by {}", b)).help("the shift amount must be from 0 to 63"));
                    return Self::err_expr();
                }
            }
        }
        if let (ExprKind::Int(a), ExprKind::Int(b)) = (&l.kind, &r.kind) {
            let (a, b) = (*a as u64, *b as u64);
            let mask = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
            let v = match op {
                AOp::BitAnd => a & b,
                AOp::BitOr => a | b,
                AOp::BitXor => a ^ b,
                AOp::Shl => a << b,
                AOp::Shr if signed => ((a as i64) >> b) as u64,
                _ => (a & mask) >> b,
            };
            return self.wrap_to(Expr::new(ExprKind::Int(v as i64), t), t);
        }
        let loc = if matches!(r.kind, ExprKind::Int(_)) { u32::MAX } else { self.loc(span) };
        let bin = |op: BinOp, a: Expr, b: Expr| Expr::new(ExprKind::Binary(op, Box::new(a), Box::new(b)), t);
        match op {
            AOp::BitAnd => bin(BinOp::BitAnd, l, r),
            AOp::BitOr => bin(BinOp::BitOr, l, r),
            AOp::BitXor => bin(BinOp::BitXor, l, r),
            AOp::Shl => {
                let e = bin(BinOp::Shl(loc), l, r);
                if bits == 64 {
                    e
                } else {
                    self.wrap_to(e, t)
                }
            }
            AOp::Shr if signed => bin(BinOp::Shr(loc), l, r),
            _ if !signed || bits == 64 => bin(BinOp::UShr(loc), l, r),
            _ => {
                let mask = Expr::new(ExprKind::Int(((1u64 << bits) - 1) as i64), t);
                let masked = bin(BinOp::BitAnd, l, mask);
                let e = bin(BinOp::UShr(loc), masked, r);
                self.wrap_to(e, t)
            }
        }
    }

    pub fn num_compare(&mut self, cmp: Cmp, a: Expr, b: Expr, span: Span) -> Expr {
        let bool_of = |op: BinOp, a: Expr, b: Expr| Expr::new(ExprKind::Binary(op, Box::new(a), Box::new(b)), T_BOOL);
        if self.types.is_integer(a.ty) && self.types.is_integer(b.ty) {
            let what = if matches!(cmp, Cmp::Eq | Cmp::Ne) { "compare" } else { "order" };
            return match self.unify_ints(a, b, what, span) {
                Some((a, b, t)) if t == T_U64 && !matches!(cmp, Cmp::Eq | Cmp::Ne) => bool_of(BinOp::UCmp(cmp), a, b),
                Some((a, b, _)) => bool_of(BinOp::ICmp(cmp), a, b),
                None => Expr::new(ExprKind::Bool(false), T_BOOL),
            };
        }
        let a = self.coerce(a, T_FLOAT, span);
        let b = self.coerce(b, T_FLOAT, span);
        bool_of(BinOp::FCmp(cmp), a, b)
    }

    pub fn num_neg(&mut self, h: Expr, span: Span) -> Expr {
        let t = h.ty;
        match self.types.get(t).clone() {
            Ty::Num(Num::F32) => Expr::new(ExprKind::Unary(UnOp::FNeg, Box::new(h)), T_F32),
            Ty::Num(n) if n.signed() => {
                if let ExprKind::Int(v) = h.kind {
                    return self.typed_lit(-(v as i128), t, span);
                }
                let l = self.loc(span);
                let e = Expr::new(ExprKind::Unary(UnOp::INeg(l), Box::new(h)), t);
                self.fit_check(e, t, span)
            }
            _ => {
                let s = self.show(t);
                self.emit(
                    Diagnostic::error(span, format!("cannot negate a value of type {}", s))
                        .help("unsigned types only hold zero and positive numbers; convert with `as int` first"),
                );
                Self::err_expr()
            }
        }
    }

    pub fn num_bitnot(&mut self, h: Expr) -> Expr {
        let t = h.ty;
        match self.types.get(t).clone() {
            Ty::Num(n) if n.packed() && !n.signed() => {
                let mask = Expr::new(ExprKind::Int(((1u64 << n.bits()) - 1) as i64), t);
                if let ExprKind::Int(v) = h.kind {
                    return Expr::new(ExprKind::Int(!v & ((1i64 << n.bits()) - 1)), t);
                }
                Expr::new(ExprKind::Binary(BinOp::BitXor, Box::new(h), Box::new(mask)), t)
            }
            _ => {
                if let ExprKind::Int(v) = h.kind {
                    return Expr::new(ExprKind::Int(!v), t);
                }
                Expr::new(ExprKind::Unary(UnOp::BitNot, Box::new(h)), t)
            }
        }
    }

    pub fn wrapping(&mut self, name: &str, recv: Expr, arg: Expr, span: Span) -> Expr {
        let t = recv.ty;
        let arg = self.coerce(arg, t, span);
        let op = match name {
            "wrappingAdd" => BinOp::WAdd,
            "wrappingSub" => BinOp::WSub,
            _ => BinOp::WMul,
        };
        let e = Expr::new(ExprKind::Binary(op, Box::new(recv), Box::new(arg)), t);
        match self.types.num_of(t) {
            Some(n) if n.packed() => self.wrap_to(e, t),
            _ => e,
        }
    }
}

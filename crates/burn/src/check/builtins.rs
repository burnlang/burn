use super::*;
use crate::hir::{BinOp, Cmp, Conv};

pub const BUILTINS: &[&str] = &[
    "print",
    "println",
    "eprint",
    "write",
    "input",
    "readStdin",
    "toString",
    "str",
    "toInt",
    "toFloat",
    "parseInt",
    "parseFloat",
    "isInt",
    "isNumber",
    "len",
    "length",
    "size",
    "push",
    "append",
    "pop",
    "insert",
    "remove",
    "contains",
    "indexOf",
    "join",
    "reverse",
    "sort",
    "slice",
    "copy",
    "clear",
    "keys",
    "values",
    "has",
    "get",
    "substring",
    "charAt",
    "split",
    "trim",
    "upper",
    "lower",
    "toUpper",
    "toLower",
    "toUpperCase",
    "toLowerCase",
    "startsWith",
    "endsWith",
    "replace",
    "repeat",
    "chars",
    "charCode",
    "isLetter",
    "isDigit",
    "isAlphanumeric",
    "isWhitespace",
    "fromCharCode",
    "sqrt",
    "pow",
    "abs",
    "floor",
    "ceil",
    "round",
    "min",
    "max",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "log",
    "log10",
    "exp",
    "random",
    "randomInt",
    "seed",
    "now",
    "nowMs",
    "millis",
    "clock",
    "sleep",
    "typeOf",
    "annotationsOf",
    "parseJSON",
    "toJSON",
    "readFile",
    "writeFile",
    "appendFile",
    "fileExists",
    "env",
    "args",
    "exit",
    "panic",
    "assert",
    "gc",
    "__localTime",
    "__httpRequest",
    "__exec",
    "__fsOp",
    "__listDir",
    "__cwd",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name)
}

pub fn signature(name: &str) -> &'static str {
    match name {
        "print" | "println" => "print(values...): prints the values separated by spaces",
        "isLetter" | "isDigit" | "isAlphanumeric" | "isWhitespace" => "isLetter(text: string): bool, true when every character is in the class",
        "eprint" => "eprint(values...): prints the values to standard error",
        "readStdin" => "readStdin(): string, all remaining standard input",
        "write" => "write(value): prints without a newline",
        "input" => "input(prompt: string = \"\"): string",
        "toString" | "str" => "toString(value): string",
        "toInt" | "parseInt" => "toInt(value: string | float | int): int",
        "toFloat" | "parseFloat" => "toFloat(value: string | int | float): float",
        "len" | "length" | "size" => "len(value: string | [T] | {K: V}): int",
        "push" => "push(array: [T], value: T)",
        "append" => "append(array: [T], value: T): [T]",
        "pop" => "pop(array: [T]): T",
        "insert" => "insert(array: [T], index: int, value: T)",
        "remove" => "remove(array: [T], index: int): T  |  remove(map: {K: V}, key: K): bool",
        "contains" => "contains(collection, value): bool",
        "indexOf" => "indexOf(collection, value): int",
        "join" => "join(array: [T], separator: string = \"\"): string",
        "split" => "split(text: string, separator: string): [string]",
        "substring" => "substring(text: string, start: int, end: int = length): string",
        "sqrt" => "sqrt(x: float): float",
        "pow" => "pow(base, exponent): int | float",
        "random" => "random(): float in [0, 1)",
        "randomInt" => "randomInt(min: int, max: int): int (inclusive)",
        "now" => "now(): float seconds since the Unix epoch",
        "nowMs" | "millis" => "nowMs(): int milliseconds since the Unix epoch",
        "sleep" => "sleep(ms: int)",
        "typeOf" => "typeOf(value): string",
        "annotationsOf" => "annotationsOf(value): [any], the annotations declared on the value's type",
        "parseJSON" => "parseJSON(text: string): any",
        "toJSON" => "toJSON(value): string",
        "readFile" => "readFile(path: string): string",
        "writeFile" => "writeFile(path: string, content: string): bool",
        "exit" => "exit(code: int = 0)",
        "panic" => "panic(message: string)",
        "assert" => "assert(condition: bool, message: string = \"\")",
        "args" => "args(): [string]",
        _ => "built-in function",
    }
}

pub enum Arg<'b> {
    H(Expr, Span),
    A(&'b ast::Expr),
}

impl<'a> Checker<'a> {
    fn barg(&mut self, a: &Arg, expected: Option<TyId>) -> Expr {
        match a {
            Arg::H(h, _) => h.clone(),
            Arg::A(e) => self.expr(e, expected),
        }
    }

    fn barg_to(&mut self, a: &Arg, t: TyId) -> Expr {
        match a {
            Arg::H(h, s) => self.coerce(h.clone(), t, *s),
            Arg::A(e) => self.expr_to(e, t),
        }
    }

    fn aspan(a: &Arg) -> Span {
        match a {
            Arg::H(_, s) => *s,
            Arg::A(e) => e.span,
        }
    }

    fn arity(&mut self, name: &str, n: usize, min: usize, max: usize, span: Span) -> bool {
        if n < min || n > max {
            let want = if min == max {
                format!("{}", min)
            } else if max == usize::MAX {
                format!("at least {}", min)
            } else {
                format!("{} to {}", min, max)
            };
            self.error_note(
                span,
                format!("`{}` expects {} argument{} but got {}", name, want, if max == 1 { "" } else { "s" }, n),
                signature(name),
            );
            return false;
        }
        true
    }

    fn rt(f: RtFn, args: Vec<Expr>, t: TyId) -> Expr {
        Expr::new(ExprKind::Rt(f, args), t)
    }

    fn float_arg(&mut self, a: &Arg) -> Expr {
        let h = self.barg(a, Some(T_FLOAT));
        if h.ty == T_INT || h.ty == T_FLOAT || h.ty == T_ERROR {
            let s = Self::aspan(a);
            return self.coerce(h, T_FLOAT, s);
        }
        let s = self.show(h.ty);
        self.error(Self::aspan(a), format!("expected a number but found {}", s));
        Self::err_expr()
    }

    fn type_err(&mut self, a: &Arg, what: &str, h: &Expr) -> Expr {
        if h.ty != T_ERROR {
            let s = self.show(h.ty);
            self.error(Self::aspan(a), format!("expected {} but found {}", what, s));
        }
        Self::err_expr()
    }

    pub fn builtin(&mut self, name: &str, recv: Option<(Expr, Span)>, args: &[ast::Expr], span: Span, expected: Option<TyId>) -> Option<Expr> {
        if !is_builtin(name) {
            return None;
        }
        if let Some((r, _)) = &recv {
            let generic = matches!(name, "toString" | "str" | "typeOf" | "toJSON");
            if !generic
                && matches!(
                    self.types.get(r.ty),
                    Ty::Record(_) | Ty::Interface(_) | Ty::Enum(_) | Ty::Func(..) | Ty::Future(_)
                )
            {
                return None;
            }
            if name.starts_with("__") {
                return None;
            }
        }
        let is_method = recv.is_some();
        let mut xs: Vec<Arg> = Vec::new();
        if let Some((r, s)) = recv {
            xs.push(Arg::H(r, s));
        }
        xs.extend(args.iter().map(Arg::A));
        let n = xs.len();
        let _ = is_method;
        let e = match name {
            "print" | "println" | "eprint" => {
                let mut acc: Option<Expr> = None;
                for a in &xs {
                    let h = self.barg(a, None);
                    if h.ty == T_VOID {
                        self.error(Self::aspan(a), "cannot print a value of type void");
                    }
                    let s = self.stringify(h);
                    acc = Some(match acc {
                        None => s,
                        Some(prev) => {
                            let sp = self.str_lit(" ");
                            let c = self.concat(prev, sp);
                            self.concat(c, s)
                        }
                    });
                }
                let s = acc.unwrap_or_else(|| self.str_lit(""));
                Self::rt(if name == "eprint" { RtFn::PrintErr } else { RtFn::Print }, vec![s], T_VOID)
            }
            "write" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], None);
                let s = self.stringify(h);
                Self::rt(RtFn::PrintRaw, vec![s], T_VOID)
            }
            "input" => {
                if !self.arity(name, n, 0, 1, span) {
                    return Some(Self::err_expr());
                }
                let p = if n == 1 {
                    let h = self.barg(&xs[0], None);
                    self.stringify(h)
                } else {
                    self.str_lit("")
                };
                Self::rt(RtFn::Input, vec![p], T_STR)
            }
            "toString" | "str" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], None);
                if h.ty == T_VOID {
                    self.error(Self::aspan(&xs[0]), "cannot convert void to a string");
                }
                self.stringify(h)
            }
            "toInt" | "parseInt" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], None);
                match self.types.get(h.ty).clone() {
                    Ty::Str => {
                        let l = self.loc_expr(span);
                        Self::rt(RtFn::ParseInt, vec![h, l], T_INT)
                    }
                    Ty::Float => Expr::new(ExprKind::Conv(Conv::FloatToInt, Box::new(h)), T_INT),
                    Ty::Int => h,
                    Ty::Bool | Ty::Enum(_) => Self::retype(h, T_INT),
                    Ty::Any => {
                        let l = self.loc_expr(span);
                        let t = h.ty;
                        Self::rt(RtFn::Cast, vec![h, Self::tid(t), Self::tid(T_INT), l], T_INT)
                    }
                    _ => self.type_err(&xs[0], "a string or number", &h),
                }
            }
            "toFloat" | "parseFloat" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], None);
                match self.types.get(h.ty).clone() {
                    Ty::Str => {
                        let l = self.loc_expr(span);
                        Self::rt(RtFn::ParseFloat, vec![h, l], T_FLOAT)
                    }
                    Ty::Int => self.coerce(h, T_FLOAT, span),
                    Ty::Float => h,
                    _ => self.type_err(&xs[0], "a string or number", &h),
                }
            }
            "isInt" | "isNumber" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg_to(&xs[0], T_STR);
                Self::rt(if name == "isInt" { RtFn::IsIntStr } else { RtFn::IsFloatStr }, vec![h], T_BOOL)
            }
            "len" | "length" | "size" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], None);
                match self.types.get(h.ty).clone() {
                    Ty::Str => Self::rt(RtFn::StrLen, vec![h], T_INT),
                    Ty::Array(_) => Expr::new(ExprKind::ArrLen(Box::new(h)), T_INT),
                    Ty::Map(..) => Self::rt(RtFn::MapLen, vec![h], T_INT),
                    Ty::Any => {
                        let l = self.loc_expr(span);
                        Self::rt(RtFn::AnyLen, vec![h, l], T_INT)
                    }
                    _ => self.type_err(&xs[0], "a string, array or map", &h),
                }
            }
            "push" | "append" => {
                if !self.arity(name, n, 2, 2, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg(&xs[0], None);
                match self.types.get(a.ty).clone() {
                    Ty::Array(et) => {
                        let at = a.ty;
                        if name == "push" {
                            let v = self.barg_to(&xs[1], et);
                            Self::rt(RtFn::ArrPush, vec![a, v], T_VOID)
                        } else {
                            let v = self.barg_to(&xs[1], et);
                            self.with_temp(a, |c, t| {
                                let _ = c;
                                Expr::new(
                                    ExprKind::Seq(vec![Stmt::Expr(Self::rt(RtFn::ArrPush, vec![t.clone(), v], T_VOID))], Box::new(t)),
                                    at,
                                )
                            })
                        }
                    }
                    _ => self.type_err(&xs[0], "an array", &a),
                }
            }
            "pop" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg(&xs[0], None);
                match self.types.get(a.ty).clone() {
                    Ty::Array(et) => {
                        let l = self.loc_expr(span);
                        Self::rt(RtFn::ArrPop, vec![a, l], et)
                    }
                    _ => self.type_err(&xs[0], "an array", &a),
                }
            }
            "insert" => {
                if !self.arity(name, n, 3, 3, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg(&xs[0], None);
                match self.types.get(a.ty).clone() {
                    Ty::Array(et) => {
                        let i = self.barg_to(&xs[1], T_INT);
                        let v = self.barg_to(&xs[2], et);
                        let l = self.loc_expr(span);
                        Self::rt(RtFn::ArrInsert, vec![a, i, v, l], T_VOID)
                    }
                    _ => self.type_err(&xs[0], "an array", &a),
                }
            }
            "remove" => {
                if !self.arity(name, n, 2, 2, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg(&xs[0], None);
                match self.types.get(a.ty).clone() {
                    Ty::Array(et) => {
                        let i = self.barg_to(&xs[1], T_INT);
                        let l = self.loc_expr(span);
                        Self::rt(RtFn::ArrRemove, vec![a, i, l], et)
                    }
                    Ty::Map(k, _) => {
                        let key = self.barg_to(&xs[1], k);
                        Self::rt(RtFn::MapRemove, vec![a, key], T_BOOL)
                    }
                    _ => self.type_err(&xs[0], "an array or map", &a),
                }
            }
            "contains" | "indexOf" => {
                if !self.arity(name, n, 2, 2, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg(&xs[0], None);
                match self.types.get(a.ty).clone() {
                    Ty::Array(et) => {
                        let v = self.barg_to(&xs[1], et);
                        if name == "contains" {
                            Self::rt(RtFn::ArrContains, vec![a, v, Self::tid(et)], T_BOOL)
                        } else {
                            Self::rt(RtFn::ArrIndexOf, vec![a, v, Self::tid(et)], T_INT)
                        }
                    }
                    Ty::Str => {
                        let v = self.barg_to(&xs[1], T_STR);
                        if name == "contains" {
                            Self::rt(RtFn::StrContains, vec![a, v], T_BOOL)
                        } else {
                            Self::rt(RtFn::StrFind, vec![a, v], T_INT)
                        }
                    }
                    Ty::Map(k, _) if name == "contains" => {
                        let key = self.barg_to(&xs[1], k);
                        Self::rt(RtFn::MapHas, vec![a, key], T_BOOL)
                    }
                    _ => self.type_err(&xs[0], "an array, string or map", &a),
                }
            }
            "join" => {
                if !self.arity(name, n, 1, 2, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg(&xs[0], None);
                match self.types.get(a.ty).clone() {
                    Ty::Array(et) => {
                        let sep = if n == 2 { self.barg_to(&xs[1], T_STR) } else { self.str_lit("") };
                        Self::rt(RtFn::ArrJoin, vec![a, sep, Self::tid(et)], T_STR)
                    }
                    _ => self.type_err(&xs[0], "an array", &a),
                }
            }
            "reverse" | "sort" | "copy" | "clear" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg(&xs[0], None);
                match self.types.get(a.ty).clone() {
                    Ty::Array(et) => match name {
                        "reverse" => Self::rt(RtFn::ArrReverse, vec![a], T_VOID),
                        "copy" => {
                            let t = a.ty;
                            Self::rt(RtFn::ArrCopy, vec![a], t)
                        }
                        "clear" => Self::rt(RtFn::ArrClear, vec![a], T_VOID),
                        _ => {
                            if !matches!(self.types.get(et), Ty::Int | Ty::Float | Ty::Str | Ty::Bool | Ty::Enum(_) | Ty::Any | Ty::Error) {
                                let s = self.show(et);
                                self.error(span, format!("cannot sort values of type {}", s));
                            }
                            Self::rt(RtFn::ArrSort, vec![a, Self::tid(et)], T_VOID)
                        }
                    },
                    Ty::Str if name == "reverse" => {
                        let chars = Self::rt(RtFn::StrChars, vec![a], T_ARR_STR);
                        self.with_temp(chars, |c, t| {
                            let empty = c.str_lit("");
                            Expr::new(
                                ExprKind::Seq(
                                    vec![Stmt::Expr(Self::rt(RtFn::ArrReverse, vec![t.clone()], T_VOID))],
                                    Box::new(Self::rt(RtFn::ArrJoin, vec![t, empty, Self::tid(T_STR)], T_STR)),
                                ),
                                T_STR,
                            )
                        })
                    }
                    Ty::Map(..) if name == "copy" || name == "clear" => {
                        self.error(span, format!("`{}` is not supported for maps yet", name));
                        Self::err_expr()
                    }
                    _ => self.type_err(&xs[0], "an array", &a),
                }
            }
            "slice" | "substring" => {
                if !self.arity(name, n, 2, 3, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg(&xs[0], None);
                let s = self.barg_to(&xs[1], T_INT);
                let e = if n == 3 { self.barg_to(&xs[2], T_INT) } else { Expr::int(i64::MAX) };
                match self.types.get(a.ty).clone() {
                    Ty::Str => Self::rt(RtFn::StrSub, vec![a, s, e], T_STR),
                    Ty::Array(_) if name == "slice" => {
                        let t = a.ty;
                        Self::rt(RtFn::ArrSlice, vec![a, s, e], t)
                    }
                    _ => self.type_err(&xs[0], if name == "slice" { "an array or string" } else { "a string" }, &a),
                }
            }
            "keys" | "values" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let m = self.barg(&xs[0], None);
                match self.types.get(m.ty).clone() {
                    Ty::Map(k, v) => {
                        let et = if name == "keys" { k } else { v };
                        let at = self.types.array(et);
                        Self::rt(if name == "keys" { RtFn::MapKeys } else { RtFn::MapValues }, vec![m, Self::tid(at)], at)
                    }
                    _ => self.type_err(&xs[0], "a map", &m),
                }
            }
            "has" => {
                if !self.arity(name, n, 2, 2, span) {
                    return Some(Self::err_expr());
                }
                let m = self.barg(&xs[0], None);
                match self.types.get(m.ty).clone() {
                    Ty::Map(k, _) => {
                        let key = self.barg_to(&xs[1], k);
                        Self::rt(RtFn::MapHas, vec![m, key], T_BOOL)
                    }
                    _ => self.type_err(&xs[0], "a map", &m),
                }
            }
            "get" => {
                if !self.arity(name, n, 2, 3, span) {
                    return Some(Self::err_expr());
                }
                let m = self.barg(&xs[0], None);
                match self.types.get(m.ty).clone() {
                    Ty::Map(k, v) => {
                        let key = self.barg_to(&xs[1], k);
                        if n == 3 {
                            let d = self.barg_to(&xs[2], v);
                            Self::rt(RtFn::MapGetOr, vec![m, key, d], v)
                        } else {
                            let l = self.loc_expr(span);
                            Self::rt(RtFn::MapGet, vec![m, key, l], v)
                        }
                    }
                    Ty::Array(et) => {
                        let i = self.barg_to(&xs[1], T_INT);
                        let l = self.loc(span);
                        Expr::new(ExprKind::Index(Box::new(m), Box::new(i), l), et)
                    }
                    _ => self.type_err(&xs[0], "a map", &m),
                }
            }
            "charAt" => {
                if !self.arity(name, n, 2, 2, span) {
                    return Some(Self::err_expr());
                }
                let s = self.barg_to(&xs[0], T_STR);
                let i = self.barg_to(&xs[1], T_INT);
                self.with_temp(i, |_, t| {
                    let end = Expr::new(ExprKind::Binary(BinOp::IAdd(u32::MAX), Box::new(t.clone()), Box::new(Expr::int(1))), T_INT);
                    Self::rt(RtFn::StrSub, vec![s, t, end], T_STR)
                })
            }
            "split" => {
                if !self.arity(name, n, 1, 2, span) {
                    return Some(Self::err_expr());
                }
                let s = self.barg_to(&xs[0], T_STR);
                let sep = if n == 2 { self.barg_to(&xs[1], T_STR) } else { self.str_lit(" ") };
                Self::rt(RtFn::StrSplit, vec![s, sep], T_ARR_STR)
            }
            "trim" | "upper" | "lower" | "toUpper" | "toLower" | "toUpperCase" | "toLowerCase" | "chars" | "charCode" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let s = self.barg_to(&xs[0], T_STR);
                match name {
                    "trim" => Self::rt(RtFn::StrTrim, vec![s], T_STR),
                    "upper" | "toUpper" | "toUpperCase" => Self::rt(RtFn::StrUpper, vec![s], T_STR),
                    "lower" | "toLower" | "toLowerCase" => Self::rt(RtFn::StrLower, vec![s], T_STR),
                    "chars" => Self::rt(RtFn::StrChars, vec![s], T_ARR_STR),
                    _ => Self::rt(RtFn::StrCode, vec![s], T_INT),
                }
            }
            "isLetter" | "isDigit" | "isAlphanumeric" | "isWhitespace" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let s = self.barg_to(&xs[0], T_STR);
                let k = match name {
                    "isLetter" => 0,
                    "isDigit" => 1,
                    "isAlphanumeric" => 2,
                    _ => 3,
                };
                Self::rt(RtFn::CharClass, vec![s, Expr::int(k)], T_BOOL)
            }
            "fromCharCode" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let c = self.barg_to(&xs[0], T_INT);
                Self::rt(RtFn::StrFromCode, vec![c], T_STR)
            }
            "startsWith" | "endsWith" | "repeat" => {
                if !self.arity(name, n, 2, 2, span) {
                    return Some(Self::err_expr());
                }
                let s = self.barg_to(&xs[0], T_STR);
                match name {
                    "startsWith" => {
                        let p = self.barg_to(&xs[1], T_STR);
                        Self::rt(RtFn::StrStarts, vec![s, p], T_BOOL)
                    }
                    "endsWith" => {
                        let p = self.barg_to(&xs[1], T_STR);
                        Self::rt(RtFn::StrEnds, vec![s, p], T_BOOL)
                    }
                    _ => {
                        let c = self.barg_to(&xs[1], T_INT);
                        Self::rt(RtFn::StrRepeat, vec![s, c], T_STR)
                    }
                }
            }
            "replace" => {
                if !self.arity(name, n, 3, 3, span) {
                    return Some(Self::err_expr());
                }
                let s = self.barg_to(&xs[0], T_STR);
                let a = self.barg_to(&xs[1], T_STR);
                let b = self.barg_to(&xs[2], T_STR);
                Self::rt(RtFn::StrReplace, vec![s, a, b], T_STR)
            }
            "sqrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "log" | "log10" | "exp" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let x = self.float_arg(&xs[0]);
                let op = match name {
                    "sqrt" => 0,
                    "sin" => 5,
                    "cos" => 6,
                    "tan" => 7,
                    "log" => 8,
                    "log10" => 9,
                    "exp" => 10,
                    "asin" => 11,
                    "acos" => 12,
                    _ => 13,
                };
                Self::rt(RtFn::FMath, vec![Expr::int(op), x], T_FLOAT)
            }
            "floor" | "ceil" | "round" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], None);
                if h.ty == T_INT {
                    h
                } else {
                    let s = Self::aspan(&xs[0]);
                    let x = if h.ty == T_FLOAT { h } else { self.coerce(h, T_FLOAT, s) };
                    let op = match name {
                        "floor" => 1,
                        "ceil" => 2,
                        _ => 3,
                    };
                    let f = Self::rt(RtFn::FMath, vec![Expr::int(op), x], T_FLOAT);
                    Expr::new(ExprKind::Conv(Conv::FloatToInt, Box::new(f)), T_INT)
                }
            }
            "abs" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], expected);
                match self.types.get(h.ty) {
                    Ty::Int => Self::rt(RtFn::IAbs, vec![h], T_INT),
                    Ty::Float => Self::rt(RtFn::FMath, vec![Expr::int(4), h], T_FLOAT),
                    _ => self.type_err(&xs[0], "a number", &h),
                }
            }
            "pow" | "min" | "max" | "atan2" => {
                if !self.arity(name, n, 2, 2, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg(&xs[0], expected);
                let b = self.barg(&xs[1], Some(a.ty));
                if !(self.types.is_numeric(a.ty) || a.ty == T_ERROR) {
                    return Some(self.type_err(&xs[0], "a number", &a));
                }
                if !(self.types.is_numeric(b.ty) || b.ty == T_ERROR) {
                    return Some(self.type_err(&xs[1], "a number", &b));
                }
                if a.ty == T_INT && b.ty == T_INT && name != "atan2" {
                    let f = match name {
                        "pow" => RtFn::IPow,
                        "min" => RtFn::IMin,
                        _ => RtFn::IMax,
                    };
                    Self::rt(f, vec![a, b], T_INT)
                } else {
                    let a = self.coerce(a, T_FLOAT, span);
                    let b = self.coerce(b, T_FLOAT, span);
                    let f = match name {
                        "pow" => RtFn::FPow,
                        "min" => RtFn::FMin,
                        "max" => RtFn::FMax,
                        _ => RtFn::FAtan2,
                    };
                    Self::rt(f, vec![a, b], T_FLOAT)
                }
            }
            "random" => {
                if !self.arity(name, n, 0, 0, span) {
                    return Some(Self::err_expr());
                }
                Self::rt(RtFn::Random, vec![], T_FLOAT)
            }
            "randomInt" => {
                if !self.arity(name, n, 2, 2, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg_to(&xs[0], T_INT);
                let b = self.barg_to(&xs[1], T_INT);
                Self::rt(RtFn::RandomInt, vec![a, b], T_INT)
            }
            "seed" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg_to(&xs[0], T_INT);
                Self::rt(RtFn::Seed, vec![a], T_VOID)
            }
            "now" | "nowMs" | "millis" | "clock" | "args" | "gc" | "readStdin" | "__localTime" => {
                if !self.arity(name, n, 0, 0, span) {
                    return Some(Self::err_expr());
                }
                match name {
                    "now" => Self::rt(RtFn::NowSec, vec![], T_FLOAT),
                    "nowMs" | "millis" => Self::rt(RtFn::NowMs, vec![], T_INT),
                    "clock" => Self::rt(RtFn::ClockNs, vec![], T_INT),
                    "args" => Self::rt(RtFn::Args, vec![], T_ARR_STR),
                    "gc" => Self::rt(RtFn::GcCollect, vec![], T_VOID),
                    "readStdin" => Self::rt(RtFn::ReadStdin, vec![], T_STR),
                    _ => Self::rt(RtFn::LocalTime, vec![], T_ARR_INT),
                }
            }
            "sleep" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], None);
                let ms = match h.ty {
                    t if t == T_FLOAT => Expr::new(ExprKind::Conv(Conv::FloatToInt, Box::new(h)), T_INT),
                    _ => {
                        let s = Self::aspan(&xs[0]);
                        self.coerce(h, T_INT, s)
                    }
                };
                Self::rt(RtFn::Sleep, vec![ms], T_VOID)
            }
            "typeOf" | "toJSON" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], None);
                let t = h.ty;
                Self::rt(
                    if name == "typeOf" { RtFn::TypeName } else { RtFn::JsonStringify },
                    vec![h, Self::tid(t)],
                    T_STR,
                )
            }
            "parseJSON" | "readFile" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let s = self.barg_to(&xs[0], T_STR);
                let l = self.loc_expr(span);
                if name == "parseJSON" {
                    Self::rt(RtFn::JsonParse, vec![s, l], T_ANY)
                } else {
                    Self::rt(RtFn::ReadFile, vec![s, l], T_STR)
                }
            }
            "writeFile" | "appendFile" => {
                if !self.arity(name, n, 2, 2, span) {
                    return Some(Self::err_expr());
                }
                let p = self.barg_to(&xs[0], T_STR);
                let h = self.barg(&xs[1], None);
                let c = self.stringify(h);
                Self::rt(if name == "writeFile" { RtFn::WriteFile } else { RtFn::AppendFile }, vec![p, c], T_BOOL)
            }
            "fileExists" | "env" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let p = self.barg_to(&xs[0], T_STR);
                if name == "env" {
                    Self::rt(RtFn::Env, vec![p], T_STR)
                } else {
                    Self::rt(RtFn::FileExists, vec![p], T_BOOL)
                }
            }
            "exit" => {
                if !self.arity(name, n, 0, 1, span) {
                    return Some(Self::err_expr());
                }
                let c = if n == 1 { self.barg_to(&xs[0], T_INT) } else { Expr::int(0) };
                Self::rt(RtFn::ExitNow, vec![c], T_VOID)
            }
            "annotationsOf" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let h = self.barg(&xs[0], None);
                self.annotations_of(h)
            }
            "panic" => {
                if !self.arity(name, n, 0, 1, span) {
                    return Some(Self::err_expr());
                }
                let m = if n == 1 {
                    let h = self.barg(&xs[0], None);
                    self.stringify(h)
                } else {
                    self.str_lit("explicit panic")
                };
                let l = self.loc_expr(span);
                Self::rt(RtFn::Panic, vec![m, l], T_VOID)
            }
            "assert" => {
                if !self.arity(name, n, 1, 2, span) {
                    return Some(Self::err_expr());
                }
                let c = self.barg_to(&xs[0], T_BOOL);
                let m = if n == 2 {
                    let h = self.barg(&xs[1], None);
                    self.stringify(h)
                } else {
                    let text = match &xs[0] {
                        Arg::A(e) => {
                            let f = self.sm.file(e.span.file);
                            f.src.get(e.span.start as usize..e.span.end as usize).unwrap_or("condition").to_string()
                        }
                        _ => "condition".to_string(),
                    };
                    self.str_lit(&text)
                };
                let l = self.loc_expr(span);
                Self::rt(RtFn::Assert, vec![c, m, l], T_VOID)
            }
            "__exec" | "__fsOp" => {
                if !self.arity(name, n, 3, 3, span) {
                    return Some(Self::err_expr());
                }
                let a = self.barg_to(&xs[0], T_STR);
                if name == "__exec" {
                    let v = self.barg_to(&xs[1], T_ARR_STR);
                    let c = self.barg_to(&xs[2], T_BOOL);
                    Self::rt(RtFn::Exec, vec![a, v, c], T_ARR_STR)
                } else {
                    let x = self.barg_to(&xs[1], T_STR);
                    let y = self.barg_to(&xs[2], T_STR);
                    Self::rt(RtFn::FsOp, vec![a, x, y], T_BOOL)
                }
            }
            "__listDir" => {
                if !self.arity(name, n, 1, 1, span) {
                    return Some(Self::err_expr());
                }
                let p = self.barg_to(&xs[0], T_STR);
                Self::rt(RtFn::ListDir, vec![p], T_ARR_STR)
            }
            "__cwd" => {
                if !self.arity(name, n, 0, 0, span) {
                    return Some(Self::err_expr());
                }
                Self::rt(RtFn::Cwd, vec![], T_STR)
            }
            "__httpRequest" => {
                if !self.arity(name, n, 4, 4, span) {
                    return Some(Self::err_expr());
                }
                let m = self.barg_to(&xs[0], T_STR);
                let u = self.barg_to(&xs[1], T_STR);
                let b = self.barg_to(&xs[2], T_STR);
                let h = self.barg_to(&xs[3], T_ARR_STR);
                Self::rt(RtFn::HttpRequest, vec![m, u, b, h], T_ARR_STR)
            }
            _ => return None,
        };
        let _ = Cmp::Eq;
        Some(e)
    }
}

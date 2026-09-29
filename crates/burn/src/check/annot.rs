use super::*;
use crate::hir::{Annotation, Const};

pub const BUILTIN: [&str; 8] = ["Getter", "Setter", "Deprecated", "Export", "Native", "Inject", "Overwrite", "Redirect"];

#[derive(Clone, Copy, PartialEq)]
pub enum Site {
    Func { method: bool, bodyless: bool },
    Type { class: bool },
    Field,
}

impl Site {
    fn describe(self) -> &'static str {
        match self {
            Site::Func { method: true, .. } => "a method",
            Site::Func { .. } => "a function",
            Site::Type { class: true } => "a class",
            Site::Type { .. } => "a type",
            Site::Field => "a field",
        }
    }
}

impl<'a> Checker<'a> {
    pub fn const_value(&mut self, e: &ast::Expr) -> Option<Const> {
        Some(match &e.kind {
            ast::ExprKind::Int(v) => Const::Int(*v),
            ast::ExprKind::Float(v) => Const::Float(*v),
            ast::ExprKind::Str(s) => Const::Str(s.clone()),
            ast::ExprKind::Bool(b) => Const::Bool(*b),
            ast::ExprKind::Null => Const::Null,
            ast::ExprKind::Unary(ast::UnOp::Neg, x) => match self.const_value(x)? {
                Const::Int(v) => Const::Int(v.wrapping_neg()),
                Const::Float(v) => Const::Float(-v),
                _ => return None,
            },
            _ => return None,
        })
    }

    fn const_arg(&mut self, e: &ast::Expr) -> Option<Const> {
        let v = self.const_value(e);
        if v.is_none() {
            self.error(e.span, "annotation arguments must be literal values: numbers, strings, booleans or null");
        }
        v
    }

    fn builtin_args(&mut self, a: &ast::Annotation, keys: &[(&str, &str)], required: Option<&str>) -> Vec<(String, Const)> {
        let mut out: Vec<(String, Const)> = Vec::new();
        for (i, (key, e)) in a.args.iter().enumerate() {
            let k = match key {
                Some(k) => k.name.clone(),
                None if i == 0 && !keys.is_empty() => keys[0].0.to_string(),
                None => {
                    self.error(
                        e.span,
                        format!(
                            "name this argument of @{}, for example `{}: ...`",
                            a.name.name,
                            keys.first().map(|k| k.0).unwrap_or("name")
                        ),
                    );
                    continue;
                }
            };
            let Some(&(_, kind)) = keys.iter().find(|(n, _)| *n == k) else {
                let known: Vec<&str> = keys.iter().map(|k| k.0).collect();
                let span = key.as_ref().map(|k| k.span).unwrap_or(e.span);
                if known.is_empty() {
                    self.error(span, format!("@{} takes no arguments", a.name.name));
                } else {
                    self.error(span, format!("@{} has no argument `{}` (it takes {})", a.name.name, k, known.join(", ")));
                }
                continue;
            };
            let Some(v) = self.const_arg(e) else { continue };
            let ok = matches!((kind, &v), ("string", Const::Str(_)) | ("bool", Const::Bool(_)) | ("int", Const::Int(_)));
            if !ok {
                self.error(e.span, format!("`{}` of @{} must be a {}", k, a.name.name, kind));
                continue;
            }
            if out.iter().any(|(x, _)| *x == k) {
                self.error(e.span, format!("`{}` is given twice", k));
                continue;
            }
            out.push((k, v));
        }
        if let Some(r) = required {
            if !out.iter().any(|(k, _)| k == r) {
                self.error(a.span, format!("@{} needs `{}`", a.name.name, r));
            }
        }
        out
    }

    pub fn resolve_annotations(&mut self, mi: usize, anns: &[ast::Annotation], site: Site) -> Vec<Annotation> {
        let mut out = Vec::new();
        for a in anns {
            let name = a.name.name.as_str();
            let bad_site = |allowed: bool| !allowed;
            let args = match name {
                "Getter" | "Setter" => {
                    if bad_site(matches!(site, Site::Type { class: true } | Site::Field)) {
                        self.error(
                            a.span,
                            format!("@{} can only be used on classes and their fields, not on {}", name, site.describe()),
                        );
                    }
                    self.builtin_args(a, &[], None)
                }
                "Deprecated" => {
                    if bad_site(matches!(site, Site::Func { .. } | Site::Type { .. })) {
                        self.error(
                            a.span,
                            format!("@Deprecated can be used on functions, methods and types, not on {}", site.describe()),
                        );
                    }
                    self.builtin_args(a, &[("message", "string")], None)
                }
                "Export" | "Native" => {
                    let bodyless = name == "Native";
                    let ok = site == Site::Func { method: false, bodyless };
                    if !ok {
                        let what = if name == "Export" {
                            "top-level functions with a body"
                        } else {
                            "top-level functions without a body, such as `@Native fun now(): int`"
                        };
                        self.error(a.span, format!("@{} can only be used on {}", name, what));
                    }
                    self.builtin_args(a, &[("name", "string")], None)
                }
                "Inject" | "Overwrite" | "Redirect" => {
                    if bad_site(matches!(
                        site,
                        Site::Func {
                            method: false,
                            bodyless: false
                        }
                    )) {
                        self.error(a.span, format!("@{} can only be used on top-level functions", name));
                    }
                    let keys: &[(&str, &str)] = match name {
                        "Inject" => &[("target", "string"), ("at", "string"), ("cancellable", "bool"), ("priority", "int")],
                        "Overwrite" => &[("target", "string"), ("priority", "int")],
                        _ => &[
                            ("target", "string"),
                            ("call", "string"),
                            ("rt", "string"),
                            ("host", "string"),
                            ("priority", "int"),
                        ],
                    };
                    let args = self.builtin_args(a, keys, Some("target"));
                    if name == "Inject" {
                        if let Some((_, Const::Str(at))) = args.iter().find(|(k, _)| k == "at") {
                            if !matches!(at.as_str(), "head" | "return") {
                                self.error(a.span, "`at` must be \"head\" or \"return\"");
                            }
                        }
                    }
                    if name == "Redirect" && !args.iter().any(|(k, _)| k == "call" || k == "rt" || k == "host") {
                        self.error(a.span, "@Redirect needs `call`, `rt` or `host` to say which call to replace");
                    }
                    args
                }
                _ => {
                    match self.user_annotation(mi, a) {
                        Some(x) => out.push(x),
                        None => continue,
                    }
                    continue;
                }
            };
            if out.iter().any(|x: &Annotation| x.name == name) {
                self.error(a.span, format!("@{} is used twice", name));
                continue;
            }
            out.push(Annotation {
                name: name.to_string(),
                ty: None,
                args,
                span: a.span,
            });
        }
        out
    }

    fn user_annotation(&mut self, mi: usize, a: &ast::Annotation) -> Option<Annotation> {
        let name = &a.name.name;
        let t = match self.mods[mi].types.get(name).map(|e| e.sym).or_else(|| {
            let saved = self.diags.len();
            let r = self.lookup_type_name(mi, name, a.name.span);
            self.diags.truncate(saved);
            r
        }) {
            Some(t) if self.annotation_types.contains(&t) => t,
            Some(t) if t != T_ERROR => {
                let s = self.show(t);
                self.error_note(
                    a.name.span,
                    format!("`{}` is not an annotation", s),
                    format!("declare it with `def annotation {} {{ ... }}`", name),
                );
                return None;
            }
            Some(_) => return None,
            None => {
                let mut cands: Vec<String> = BUILTIN.iter().map(|s| s.to_string()).collect();
                for mm in 0..self.mods.len() {
                    for (n, e) in &self.mods[mm].types {
                        if self.annotation_types.contains(&e.sym) {
                            cands.push(n.clone());
                        }
                    }
                }
                match suggest(name, cands.iter().map(|s| s.as_str())) {
                    Some(s) => self.error_note(a.name.span, format!("unknown annotation `@{}`", name), format!("did you mean `@{}`?", s)),
                    None => self.error_note(
                        a.name.span,
                        format!("unknown annotation `@{}`", name),
                        format!("declare it with `def annotation {} {{ ... }}`", name),
                    ),
                }
                return None;
            }
        };
        let rec = self.types.record_of(t)?.clone();
        self.def_link(a.name.span, rec.span);
        let mut given: Vec<Option<(Const, Span)>> = vec![None; rec.fields.len()];
        for (i, (key, e)) in a.args.iter().enumerate() {
            let fi = match key {
                Some(k) => match rec.field_index(&k.name) {
                    Some(fi) => fi,
                    None => {
                        self.error(k.span, format!("@{} has no field `{}`", name, k.name));
                        continue;
                    }
                },
                None if i < rec.fields.len() => i,
                None => {
                    self.error(e.span, format!("@{} takes {} arguments", name, rec.fields.len()));
                    continue;
                }
            };
            let Some(v) = self.const_arg(e) else { continue };
            if given[fi].is_some() {
                self.error(e.span, format!("`{}` is given twice", rec.fields[fi].name));
            }
            given[fi] = Some((v, e.span));
        }
        let mut args = Vec::new();
        for (fi, f) in rec.fields.iter().enumerate() {
            let v = match given[fi].take() {
                Some(v) => Some(v),
                None => match &f.default {
                    Some(d) => {
                        let v = self.const_value(d);
                        if v.is_none() {
                            self.error(d.span, "defaults of annotation fields must be literal values");
                        }
                        v.map(|v| (v, d.span))
                    }
                    None => {
                        self.error(a.span, format!("@{} needs a value for `{}`", name, f.name));
                        None
                    }
                },
            };
            let Some((v, span)) = v else { continue };
            let base = self.types.unwrap_optional(f.ty);
            let nullable = self.types.is_nullable(f.ty) || f.ty == T_ANY;
            let v = match (&v, base) {
                (Const::Int(x), T_FLOAT) => Const::Float(*x as f64),
                _ => v,
            };
            let ok = f.ty == T_ANY
                || matches!(
                    (&v, base),
                    (Const::Int(_), T_INT) | (Const::Float(_), T_FLOAT) | (Const::Bool(_), T_BOOL) | (Const::Str(_), T_STR)
                )
                || (v == Const::Null && nullable);
            if !ok {
                let want = self.show(f.ty);
                self.error(span, format!("`{}` of @{} must be {}", f.name, name, want));
                continue;
            }
            args.push((f.name.clone(), v));
        }
        Some(Annotation {
            name: name.clone(),
            ty: Some(t),
            args,
            span: a.span,
        })
    }

    pub fn deprecation_note(anns: &[Annotation]) -> Option<String> {
        anns.iter()
            .find(|a| a.name == "Deprecated")
            .map(|a| a.str_arg("message").unwrap_or("").to_string())
    }

    pub fn warn_deprecated_func(&mut self, fid: FuncId, span: Span) {
        if self.is_dry() {
            return;
        }
        if let Some(msg) = self.funcs[fid as usize].deprecated.clone() {
            let name = self.funcs[fid as usize].name.clone();
            if msg.is_empty() {
                self.warn(span, format!("`{}` is deprecated", name));
            } else {
                self.warn(span, format!("`{}` is deprecated: {}", name, msg));
            }
        }
    }

    pub fn warn_deprecated_type(&mut self, t: TyId, span: Span) {
        if self.is_dry() {
            return;
        }
        if let Some(msg) = self.deprecated_types.get(&t).cloned() {
            let name = self.show(t);
            if msg.is_empty() {
                self.warn(span, format!("`{}` is deprecated", name));
            } else {
                self.warn(span, format!("`{}` is deprecated: {}", name, msg));
            }
        }
    }

    pub fn const_expr(v: &Const, ty: TyId) -> Expr {
        match v {
            Const::Int(x) => Expr::new(ExprKind::Int(*x), T_INT),
            Const::Float(x) => Expr::new(ExprKind::Float(*x), T_FLOAT),
            Const::Bool(b) => Expr::new(ExprKind::Bool(*b), T_BOOL),
            Const::Null => Expr::new(ExprKind::Null, ty),
            Const::Str(_) => unreachable!(),
        }
    }

    fn annotation_instance(&mut self, a: &Annotation) -> Option<Expr> {
        let t = a.ty?;
        let rec = self.types.record_of(t)?.clone();
        let mut fields = Vec::with_capacity(rec.fields.len());
        for f in &rec.fields {
            let v = a.arg(&f.name).cloned().unwrap_or(Const::Null);
            let e = match &v {
                Const::Str(s) => self.str_lit(s),
                other => Self::const_expr(other, if *other == Const::Null { T_NULL } else { f.ty }),
            };
            let e = match self.try_coerce(e, f.ty) {
                Ok(e) => e,
                Err(e) => e,
            };
            fields.push(e);
        }
        let inst = Expr::new(ExprKind::NewStruct(t, fields), t);
        Some(self.coerce(inst, T_ANY, a.span))
    }

    fn annotations_array(&mut self, t: TyId) -> Expr {
        let anns = self.type_anns.get(&t).cloned().unwrap_or_default();
        let items: Vec<Expr> = anns.iter().filter_map(|a| self.annotation_instance(a)).collect();
        Expr::new(ExprKind::NewArray(T_ARR_ANY, items), T_ARR_ANY)
    }

    pub fn annotations_of(&mut self, h: Expr) -> Expr {
        let t = self.types.unwrap_optional(h.ty);
        if let Ty::Record(_) = self.types.get(t) {
            if !self.types.is_nullable(h.ty) && !h.has_side_effects() {
                return self.annotations_array(t);
            }
        }
        let annotated: Vec<TyId> = self
            .type_anns
            .iter()
            .filter(|(_, v)| v.iter().any(|a| a.ty.is_some()))
            .map(|(t, _)| *t)
            .collect();
        let mut annotated = annotated;
        annotated.sort();
        let from = h.ty;
        let slot = self.new_local(from);
        let res = self.new_local(T_ARR_ANY);
        let mut stmts = vec![
            Stmt::Expr(Expr::new(ExprKind::SetLocal(slot, Box::new(h)), from)),
            Stmt::Expr(Expr::new(
                ExprKind::SetLocal(res, Box::new(Expr::new(ExprKind::NewArray(T_ARR_ANY, vec![]), T_ARR_ANY))),
                T_ARR_ANY,
            )),
        ];
        if !matches!(self.types.get(t), Ty::Int | Ty::Float | Ty::Bool | Ty::Str | Ty::Enum(_) | Ty::Func(..)) {
            for rt in annotated {
                let could = from == T_ANY || t == rt || self.types.implements(rt, t) || matches!(self.types.get(t), Ty::Any);
                if !could {
                    continue;
                }
                let test = Expr::new(
                    ExprKind::Rt(RtFn::IsType, vec![Expr::new(ExprKind::Local(slot), from), Self::tid(from), Self::tid(rt)]),
                    T_BOOL,
                );
                let arr = self.annotations_array(rt);
                stmts.push(Stmt::If(
                    test,
                    vec![Stmt::Expr(Expr::new(ExprKind::SetLocal(res, Box::new(arr)), T_ARR_ANY))],
                    vec![],
                ));
            }
        }
        Expr::new(ExprKind::Seq(stmts, Box::new(Expr::new(ExprKind::Local(res), T_ARR_ANY))), T_ARR_ANY)
    }
}

impl<'a> Checker<'a> {
    pub fn check_mixins(&mut self) {
        let hooks: Vec<(FuncId, Annotation)> = self
            .funcs
            .iter()
            .enumerate()
            .flat_map(|(i, f)| {
                f.annotations
                    .iter()
                    .filter(|a| matches!(a.name.as_str(), "Inject" | "Overwrite" | "Redirect"))
                    .map(move |a| (i as FuncId, a.clone()))
            })
            .collect();
        for (hook, a) in hooks {
            let Some(target) = a.str_arg("target").map(|s| s.to_string()) else { continue };
            let Some(tf) = self.funcs.iter().position(|f| f.name == target).map(|i| i as FuncId) else {
                continue;
            };
            if tf == hook {
                self.error(a.span, "a mixin cannot target itself");
                continue;
            }
            let (want_params, want_ret, what): (Vec<TyId>, Option<TyId>, String) = match a.name.as_str() {
                "Redirect" => {
                    let Some(call) = a.str_arg("call").map(|s| s.to_string()) else { continue };
                    let Some(cf) = self.funcs.iter().position(|f| f.name == call).map(|i| i as FuncId) else {
                        continue;
                    };
                    let ret = self.func_ret(cf);
                    (self.funcs[cf as usize].params.iter().map(|p| p.1).collect(), Some(ret), format!("`{}`", call))
                }
                _ => {
                    let ret = self.func_ret(tf);
                    let params: Vec<TyId> = self.funcs[tf as usize].params.iter().map(|p| p.1).collect();
                    let at = a.str_arg("at").unwrap_or("head");
                    let cancellable = matches!(a.arg("cancellable"), Some(Const::Bool(true)));
                    match (a.name.as_str(), at) {
                        ("Overwrite", _) => (params, Some(ret), format!("`{}`", target)),
                        ("Inject", "return") => {
                            let hp = self.funcs[hook as usize].params.len();
                            if hp == params.len() + 1 {
                                let mut ps = params;
                                ps.push(ret);
                                (ps, Some(ret), format!("`{}` plus its result", target))
                            } else {
                                (params, None, format!("`{}`", target))
                            }
                        }
                        (_, _) if cancellable => {
                            let r = if ret == T_VOID {
                                T_BOOL
                            } else if self.types.is_unboxed(ret) {
                                ret
                            } else {
                                self.types.optional(ret)
                            };
                            (params, Some(r), format!("`{}`", target))
                        }
                        _ => (params, None, format!("`{}`", target)),
                    }
                }
            };
            let hp: Vec<TyId> = self.funcs[hook as usize].params.iter().map(|p| p.1).collect();
            let hname = self.funcs[hook as usize].name.clone();
            if hp != want_params {
                let want: Vec<String> = want_params.iter().map(|t| self.show(*t)).collect();
                let got: Vec<String> = hp.iter().map(|t| self.show(*t)).collect();
                self.error_note(
                    a.span,
                    format!("the mixin `{}` must take the parameters of {}", hname, what),
                    format!("expected ({}) but `{}` takes ({})", want.join(", "), hname, got.join(", ")),
                );
                continue;
            }
            if let Some(want) = want_ret {
                let got = self.func_ret(hook);
                let ok = got == want || (want != T_VOID && self.types.unwrap_optional(want) == got && self.types.is_nullable(want) && got != T_VOID);
                if !ok {
                    let (w, g) = (self.show(want), self.show(got));
                    self.error_note(
                        a.span,
                        format!("the mixin `{}` must return {}", hname, w),
                        format!("it returns {}", if got == T_VOID { "nothing".to_string() } else { g }),
                    );
                }
            }
        }
    }
}

use crate::ast::*;
use crate::lexer::escape;
use crate::source::Span;

struct W {
    out: String,
    depth: usize,
}

fn sp(s: Span) -> String {
    format!("@{}..{}", s.start, s.end)
}

fn id(i: &Ident) -> String {
    format!("{}{}", i.name, sp(i.span))
}

fn vis(v: Vis) -> &'static str {
    match v {
        Vis::Default => "default",
        Vis::Pub => "pub",
        Vis::Priv => "priv",
    }
}

fn unop(o: UnOp) -> &'static str {
    match o {
        UnOp::Neg => "-",
        UnOp::Not => "!",
        UnOp::BitNot => "~",
    }
}

impl W {
    fn line(&mut self, label: &str, text: &str) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        if !label.is_empty() {
            self.out.push_str(label);
            self.out.push_str(": ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn nest(&mut self, label: &str, text: &str, f: impl FnOnce(&mut W)) {
        self.line(label, text);
        self.depth += 1;
        f(self);
        self.depth -= 1;
    }

    fn annotations(&mut self, anns: &[Annotation]) {
        for a in anns {
            self.nest("ann", &format!("{} {}", id(&a.name), sp(a.span)), |w| {
                for (k, v) in &a.args {
                    let label = match k {
                        Some(k) => format!("arg {}", id(k)),
                        None => "arg".to_string(),
                    };
                    w.expr(&label, v);
                }
            });
        }
    }

    fn module(&mut self, m: &Module) {
        self.nest("", &format!("Module destroy={}", m.has_destroy), |w| {
            for it in &m.items {
                w.item(it);
            }
        });
    }

    fn item(&mut self, it: &Item) {
        self.nest("", &format!("Item {} {}", vis(it.vis), sp(it.span)), |w| {
            w.annotations(&it.annotations);
            match &it.kind {
                ItemKind::Import(list) => w.nest("", "Import", |w| {
                    for (p, s) in list {
                        w.line("path", &format!("{}{}", escape(p), sp(*s)));
                    }
                }),
                ItemKind::Fun(f) => w.fun("", f),
                ItemKind::Def(d) => w.def(d),
                ItemKind::Stmt(s) => w.stmt("", s),
            }
        });
    }

    fn tparams(&mut self, ts: &[Ident]) {
        for t in ts {
            self.line("tparam", &id(t));
        }
    }

    fn params(&mut self, ps: &[Param]) {
        for p in ps {
            self.nest("param", &id(&p.name), |w| w.ty("", &p.ty));
        }
    }

    fn fun(&mut self, label: &str, f: &FunDecl) {
        let head = format!(
            "Fun {} {} async={} static={} abstract={} bodyless={}",
            id(&f.name),
            sp(f.span),
            f.is_async,
            f.is_static,
            f.is_abstract,
            f.bodyless
        );
        self.nest(label, &head, |w| {
            w.annotations(&f.annotations);
            w.tparams(&f.tparams);
            w.params(&f.params);
            if let Some(r) = &f.ret {
                w.ty("ret", r);
            }
            w.block("body", &f.body);
        });
    }

    fn fields(&mut self, fs: &[Field]) {
        for f in fs {
            self.nest("field", &format!("{} {}", id(&f.name), vis(f.vis)), |w| {
                w.annotations(&f.annotations);
                w.ty("type", &f.ty);
                if let Some(d) = &f.default {
                    w.expr("default", d);
                }
            });
        }
    }

    fn super_ref(&mut self, label: &str, r: &SuperRef) {
        self.nest(label, &id(&r.0), |w| {
            if let Some(args) = &r.1 {
                w.nest("args", &args.len().to_string(), |w| {
                    for a in args {
                        w.expr("", a);
                    }
                });
            }
        });
    }

    fn def(&mut self, d: &Def) {
        match d {
            Def::Type { name, fields, tparams } => self.nest("", &format!("Type {}", id(name)), |w| {
                w.tparams(tparams);
                w.fields(fields);
            }),
            Def::Alias { name, ty } => self.nest("", &format!("Alias {}", id(name)), |w| w.ty("", ty)),
            Def::Interface { name, methods } => self.nest("", &format!("Interface {}", id(name)), |w| {
                for m in methods {
                    w.nest("method", &format!("{} async={}", id(&m.name), m.is_async), |w| {
                        w.params(&m.params);
                        if let Some(r) = &m.ret {
                            w.ty("ret", r);
                        }
                    });
                }
            }),
            Def::Struct {
                name,
                kind,
                params,
                param_anns,
                extends,
                supers,
                colon_extra,
                fields,
                methods,
                statics,
                tparams,
            } => {
                let k = match kind {
                    StructKind::Normal => "normal",
                    StructKind::Abstract => "abstract",
                    StructKind::Static => "static",
                };
                self.nest("", &format!("Struct {} {} colon_extra={}", id(name), k, colon_extra), |w| {
                    w.tparams(tparams);
                    w.params(params);
                    for (i, anns) in param_anns.iter().enumerate() {
                        if !anns.is_empty() {
                            w.nest("param_anns", &i.to_string(), |w| w.annotations(anns));
                        }
                    }
                    if let Some(e) = extends {
                        w.super_ref("extends", e);
                    }
                    for s in supers {
                        w.super_ref("super", s);
                    }
                    w.fields(fields);
                    for s in statics {
                        w.nest("static", &format!("{} {} const={}", id(&s.name), vis(s.vis), s.is_const), |w| {
                            if let Some(t) = &s.ty {
                                w.ty("type", t);
                            }
                            w.expr("init", &s.init);
                        });
                    }
                    for (v, m) in methods {
                        w.fun(&format!("method {}", vis(*v)), m);
                    }
                });
            }
            Def::Enum { name, variants, fields } => self.nest("", &format!("Enum {}", id(name)), |w| {
                for (v, fs) in variants.iter().zip(fields) {
                    w.nest("variant", &id(v), |w| {
                        if let Some(fs) = fs {
                            w.nest("fields", &fs.len().to_string(), |w| w.params(fs));
                        }
                    });
                }
            }),
            Def::Annotation { name, fields } => self.nest("", &format!("Annotation {}", id(name)), |w| w.fields(fields)),
        }
    }

    fn ty(&mut self, label: &str, t: &TypeExpr) {
        let s = sp(t.span);
        match &t.kind {
            TypeExprKind::Named(n, args) => self.nest(label, &format!("Named {} {}", n, s), |w| {
                for a in args {
                    w.ty("arg", a);
                }
            }),
            TypeExprKind::Array(e) => self.nest(label, &format!("Array {}", s), |w| w.ty("", e)),
            TypeExprKind::Optional(e) => self.nest(label, &format!("Optional {}", s), |w| w.ty("", e)),
            TypeExprKind::Map(k, v) => self.nest(label, &format!("Map {}", s), |w| {
                w.ty("key", k);
                w.ty("value", v);
            }),
            TypeExprKind::Func(ps, r) => self.nest(label, &format!("Func {}", s), |w| {
                for p in ps {
                    w.ty("param", p);
                }
                if let Some(r) = r {
                    w.ty("ret", r);
                }
            }),
        }
    }

    fn block(&mut self, label: &str, b: &Block) {
        self.nest(label, &format!("Block {}", sp(b.span)), |w| {
            for s in &b.stmts {
                w.stmt("", s);
            }
        });
    }

    fn stmt(&mut self, label: &str, st: &Stmt) {
        let s = sp(st.span);
        match &st.kind {
            StmtKind::Var { name, ty, init, is_const } => self.nest(label, &format!("Var {} const={} {}", id(name), is_const, s), |w| {
                if let Some(t) = ty {
                    w.ty("type", t);
                }
                if let Some(i) = init {
                    w.expr("init", i);
                }
            }),
            StmtKind::Expr(e) => self.nest(label, &format!("ExprStmt {}", s), |w| w.expr("", e)),
            StmtKind::If { cond, then, els } => self.nest(label, &format!("If {}", s), |w| {
                w.expr("cond", cond);
                w.block("then", then);
                if let Some(e) = els {
                    w.block("else", e);
                }
            }),
            StmtKind::While { cond, body } => self.nest(label, &format!("While {}", s), |w| {
                w.expr("cond", cond);
                w.block("body", body);
            }),
            StmtKind::For { init, cond, step, body } => self.nest(label, &format!("For {}", s), |w| {
                if let Some(i) = init {
                    w.stmt("init", i);
                }
                if let Some(c) = cond {
                    w.expr("cond", c);
                }
                if let Some(st) = step {
                    w.expr("step", st);
                }
                w.block("body", body);
            }),
            StmtKind::ForIn { var, index, iter, body } => {
                let idx = index.as_ref().map(|i| format!(" index={}", id(i))).unwrap_or_default();
                self.nest(label, &format!("ForIn {}{} {}", id(var), idx, s), |w| {
                    match iter {
                        ForIter::Range(a, b, inclusive) => w.nest("range", &format!("inclusive={}", inclusive), |w| {
                            w.expr("from", a);
                            w.expr("to", b);
                        }),
                        ForIter::Expr(e) => w.expr("iter", e),
                    }
                    w.block("body", body);
                })
            }
            StmtKind::Return(v) => self.nest(label, &format!("Return {}", s), |w| {
                if let Some(v) = v {
                    w.expr("", v);
                }
            }),
            StmtKind::Break => self.line(label, &format!("Break {}", s)),
            StmtKind::Continue => self.line(label, &format!("Continue {}", s)),
            StmtKind::Block(b) => self.nest(label, &format!("BlockStmt {}", s), |w| w.block("", b)),
            StmtKind::Extend { target, func } => self.nest(label, &format!("Extend {} {}", id(target), s), |w| w.fun("", func)),
            StmtKind::Destroy { target, args } => self.nest(label, &format!("Destroy {} {}", id(target), s), |w| {
                for a in args {
                    w.expr("arg", a);
                }
            }),
        }
    }

    fn exprs(&mut self, label: &str, xs: &[Expr]) {
        for x in xs {
            self.expr(label, x);
        }
    }

    fn expr(&mut self, label: &str, e: &Expr) {
        let s = sp(e.span);
        match &e.kind {
            ExprKind::Int(v) => self.line(label, &format!("Int {} {}", v, s)),
            ExprKind::BigInt(v, hex) => self.line(label, &format!("BigInt {} {} {}", v, if *hex { "hex" } else { "dec" }, s)),
            ExprKind::Float(v) => self.line(label, &format!("Float {} {}", burn_runtime::fmt::float_str(*v), s)),
            ExprKind::Str(v) => self.line(label, &format!("Str {} {}", escape(v), s)),
            ExprKind::Template(parts) => self.nest(label, &format!("Template {}", s), |w| {
                for p in parts {
                    match p {
                        TplExpr::Lit(t) => w.line("lit", &escape(t)),
                        TplExpr::Expr(x) => w.expr("expr", x),
                    }
                }
            }),
            ExprKind::Bool(b) => self.line(label, &format!("Bool {} {}", b, s)),
            ExprKind::Null => self.line(label, &format!("Null {}", s)),
            ExprKind::Ident(n) => self.line(label, &format!("Ident {} {}", n, s)),
            ExprKind::Unary(op, x) => self.nest(label, &format!("Unary {} {}", unop(*op), s), |w| w.expr("", x)),
            ExprKind::Binary(op, a, b) => self.nest(label, &format!("Binary {} {}", op.symbol(), s), |w| {
                w.expr("", a);
                w.expr("", b);
            }),
            ExprKind::Assign { target, op, value } => {
                let o = op.map(|o| o.symbol()).unwrap_or("=");
                self.nest(label, &format!("Assign {} {}", o, s), |w| {
                    w.expr("target", target);
                    w.expr("value", value);
                })
            }
            ExprKind::Call { callee, args } => self.nest(label, &format!("Call {}", s), |w| {
                w.expr("callee", callee);
                w.exprs("arg", args);
            }),
            ExprKind::Field { obj, name } => self.nest(label, &format!("Field {} {}", id(name), s), |w| w.expr("", obj)),
            ExprKind::Index { obj, index } => self.nest(label, &format!("Index {}", s), |w| {
                w.expr("obj", obj);
                w.expr("index", index);
            }),
            ExprKind::Array(xs) => self.nest(label, &format!("Array {}", s), |w| w.exprs("", xs)),
            ExprKind::StructLit { ty, fields } => {
                let t = ty.as_ref().map(id).unwrap_or_else(|| "_".to_string());
                self.nest(label, &format!("StructLit {} {}", t, s), |w| {
                    for (n, v) in fields {
                        w.expr(&format!("field {}", id(n)), v);
                    }
                })
            }
            ExprKind::MapLit(pairs) => self.nest(label, &format!("MapLit {}", s), |w| {
                for (k, v) in pairs {
                    w.expr("key", k);
                    w.expr("value", v);
                }
            }),
            ExprKind::Is(x, t) => self.nest(label, &format!("Is {}", s), |w| {
                w.expr("", x);
                w.ty("type", t);
            }),
            ExprKind::As(x, t) => self.nest(label, &format!("As {}", s), |w| {
                w.expr("", x);
                w.ty("type", t);
            }),
            ExprKind::Await(x) => self.nest(label, &format!("Await {}", s), |w| w.expr("", x)),
            ExprKind::Lambda(f) => self.nest(label, &format!("Lambda {}", s), |w| w.fun("", f)),
            ExprKind::NotNull(x) => self.nest(label, &format!("NotNull {}", s), |w| w.expr("", x)),
            ExprKind::New { ty, targs, args } => self.nest(label, &format!("New {} {}", id(ty), s), |w| {
                for t in targs {
                    w.ty("targ", t);
                }
                w.exprs("arg", args);
            }),
            ExprKind::SafeGet { obj, name, args } => self.nest(label, &format!("SafeGet {} {}", id(name), s), |w| {
                w.expr("obj", obj);
                if let Some(args) = args {
                    w.nest("args", &args.len().to_string(), |w| w.exprs("", args));
                }
            }),
            ExprKind::Coalesce(a, b) => self.nest(label, &format!("Coalesce {}", s), |w| {
                w.expr("", a);
                w.expr("", b);
            }),
            ExprKind::SafeAs(x, t) => self.nest(label, &format!("SafeAs {}", s), |w| {
                w.expr("", x);
                w.ty("type", t);
            }),
            ExprKind::Match { subject, arms } => self.nest(label, &format!("Match {}", s), |w| {
                if let Some(x) = subject {
                    w.expr("subject", x);
                }
                for a in arms {
                    w.nest("arm", &sp(a.span), |w| {
                        for p in &a.patterns {
                            match p {
                                Pattern::Value(x) => w.expr("value", x),
                                Pattern::Range(a, b, inclusive) => w.nest("range", &format!("inclusive={}", inclusive), |w| {
                                    w.expr("from", a);
                                    w.expr("to", b);
                                }),
                                Pattern::Is(t, bind) => {
                                    let b = bind.as_ref().map(id).unwrap_or_else(|| "_".to_string());
                                    w.nest("is", &b, |w| w.ty("", t));
                                }
                            }
                        }
                        if let Some(g) = &a.guard {
                            w.expr("guard", g);
                        }
                        match &a.body {
                            ArmBody::Expr(x) => w.expr("body", x),
                            ArmBody::Block(b) => w.block("body", b),
                        }
                    });
                }
            }),
        }
    }
}

pub fn dump(src: &str) -> String {
    let (toks, mut diags) = crate::lexer::lex(src, 0);
    let (module, pd) = crate::parser::parse_module(toks, 0);
    diags.extend(pd);
    let mut w = W { out: String::new(), depth: 0 };
    w.module(&module);
    for d in &diags {
        w.out.push_str(&format!("error {} {} {}\n", d.span.start, d.span.end, d.message));
    }
    w.out
}

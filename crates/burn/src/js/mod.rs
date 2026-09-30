use crate::hir::{BinOp, Cmp, Conv, Expr, ExprKind, Program, Stmt, UnOp};
use burn_runtime::meta::Desc;
use std::fmt::Write;

const PRELUDE: &str = include_str!("prelude.js");

fn js_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn num(f: f64) -> String {
    if f.is_nan() {
        "NaN".into()
    } else if f.is_infinite() {
        if f > 0.0 {
            "Infinity".into()
        } else {
            "(-Infinity)".into()
        }
    } else {
        let s = format!("{:?}", f);
        if f < 0.0 {
            format!("({})", s)
        } else {
            s
        }
    }
}

fn desc(d: &Desc) -> String {
    match d {
        Desc::Error => "[\"err\"]".into(),
        Desc::Void => "[\"void\"]".into(),
        Desc::Null => "[\"null\"]".into(),
        Desc::Int => "[\"int\"]".into(),
        Desc::Float => "[\"float\"]".into(),
        Desc::Bool => "[\"bool\"]".into(),
        Desc::Str => "[\"str\"]".into(),
        Desc::Any => "[\"any\"]".into(),
        Desc::Array(e) => format!("[\"arr\",{}]", e),
        Desc::Map(k, v) => format!("[\"map\",{},{}]", k, v),
        Desc::Optional(i) => format!("[\"opt\",{}]", i),
        Desc::Func => "[\"fun\"]".into(),
        Desc::Future(i) => format!("[\"future\",{}]", i),
        Desc::Record {
            name,
            fields,
            class,
            implements,
        } => {
            let fs: Vec<String> = fields.iter().map(|(n, t)| format!("[{},{}]", js_str(n), t)).collect();
            let is: Vec<String> = implements.iter().map(|i| i.to_string()).collect();
            format!("[\"rec\",{},[{}],{},[{}]]", js_str(name), fs.join(","), class, is.join(","))
        }
        Desc::Interface { name } => format!("[\"iface\",{}]", js_str(name)),
        Desc::Enum { name, variants } => {
            let vs: Vec<String> = variants.iter().map(|v| js_str(v)).collect();
            format!("[\"enum\",{},[{}]]", js_str(name), vs.join(","))
        }
    }
}

struct Gen<'p> {
    p: &'p Program,
    out: String,
    label: usize,
    loops: Vec<(String, String)>,
    indent: usize,
}

pub fn validate(p: &Program) -> Result<(), String> {
    if let Some(l) = p.libs.first() {
        return Err(format!(
            "`{}` is a bytecode library; the JavaScript backend cannot run bvm code, so use burni, `--target bvm`, `--target bar` or a native build",
            l.path.display()
        ));
    }
    for f in &p.funcs {
        if matches!(f.external, Some(crate::hir::External::Native { .. })) {
            return Err(format!(
                "`{}` is marked @Native, which needs bvm; the JavaScript backend has no host functions",
                f.name
            ));
        }
        if let Some(a) = f.annotations.iter().find(|a| matches!(a.name.as_str(), "Inject" | "Overwrite" | "Redirect")) {
            return Err(format!(
                "@{} on `{}` is a mixin, and mixins change bvm bytecode; the JavaScript backend does not support them",
                a.name, f.name
            ));
        }
    }
    Ok(())
}

pub fn generate(p: &Program) -> String {
    let mut g = Gen {
        p,
        out: String::with_capacity(1 << 16),
        label: 0,
        loops: Vec::new(),
        indent: 0,
    };
    g.program();
    g.out
}

fn cmp_op(c: Cmp) -> &'static str {
    match c {
        Cmp::Eq => "===",
        Cmp::Ne => "!==",
        Cmp::Lt => "<",
        Cmp::Le => "<=",
        Cmp::Gt => ">",
        Cmp::Ge => ">=",
    }
}

impl<'p> Gen<'p> {
    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn program(&mut self) {
        let p = self.p;
        self.out.push_str("#!/usr/bin/env node\n");
        self.out.push_str(PRELUDE);
        let descs: Vec<String> = p.types.descs().iter().map(desc).collect();
        let _ = writeln!(self.out, "const $T = [{}];", descs.join(","));
        let locs: Vec<String> = p.locs.iter().map(|l| js_str(l)).collect();
        let _ = writeln!(self.out, "const $LOCS = [{}];", locs.join(","));
        self.out
            .push_str("function $ci(t, ...a) { const f = t[a[0][0]]; if (!f) $err(\"interface method not implemented\", -1); return f(...a); }\n");
        for (i, s) in p.slots.iter().enumerate() {
            let entries: Vec<String> = s.impls.iter().map(|(t, f)| format!("{}: f{}", t, f)).collect();
            let _ = writeln!(self.out, "const $I{} = {{{}}};", i, entries.join(", "));
        }
        if !p.globals.is_empty() {
            let gs: Vec<String> = (0..p.globals.len()).map(|i| format!("g{} = null", i)).collect();
            let _ = writeln!(self.out, "let {};", gs.join(", "));
        }
        for (i, f) in p.funcs.iter().enumerate() {
            let params: Vec<String> = (0..f.params).map(|s| format!("l{}", s)).collect();
            let _ = writeln!(self.out, "function f{}({}) {{", i, params.join(", "));
            self.indent = 1;
            let n = f.locals.len();
            if n > f.params as usize {
                let ls: Vec<String> = (f.params as usize..n).map(|s| format!("l{}", s)).collect();
                self.line(&format!("let {};", ls.join(", ")));
            }
            for s in &f.body {
                self.stmt(s);
            }
            self.indent = 0;
            self.out.push_str("}\n");
        }
        let _ = writeln!(
            self.out,
            "try {{\n  f{}();\n  $flush();\n}} catch (e) {{\n  $flush();\n  if (e instanceof BurnExit) process.exit(e.code);\n  if (e instanceof BurnError) {{ process.stderr.write(e.message + \"\\n\"); process.exit(1); }}\n  if (e instanceof RangeError) {{ process.stderr.write(\"runtime error: stack overflow (recursion is too deep)\\n\"); process.exit(1); }}\n  throw e;\n}}",
            p.entry
        );
    }

    fn stmts(&mut self, ss: &[Stmt]) {
        for s in ss {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Expr(e) => {
                let x = self.expr(e);
                self.line(&format!("{};", x));
            }
            Stmt::If(c, a, b) => {
                let c = self.expr(c);
                self.line(&format!("if ({}) {{", c));
                self.indent += 1;
                self.stmts(a);
                self.indent -= 1;
                if b.is_empty() {
                    self.line("}");
                } else {
                    self.line("} else {");
                    self.indent += 1;
                    self.stmts(b);
                    self.indent -= 1;
                    self.line("}");
                }
            }
            Stmt::Loop { cond, body, step } => {
                self.label += 1;
                let outer = format!("L{}", self.label);
                let inner = format!("B{}", self.label);
                self.line(&format!("{}: for (;;) {{", outer));
                self.indent += 1;
                if let Some(c) = cond {
                    let c = self.expr(c);
                    self.line(&format!("if (!({})) break;", c));
                }
                self.line(&format!("{}: {{", inner));
                self.indent += 1;
                self.loops.push((outer, inner));
                self.stmts(body);
                self.loops.pop();
                self.indent -= 1;
                self.line("}");
                self.stmts(step);
                self.indent -= 1;
                self.line("}");
            }
            Stmt::Return(Some(e)) => {
                let x = self.expr(e);
                self.line(&format!("return {};", x));
            }
            Stmt::Return(None) => self.line("return 0;"),
            Stmt::Break => {
                let l = self.loops.last().unwrap().0.clone();
                self.line(&format!("break {};", l));
            }
            Stmt::Continue => {
                let l = self.loops.last().unwrap().1.clone();
                self.line(&format!("break {};", l));
            }
        }
    }

    fn args(&mut self, xs: &[Expr]) -> String {
        let parts: Vec<String> = xs.iter().map(|x| self.expr(x)).collect();
        parts.join(", ")
    }

    fn expr(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::TypeId(v) | ExprKind::LocId(v) => v.to_string(),
            ExprKind::Int(v) => {
                if *v < 0 {
                    format!("({})", v)
                } else {
                    v.to_string()
                }
            }
            ExprKind::Float(f) => num(*f),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Str(i) => js_str(&self.p.strings[*i as usize]),
            ExprKind::Null => "null".into(),
            ExprKind::Local(s) => format!("l{}", s),
            ExprKind::Global(g) => format!("g{}", g),
            ExprKind::SetLocal(s, v) => format!("(l{} = {})", s, self.expr(v)),
            ExprKind::SetGlobal(g, v) => format!("(g{} = {})", g, self.expr(v)),
            ExprKind::Unary(op, x) => {
                let x = self.expr(x);
                match op {
                    UnOp::INeg(l) => format!("$ov(-{}, {})", x, l),
                    UnOp::FNeg => format!("(-{})", x),
                    UnOp::Not => format!("(!{})", x),
                }
            }
            ExprKind::Binary(op, a, b) => {
                let (a, b) = (self.expr(a), self.expr(b));
                match op {
                    BinOp::IAdd(u32::MAX) => format!("({} + {})", a, b),
                    BinOp::IAdd(l) => format!("$ov({} + {}, {})", a, b, l),
                    BinOp::ISub(l) => format!("$ov({} - {}, {})", a, b, l),
                    BinOp::IMul(l) => format!("$ov({} * {}, {})", a, b, l),
                    BinOp::FAdd => format!("({} + {})", a, b),
                    BinOp::FSub => format!("({} - {})", a, b),
                    BinOp::FMul => format!("({} * {})", a, b),
                    BinOp::FDiv => format!("({} / {})", a, b),
                    BinOp::IDiv(l) => format!("$idiv({}, {}, {})", a, b, l),
                    BinOp::IMod(l) => format!("$imod({}, {}, {})", a, b, l),
                    BinOp::ICmp(c) | BinOp::FCmp(c) => format!("({} {} {})", a, cmp_op(*c), b),
                }
            }
            ExprKind::And(a, b) => format!("({} && {})", self.expr(a), self.expr(b)),
            ExprKind::Or(a, b) => format!("({} || {})", self.expr(a), self.expr(b)),
            ExprKind::Conv(c, x) => {
                let x = self.expr(x);
                match c {
                    Conv::IntToFloat => x,
                    Conv::FloatToInt => format!("$f2i({})", x),
                }
            }
            ExprKind::Call(f, xs) => format!("f{}({})", f, self.args(xs)),
            ExprKind::CallIndirect(c, xs) => {
                let a = self.args(xs);
                format!("({})({})", self.expr(c), a)
            }
            ExprKind::CallIface(slot, xs) => format!("$ci($I{}, {})", slot, self.args(xs)),
            ExprKind::Rt(f, xs) => format!("$R.{:?}({})", f, self.args(xs)),
            ExprKind::Spawn(f, xs) => format!("({{v: f{}({})}})", f, self.args(xs)),
            ExprKind::FuncRef(f) => format!("f{}", f),
            ExprKind::NewStruct(t, xs) => format!("$rec({}, [{}])", t, self.args(xs)),
            ExprKind::GetField(o, i) => format!("{}[{}]", self.expr(o), i + 1),
            ExprKind::SetField(o, i, v) => {
                let o = self.expr(o);
                format!("({}[{}] = {})", o, i + 1, self.expr(v))
            }
            ExprKind::NewArray(_, xs) => format!("[{}]", self.args(xs)),
            ExprKind::Index(a, i, l) => {
                let a = self.expr(a);
                format!("$idx({}, {}, {})", a, self.expr(i), l)
            }
            ExprKind::SetIndex(a, i, v, l) => {
                let a = self.expr(a);
                let i = self.expr(i);
                format!("$sidx({}, {}, {}, {})", a, i, self.expr(v), l)
            }
            ExprKind::ArrLen(a) => format!("{}.length", self.expr(a)),
            ExprKind::BoxVal(x) => format!("{}.v", self.expr(x)),
            ExprKind::Seq(ss, x) => {
                if ss.iter().all(|s| matches!(s, Stmt::Expr(_))) {
                    let mut parts: Vec<String> = ss
                        .iter()
                        .map(|s| match s {
                            Stmt::Expr(e) => self.expr(e),
                            _ => unreachable!(),
                        })
                        .collect();
                    parts.push(self.expr(x));
                    format!("({})", parts.join(", "))
                } else {
                    let saved = std::mem::take(&mut self.out);
                    let saved_indent = self.indent;
                    self.indent = 0;
                    self.stmts(ss);
                    let body = std::mem::replace(&mut self.out, saved);
                    self.indent = saved_indent;
                    let v = self.expr(x);
                    format!("(() => {{ {} return {}; }})()", body.replace('\n', " "), v)
                }
            }
        }
    }
}

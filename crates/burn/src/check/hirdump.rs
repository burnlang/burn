use super::*;
use crate::hir::{Const, External, Func, Program};
use std::fmt::Write;

fn span(s: Span) -> String {
    format!("{}:{}-{}", s.file, s.start, s.end)
}

fn konst(v: &Const) -> String {
    match v {
        Const::Int(i) => format!("int {}", i),
        Const::Float(f) => format!("float {}", burn_runtime::fmt::float_str(*f)),
        Const::Bool(b) => format!("bool {}", b),
        Const::Str(s) => format!("str {}", crate::lexer::escape(s)),
        Const::Null => "null".into(),
    }
}

fn annotations(out: &mut String, pad: &str, anns: &[hir::Annotation]) {
    for a in anns {
        let args: Vec<String> = a.args.iter().map(|(k, v)| format!("{}={}", k, konst(v))).collect();
        let ty = a.ty.map(|t| t.to_string()).unwrap_or_else(|| "-".into());
        let _ = writeln!(out, "{}@{} ty={} span={} [{}]", pad, a.name, ty, span(a.span), args.join(", "));
    }
}

fn expr(out: &mut String, depth: usize, e: &Expr) {
    let pad = "  ".repeat(depth);
    let (head, kids, stmts): (String, Vec<&Expr>, &[Stmt]) = match &e.kind {
        ExprKind::Int(v) => (format!("Int {}", v), vec![], &[]),
        ExprKind::TypeId(t) => (format!("TypeId {}", t), vec![], &[]),
        ExprKind::LocId(l) => (format!("LocId {}", l), vec![], &[]),
        ExprKind::Float(f) => (format!("Float {}", burn_runtime::fmt::float_str(*f)), vec![], &[]),
        ExprKind::Bool(b) => (format!("Bool {}", b), vec![], &[]),
        ExprKind::Str(s) => (format!("Str {}", s), vec![], &[]),
        ExprKind::Null => ("Null".into(), vec![], &[]),
        ExprKind::Local(l) => (format!("Local {}", l), vec![], &[]),
        ExprKind::Global(g) => (format!("Global {}", g), vec![], &[]),
        ExprKind::SetLocal(l, v) => (format!("SetLocal {}", l), vec![&**v], &[]),
        ExprKind::SetGlobal(g, v) => (format!("SetGlobal {}", g), vec![&**v], &[]),
        ExprKind::Unary(op, v) => (format!("Unary {:?}", op), vec![&**v], &[]),
        ExprKind::Binary(op, a, b) => (format!("Binary {:?}", op), vec![&**a, &**b], &[]),
        ExprKind::And(a, b) => ("And".into(), vec![&**a, &**b], &[]),
        ExprKind::Or(a, b) => ("Or".into(), vec![&**a, &**b], &[]),
        ExprKind::Conv(c, v) => (format!("Conv {:?}", c), vec![&**v], &[]),
        ExprKind::Call(f, args) => (format!("Call {}", f), args.iter().collect(), &[]),
        ExprKind::CallIndirect(f, args) => ("CallIndirect".into(), std::iter::once(&**f).chain(args.iter()).collect(), &[]),
        ExprKind::CallIface(s, args) => (format!("CallIface {}", s), args.iter().collect(), &[]),
        ExprKind::Rt(r, args) => (format!("Rt {:?}", r), args.iter().collect(), &[]),
        ExprKind::Spawn(f, args) => (format!("Spawn {}", f), args.iter().collect(), &[]),
        ExprKind::FuncRef(f) => (format!("FuncRef {}", f), vec![], &[]),
        ExprKind::NewStruct(t, args) => (format!("NewStruct {}", t), args.iter().collect(), &[]),
        ExprKind::GetField(o, i) => (format!("GetField {}", i), vec![&**o], &[]),
        ExprKind::SetField(o, i, v) => (format!("SetField {}", i), vec![&**o, &**v], &[]),
        ExprKind::NewArray(t, items) => (format!("NewArray {}", t), items.iter().collect(), &[]),
        ExprKind::Index(a, i, l) => (format!("Index {}", l), vec![&**a, &**i], &[]),
        ExprKind::SetIndex(a, i, v, l) => (format!("SetIndex {}", l), vec![&**a, &**i, &**v], &[]),
        ExprKind::ArrLen(a) => ("ArrLen".into(), vec![&**a], &[]),
        ExprKind::BoxVal(v) => ("BoxVal".into(), vec![&**v], &[]),
        ExprKind::Seq(ss, v) => ("Seq".into(), vec![&**v], ss),
        ExprKind::Retain(v) => ("Retain".into(), vec![&**v], &[]),
        ExprKind::Release(v) => ("Release".into(), vec![&**v], &[]),
    };
    let _ = writeln!(out, "{}{} : {}", pad, head, e.ty);
    for s in stmts {
        stmt(out, depth + 1, s);
    }
    for k in kids {
        expr(out, depth + 1, k);
    }
}

fn stmts(out: &mut String, depth: usize, list: &[Stmt]) {
    for s in list {
        stmt(out, depth, s);
    }
}

fn stmt(out: &mut String, depth: usize, s: &Stmt) {
    let pad = "  ".repeat(depth);
    match s {
        Stmt::Expr(e) => {
            let _ = writeln!(out, "{}expr", pad);
            expr(out, depth + 1, e);
        }
        Stmt::If(c, a, b) => {
            let _ = writeln!(out, "{}if", pad);
            expr(out, depth + 1, c);
            let _ = writeln!(out, "{}then", pad);
            stmts(out, depth + 1, a);
            let _ = writeln!(out, "{}else", pad);
            stmts(out, depth + 1, b);
            let _ = writeln!(out, "{}end", pad);
        }
        Stmt::Loop { cond, body, step } => {
            let _ = writeln!(out, "{}loop", pad);
            if let Some(c) = cond {
                expr(out, depth + 1, c);
            }
            let _ = writeln!(out, "{}body", pad);
            stmts(out, depth + 1, body);
            let _ = writeln!(out, "{}step", pad);
            stmts(out, depth + 1, step);
            let _ = writeln!(out, "{}end", pad);
        }
        Stmt::Return(v) => {
            let _ = writeln!(out, "{}return", pad);
            if let Some(v) = v {
                expr(out, depth + 1, v);
            }
        }
        Stmt::Break => {
            let _ = writeln!(out, "{}break", pad);
        }
        Stmt::Continue => {
            let _ = writeln!(out, "{}continue", pad);
        }
    }
}

fn func(out: &mut String, i: usize, f: &Func) {
    let ext = match &f.external {
        Some(External::Native { name }) => format!("native {}", name),
        Some(External::Lib { lib, name }) => format!("lib {} {}", lib, name),
        None => "-".into(),
    };
    let locals: Vec<String> = f.locals.iter().map(|t| t.to_string()).collect();
    let _ = writeln!(
        out,
        "func {} {} params={} ret={} async={} span={} end_loc={} external={} locals=[{}]",
        i,
        f.name,
        f.params,
        f.ret,
        f.is_async,
        span(f.span),
        f.end_loc,
        ext,
        locals.join(",")
    );
    annotations(out, "  ", &f.annotations);
    stmts(out, 1, &f.body);
}

fn program(out: &mut String, p: &Program) {
    let _ = writeln!(
        out,
        "program {} entry={} main={} no_std={}",
        p.name,
        p.entry,
        p.main.map(|m| m.to_string()).unwrap_or_else(|| "-".into()),
        p.no_std
    );
    for t in 0..p.types.len() as TyId {
        let _ = writeln!(out, "type {} {}", t, p.types.display(t));
    }
    for (i, f) in p.funcs.iter().enumerate() {
        func(out, i, f);
    }
    for (i, g) in p.globals.iter().enumerate() {
        let _ = writeln!(out, "global {} {} {} {}", i, g.name, g.ty, g.module);
    }
    for (i, s) in p.strings.iter().enumerate() {
        let _ = writeln!(out, "string {} {}", i, crate::lexer::escape(s));
    }
    for (i, l) in p.locs.iter().enumerate() {
        let _ = writeln!(out, "loc {} {}", i, l);
    }
    for (i, s) in p.slots.iter().enumerate() {
        let impls: Vec<String> = s.impls.iter().map(|(t, f)| format!("{}:{}", t, f)).collect();
        let _ = writeln!(out, "slot {} {} argc={} impls=[{}]", i, s.name, s.argc, impls.join(", "));
    }
    for (k, f) in &p.inits {
        let _ = writeln!(out, "init {} {}", k, f);
    }
    for (t, anns) in &p.type_annotations {
        let _ = writeln!(out, "type_annotations {}", t);
        annotations(out, "  ", anns);
    }
}

pub enum Form {
    Checked,
    Owned,
    Bytecode,
}

pub fn dump(loaded: &Loaded, form: Form) -> String {
    let r = check(loaded, CheckOptions::default());
    let mut out = String::new();
    for d in &r.diags {
        out.push_str(&crate::diag::render(&loaded.sm, d, false));
    }
    match &r.program {
        Some(p) => match form {
            Form::Checked => program(&mut out, p),
            Form::Owned => program(&mut out, &crate::own::lower(p)),
            Form::Bytecode => out.push_str(&bvm::asm::disassemble(&crate::vm::module(p))),
        },
        None => out.push_str("no program\n"),
    }
    out
}

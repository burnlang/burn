use super::*;
use crate::hir::Const;
use std::fmt::Write;

fn opt_ty(c: &Checker, t: Option<TyId>) -> String {
    t.map(|t| t.to_string() + " " + &c.show(t)).unwrap_or_else(|| "-".into())
}

fn opt_num(v: Option<u32>) -> String {
    v.map(|v| v.to_string()).unwrap_or_else(|| "-".into())
}

fn vis(v: Vis) -> &'static str {
    match v {
        Vis::Pub => "pub",
        Vis::Priv => "priv",
        Vis::Default => "default",
    }
}

fn span(s: Span) -> String {
    format!("{}:{}-{}", s.file, s.start, s.end)
}

fn konst(v: &Const) -> String {
    match v {
        Const::Int(i) => format!("int {}", i),
        Const::Float(f) => format!("float {}", bvm::runtime::fmt::float_str(*f)),
        Const::Bool(b) => format!("bool {}", b),
        Const::Str(s) => format!("str {}", crate::lexer::escape(s)),
        Const::Null => "null".into(),
    }
}

fn annotations(out: &mut String, pad: &str, anns: &[hir::Annotation]) {
    for a in anns {
        let args: Vec<String> = a.args.iter().map(|(k, v)| format!("{}={}", k, konst(v))).collect();
        let _ = writeln!(out, "{}@{} ty={} span={} [{}]", pad, a.name, opt_num(a.ty), span(a.span), args.join(", "));
    }
}

fn kind(t: &Ty) -> String {
    let list = |ts: &[TyId]| ts.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(",");
    match t {
        Ty::Error => "Error".into(),
        Ty::Void => "Void".into(),
        Ty::Null => "Null".into(),
        Ty::Int => "Int".into(),
        Ty::Float => "Float".into(),
        Ty::Bool => "Bool".into(),
        Ty::Str => "Str".into(),
        Ty::Any => "Any".into(),
        Ty::Num(n) => format!("Num {}", n.name()),
        Ty::Array(e) => format!("Array {}", e),
        Ty::Map(k, v) => format!("Map {} {}", k, v),
        Ty::Optional(i) => format!("Optional {}", i),
        Ty::Func(ps, r) => format!("Func ({}) {}", list(ps), r),
        Ty::Future(i) => format!("Future {}", i),
        Ty::Record(i) => format!("Record {}", i),
        Ty::Interface(i) => format!("Interface {}", i),
        Ty::Enum(i) => format!("Enum {}", i),
    }
}

fn sorted<T: Clone>(m: &HashMap<String, T>) -> Vec<(String, T)> {
    let mut v: Vec<(String, T)> = m.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

pub fn dump(c: &Checker, loaded: &Loaded) -> String {
    let mut out = String::new();
    for t in 0..c.types.len() as TyId {
        let _ = writeln!(out, "type {} {} = {}", t, kind(c.types.get(t)), c.show(t));
    }
    for (i, r) in c.types.records.iter().enumerate() {
        let _ = writeln!(
            out,
            "record {} {} ty={} module={} span={} class={} abstract={} static={} anon={} parent={} ctor={} init={}",
            i,
            r.name,
            r.ty,
            r.module,
            span(r.span),
            r.is_class,
            r.is_abstract,
            r.is_static,
            r.anon,
            opt_num(r.parent),
            opt_num(r.ctor),
            opt_num(r.init)
        );
        for f in &r.fields {
            let _ = writeln!(
                out,
                "  field {} {} private={} default={} span={}",
                f.name,
                opt_ty(c, Some(f.ty)),
                f.private,
                f.default.is_some(),
                span(f.span)
            );
        }
        let imps: Vec<String> = r.implements.iter().map(|i| i.to_string()).collect();
        let _ = writeln!(out, "  implements [{}]", imps.join(", "));
        for (n, f) in sorted(&r.methods) {
            let _ = writeln!(out, "  method {} {}", n, f);
        }
        for (n, f) in sorted(&r.statics) {
            let _ = writeln!(out, "  static {} {}", n, f);
        }
        for (n, g) in sorted(&r.static_vals) {
            let _ = writeln!(out, "  static_val {} {}", n, g);
        }
    }
    for (i, f) in c.types.ifaces.iter().enumerate() {
        let _ = writeln!(out, "iface {} {} ty={} module={} span={}", i, f.name, f.ty, f.module, span(f.span));
        for m in &f.methods {
            let ps: Vec<String> = m.params.iter().map(|p| p.to_string()).collect();
            let _ = writeln!(
                out,
                "  method {} ({}) {} async={} slot={} span={}",
                m.name,
                ps.join(","),
                m.ret,
                m.is_async,
                m.slot,
                span(m.span)
            );
        }
        for v in &f.variants {
            let _ = writeln!(out, "  variant {} record={} span={}", v.name, v.record, span(v.span));
        }
    }
    for (i, e) in c.types.enums.iter().enumerate() {
        let _ = writeln!(out, "enum {} {} ty={} module={} span={}", i, e.name, e.ty, e.module, span(e.span));
        for (n, s) in &e.variants {
            let _ = writeln!(out, "  variant {} span={}", n, span(*s));
        }
    }
    for (i, m) in c.mods.iter().enumerate() {
        let imps: Vec<String> = m.imports.iter().map(|i| i.to_string()).collect();
        let _ = writeln!(
            out,
            "module {} file={} init={} std={} snippet={} import_at={},{} imports=[{}]",
            i,
            m.file,
            m.init,
            m.std_name.as_deref().unwrap_or("-"),
            m.snippet,
            m.import_at.0,
            m.import_at.1,
            imps.join(", ")
        );
        for (n, e) in sorted(&m.values) {
            let what = match e.sym {
                ValSym::Func(f) => format!("func {}", f),
                ValSym::Global(g) => format!("global {}", g),
            };
            let _ = writeln!(out, "  value {} {} {} span={}", n, what, vis(e.vis), span(e.span));
        }
        for (n, e) in sorted(&m.types) {
            let _ = writeln!(out, "  type {} {} {} span={}", n, opt_ty(c, Some(e.sym)), vis(e.vis), span(e.span));
        }
        for (n, e) in sorted(&m.gfuncs) {
            let _ = writeln!(out, "  gfunc {} {} {} span={}", n, e.sym, vis(e.vis), span(e.span));
        }
        for (n, e) in sorted(&m.gtypes) {
            let _ = writeln!(out, "  gtype {} {} {} span={}", n, e.sym, vis(e.vis), span(e.span));
        }
    }
    for (i, f) in c.funcs.iter().enumerate() {
        let ps: Vec<String> = f
            .params
            .iter()
            .map(|(n, t, s)| format!("{}: {} {}", n, opt_ty(c, Some(*t)), span(*s)))
            .collect();
        let ext = match &f.external {
            Some(hir::External::Native { name }) => format!("native {}", name),
            Some(hir::External::Lib { lib, name }) => format!("lib {} {}", lib, name),
            None => "-".into(),
        };
        let state = match f.state {
            FnState::Pending => "pending",
            FnState::InProgress => "in_progress",
            FnState::Done => "done",
        };
        let _ = writeln!(
            out,
            "func {} {} module={} span={} ret={} async={} static={} private={} self={} state={} body={} deprecated={} external={}",
            i,
            f.name,
            f.module,
            span(f.span),
            opt_ty(c, f.ret),
            f.is_async,
            f.is_static,
            f.private,
            opt_ty(c, f.self_ty),
            state,
            f.decl.is_some(),
            f.deprecated.as_deref().map(crate::lexer::escape).unwrap_or_else(|| "-".into()),
            ext
        );
        for p in ps {
            let _ = writeln!(out, "  param {}", p);
        }
        if let Some(env) = &f.tenv {
            for (n, t) in sorted(env) {
                let _ = writeln!(out, "  tparam {} {}", n, opt_ty(c, Some(t)));
            }
        }
        annotations(&mut out, "  ", &f.annotations);
    }
    for (i, g) in c.globals.iter().enumerate() {
        let _ = writeln!(
            out,
            "global {} {} ty={} const={} module={} span={} private={}",
            i,
            g.name,
            opt_ty(c, g.ty),
            g.is_const,
            g.module,
            span(g.span),
            c.static_private.contains(&(i as u32))
        );
    }
    for (i, s) in c.slots.iter().enumerate() {
        let impls: Vec<String> = s.impls.iter().map(|(t, f)| format!("{}:{}", t, f)).collect();
        let _ = writeln!(out, "slot {} {} argc={} impls=[{}]", i, s.name, s.argc, impls.join(", "));
    }
    for (i, g) in c.generic_fns.iter().enumerate() {
        let _ = writeln!(out, "generic_fn {} {} module={} private={}", i, g.decl.name.name, g.module, g.private);
    }
    for (i, g) in c.generic_types.iter().enumerate() {
        let mut inst: Vec<(Vec<TyId>, TyId)> = g.instances.iter().map(|(k, v)| (k.clone(), *v)).collect();
        inst.sort();
        let _ = writeln!(out, "generic_type {} {} module={}", i, g.def.name().name, g.module);
        for (args, t) in inst {
            let a: Vec<String> = args.iter().map(|x| x.to_string()).collect();
            let _ = writeln!(out, "  instance [{}] {}", a.join(","), t);
        }
    }
    for (gid, ri, targs, s) in &c.pending_instances {
        let a: Vec<String> = targs.iter().map(|x| x.to_string()).collect();
        let _ = writeln!(out, "pending {} record={} [{}] span={}", gid, ri, a.join(","), span(*s));
    }
    let mut keys: Vec<TyId> = c.annotation_types.iter().copied().collect();
    keys.sort();
    for t in keys {
        let _ = writeln!(out, "annotation_type {}", t);
    }
    let mut keys: Vec<TyId> = c.type_anns.keys().copied().collect();
    keys.sort();
    for t in keys {
        let _ = writeln!(out, "type_annotations {}", t);
        annotations(&mut out, "  ", &c.type_anns[&t]);
    }
    let mut keys: Vec<TyId> = c.deprecated_types.keys().copied().collect();
    keys.sort();
    for t in keys {
        let _ = writeln!(out, "deprecated_type {} {}", t, crate::lexer::escape(&c.deprecated_types[&t]));
    }
    let mut keys: Vec<u32> = c.struct_decls.keys().copied().collect();
    keys.sort();
    for ri in keys {
        let d = &c.struct_decls[&ri];
        let ps: Vec<String> = d.params.iter().map(|(n, t, _)| format!("{}: {}", n, t)).collect();
        let pf: Vec<String> = d.param_fields.iter().map(|b| b.to_string()).collect();
        let _ = writeln!(
            out,
            "struct_decl {} module={} params=[{}] super_args={} super_span={} own_start={} param_fields=[{}] statics={}",
            ri,
            d.module,
            ps.join(", "),
            d.super_args.as_ref().map(|a| a.len().to_string()).unwrap_or_else(|| "-".into()),
            span(d.super_span),
            d.own_start,
            pf.join(","),
            d.statics.len()
        );
    }
    let mut keys: Vec<(u32, String)> = c.abstract_sigs.keys().cloned().collect();
    keys.sort();
    for k in keys {
        let s = &c.abstract_sigs[&k];
        let ps: Vec<String> = s.params.iter().map(|p| p.to_string()).collect();
        let _ = writeln!(
            out,
            "abstract {} {} ({}) {} async={} span={}",
            k.0,
            k.1,
            ps.join(","),
            s.ret,
            s.is_async,
            span(s.span)
        );
    }
    let mut keys: Vec<(u32, u32)> = c.iface_slot_base.iter().map(|(k, v)| (*k, *v)).collect();
    keys.sort();
    for (i, b) in keys {
        let _ = writeln!(out, "slot_base {} {}", i, b);
    }
    let _ = writeln!(out, "root {} no_std={}", loaded.root, c.no_std);
    for d in &c.diags {
        out.push_str(&crate::diag::render(&loaded.sm, d, false));
    }
    out
}

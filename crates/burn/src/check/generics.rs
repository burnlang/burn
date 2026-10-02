use super::*;
use crate::ast::ExprKind as A;

pub struct GenericFn {
    pub decl: Rc<ast::FunDecl>,
    pub module: usize,
    pub private: bool,
    pub instances: HashMap<Vec<TyId>, FuncId>,
}

pub struct GenericType {
    pub def: Rc<Def>,
    pub module: usize,
    pub instances: HashMap<Vec<TyId>, TyId>,
}

pub enum Inputs<'e> {
    Positional(&'e [ast::Expr]),
    Named(&'e [(ast::Ident, ast::Expr)]),
}

fn mentions(te: &TypeExpr, names: &[String]) -> bool {
    match &te.kind {
        TypeExprKind::Named(n, args) => (args.is_empty() && names.contains(n)) || args.iter().any(|a| mentions(a, names)),
        TypeExprKind::Array(e) | TypeExprKind::Optional(e) => mentions(e, names),
        TypeExprKind::Map(k, v) => mentions(k, names) || mentions(v, names),
        TypeExprKind::Func(ps, r) => ps.iter().any(|p| mentions(p, names)) || r.as_ref().map(|r| mentions(r, names)).unwrap_or(false),
    }
}

const MAX_INSTANCES: usize = 256;

impl<'a> Checker<'a> {
    pub fn declare_generic_type(&mut self, mi: usize, d: &Def, vis: Vis) {
        let name = d.name();
        if Self::primitive(&name.name).is_some() {
            self.error(name.span, format!("`{}` is a built-in type name", name.name));
        }
        if let Some(prev) = self.mods[mi]
            .types
            .get(&name.name)
            .map(|e| e.span)
            .or_else(|| self.mods[mi].gtypes.get(&name.name).map(|e| e.span))
        {
            let (l, _) = self.sm.file(prev.file).line_col(prev.start as usize);
            self.error(name.span, format!("type `{}` is already defined on line {}", name.name, l));
            return;
        }
        let mut seen: Vec<&str> = Vec::new();
        for t in d.tparams() {
            if seen.contains(&t.name.as_str()) {
                self.error(t.span, format!("type parameter `{}` is listed twice", t.name));
            }
            seen.push(&t.name);
        }
        if let Def::Struct { extends, kind, statics, .. } = d {
            if let Some((p, _)) = extends {
                self.error(p.span, "generic structs cannot extend other structs yet");
            }
            if *kind != ast::StructKind::Normal {
                self.error(name.span, "abstract and static structs cannot have type parameters");
            }
            if let Some(sv) = statics.first() {
                self.error(sv.name.span, "generic structs cannot have static members yet");
            }
        }
        let gid = self.generic_types.len() as u32;
        self.generic_types.push(GenericType {
            def: Rc::new(d.clone()),
            module: mi,
            instances: HashMap::new(),
        });
        self.mods[mi].gtypes.insert(
            name.name.clone(),
            Entry {
                sym: gid,
                vis,
                span: name.span,
            },
        );
    }

    pub fn declare_generic_fn(&mut self, mi: usize, f: &ast::FunDecl, vis: Vis) {
        if let Some(prev) = self.mods[mi]
            .values
            .get(&f.name.name)
            .map(|e| e.span)
            .or_else(|| self.mods[mi].gfuncs.get(&f.name.name).map(|e| e.span))
        {
            let (l, _) = self.sm.file(prev.file).line_col(prev.start as usize);
            self.error(f.name.span, format!("`{}` is already defined on line {}", f.name.name, l));
            return;
        }
        if f.bodyless {
            self.error(f.name.span, "functions with type parameters need a body");
        }
        let names: Vec<String> = f.tparams.iter().map(|t| t.name.clone()).collect();
        let mut env = HashMap::new();
        for n in &names {
            env.insert(n.clone(), T_ANY);
        }
        self.tenv.push(Rc::new(env));
        for p in &f.params {
            self.resolve_type_in(&p.ty, mi);
        }
        if let Some(r) = &f.ret {
            self.resolve_type_in(r, mi);
        }
        self.tenv.pop();
        for t in &f.tparams {
            if !f.params.iter().any(|p| mentions(&p.ty, std::slice::from_ref(&t.name)))
                && !f.ret.as_ref().map(|r| mentions(r, std::slice::from_ref(&t.name))).unwrap_or(false)
            {
                self.warn(t.span, format!("type parameter `{}` is not used in the parameters or the return type", t.name));
            }
        }
        let gid = self.generic_fns.len() as u32;
        self.generic_fns.push(GenericFn {
            decl: Rc::new(f.clone()),
            module: mi,
            private: vis == Vis::Priv,
            instances: HashMap::new(),
        });
        self.mods[mi].gfuncs.insert(
            f.name.name.clone(),
            Entry {
                sym: gid,
                vis,
                span: f.name.span,
            },
        );
    }

    pub fn lookup_generic_fn(&self, name: &str) -> Option<u32> {
        let m = self.cur_module();
        self.visible(m, name, |s| &s.gfuncs).ok().flatten().map(|e| e.sym)
    }

    pub fn lookup_generic_type(&self, module: usize, name: &str) -> Option<u32> {
        self.visible(module, name, |s| &s.gtypes).ok().flatten().map(|e| e.sym)
    }

    pub fn tparam(&self, name: &str) -> Option<TyId> {
        self.tenv.last().and_then(|e| e.get(name).copied())
    }

    pub fn resolve_generic_named(&mut self, name: &str, args: &[TypeExpr], module: usize, span: Span) -> Option<TyId> {
        if args.is_empty() {
            if let Some(t) = self.tparam(name) {
                return Some(t);
            }
            if let Some(gid) = self.lookup_generic_type(module, name) {
                if self.types_named(module, name) {
                    return None;
                }
                let ps: Vec<String> = self.generic_types[gid as usize].def.tparams().iter().map(|t| t.name.clone()).collect();
                self.emit(Diagnostic::error(span, format!("`{}` needs type arguments", name)).help(format!(
                    "write `{}<{}>` with the types to use",
                    name,
                    ps.join(", ")
                )));
                return Some(T_ERROR);
            }
            return None;
        }
        let gid = self.lookup_generic_type(module, name)?;
        let want = self.generic_types[gid as usize].def.tparams().len();
        let targs: Vec<TyId> = args.iter().map(|a| self.resolve_type_in(a, module)).collect();
        if targs.len() != want {
            self.error(span, format!("`{}` takes {} type argument(s) but {} were given", name, want, targs.len()));
            return Some(T_ERROR);
        }
        if targs.contains(&T_ERROR) {
            return Some(T_ERROR);
        }
        Some(self.instantiate_type(gid, targs, span))
    }

    fn types_named(&self, module: usize, name: &str) -> bool {
        matches!(self.visible(module, name, |s| &s.types), Ok(Some(_)))
    }

    pub fn instantiate_type(&mut self, gid: u32, targs: Vec<TyId>, span: Span) -> TyId {
        if let Some(t) = self.generic_types[gid as usize].instances.get(&targs) {
            return *t;
        }
        let def = self.generic_types[gid as usize].def.clone();
        let mi = self.generic_types[gid as usize].module;
        if self.generic_types[gid as usize].instances.len() >= MAX_INSTANCES {
            self.error(
                span,
                format!(
                    "`{}` is used with too many different type arguments; does it contain itself with a bigger type?",
                    def.name().name
                ),
            );
            return T_ERROR;
        }
        let shown: Vec<String> = targs.iter().map(|t| self.show(*t)).collect();
        let name = format!("{}<{}>", def.name().name, shown.join(", "));
        let is_class = matches!(&*def, Def::Struct { .. });
        let ri = self.types.new_record(RecordDef {
            name,
            is_class,
            module: mi as u32,
            span: def.name().span,
            ..Default::default()
        });
        let t = self.types.records[ri as usize].ty;
        self.generic_types[gid as usize].instances.insert(targs.clone(), t);
        self.instance_of.insert(t, (gid, targs.clone()));
        self.pending_instances.push((gid, ri, targs, span));
        if self.structs_ready {
            self.complete_instances();
        }
        t
    }

    pub fn complete_instances(&mut self) {
        while !self.pending_instances.is_empty() {
            let (gid, ri, targs, span) = self.pending_instances.remove(0);
            self.complete_instance(gid, ri, targs, span);
        }
    }

    fn env_for(names: &[ast::Ident], targs: &[TyId]) -> Rc<HashMap<String, TyId>> {
        Rc::new(names.iter().map(|n| n.name.clone()).zip(targs.iter().copied()).collect())
    }

    fn complete_instance(&mut self, gid: u32, ri: u32, targs: Vec<TyId>, site: Span) {
        let def = self.generic_types[gid as usize].def.clone();
        let mi = self.generic_types[gid as usize].module;
        let t = self.types.records[ri as usize].ty;
        let env = Self::env_for(def.tparams(), &targs);
        self.tenv.push(env);
        let init = self.mods[mi].init;
        let ctx = self.new_ctx(init, mi, false);
        self.fx.push(ctx);
        self.fill_def_for(mi, &def, t);
        if let Def::Struct { name, methods, .. } = &*def {
            let mut state = vec![0u8; self.types.records.len()];
            self.link_struct(ri, &mut state);
            let iname = ast::Ident {
                name: self.types.records[ri as usize].name.clone(),
                span: name.span,
            };
            self.declare_struct_members(mi, ri, &iname, methods, &[]);
            let rec = &self.types.records[ri as usize];
            let members: Vec<FuncId> = rec.methods.values().chain(rec.init.iter()).chain(rec.ctor.iter()).copied().collect();
            let iname = rec.name.clone();
            for f in members {
                self.inst_sites.insert(f, (iname.clone(), site));
            }
            self.check_record_conformance(ri as usize);
            if self.types.records[ri as usize].ctor.is_some() {
                self.build_ctor(ri);
            }
        }
        self.fx.pop();
        self.tenv.pop();
    }

    pub fn instantiate_fn(&mut self, gid: u32, targs: Vec<TyId>, site: Span) -> FuncId {
        if let Some(f) = self.generic_fns[gid as usize].instances.get(&targs) {
            return *f;
        }
        let decl = self.generic_fns[gid as usize].decl.clone();
        let mi = self.generic_fns[gid as usize].module;
        let private = self.generic_fns[gid as usize].private;
        let env = Self::env_for(&decl.tparams, &targs);
        self.tenv.push(env);
        let fid = self.declare_fun(mi, &decl, None, private, false);
        self.tenv.pop();
        let shown: Vec<String> = targs.iter().map(|t| self.show(*t)).collect();
        self.funcs[fid as usize].name = format!("{}<{}>", decl.name.name, shown.join(", "));
        self.inst_sites.insert(fid, (self.funcs[fid as usize].name.clone(), site));
        self.generic_fns[gid as usize].instances.insert(targs, fid);
        fid
    }

    fn unify(&self, te: &TypeExpr, actual: TyId, names: &[String], binds: &mut HashMap<String, TyId>) {
        if actual == T_ERROR {
            return;
        }
        match &te.kind {
            TypeExprKind::Named(n, args) if args.is_empty() && names.contains(n) => {
                if actual == T_NULL || actual == T_VOID {
                    return;
                }
                match binds.get(n).copied() {
                    None => {
                        binds.insert(n.clone(), actual);
                    }
                    Some(prev) if prev == actual => {}
                    Some(prev) => {
                        let merged = if (prev == T_INT && actual == T_FLOAT) || (prev == T_FLOAT && actual == T_INT) {
                            Some(T_FLOAT)
                        } else if self.types.unwrap_optional(prev) == actual {
                            Some(prev)
                        } else if self.types.unwrap_optional(actual) == prev {
                            Some(actual)
                        } else {
                            self.common_type(prev, actual)
                        };
                        if let Some(m) = merged {
                            binds.insert(n.clone(), m);
                        }
                    }
                }
            }
            TypeExprKind::Named(n, args) => {
                if let Some((gid, targs)) = self.instance_of.get(&actual) {
                    let tn = &self.generic_types[*gid as usize].def.name().name;
                    if tn == n && targs.len() == args.len() {
                        for (a, t) in args.iter().zip(targs.iter()) {
                            self.unify(a, *t, names, binds);
                        }
                    }
                    return;
                }
                match (n.as_str(), args.len(), self.types.get(actual).clone()) {
                    ("Future" | "Task" | "Promise", 1, Ty::Future(x)) => self.unify(&args[0], x, names, binds),
                    ("Array" | "List" | "array", 1, Ty::Array(x)) => self.unify(&args[0], x, names, binds),
                    ("Optional" | "Option", 1, Ty::Optional(x)) => self.unify(&args[0], x, names, binds),
                    ("Optional" | "Option", 1, _) => self.unify(&args[0], actual, names, binds),
                    ("Map" | "map" | "Dict", 2, Ty::Map(k, v)) => {
                        self.unify(&args[0], k, names, binds);
                        self.unify(&args[1], v, names, binds);
                    }
                    _ => {}
                }
            }
            TypeExprKind::Array(e) => {
                if let Ty::Array(x) = self.types.get(actual).clone() {
                    self.unify(e, x, names, binds);
                }
            }
            TypeExprKind::Optional(e) => match self.types.get(actual).clone() {
                Ty::Optional(x) => self.unify(e, x, names, binds),
                Ty::Null => {}
                _ => self.unify(e, actual, names, binds),
            },
            TypeExprKind::Map(k, v) => {
                if let Ty::Map(x, y) = self.types.get(actual).clone() {
                    self.unify(k, x, names, binds);
                    self.unify(v, y, names, binds);
                }
            }
            TypeExprKind::Func(ps, r) => {
                if let Ty::Func(aps, ar) = self.types.get(actual).clone() {
                    for (p, a) in ps.iter().zip(aps.iter()) {
                        self.unify(p, *a, names, binds);
                    }
                    if let Some(r) = r {
                        self.unify(r, ar, names, binds);
                    }
                }
            }
        }
    }

    fn infer_arg(&mut self, e: &ast::Expr) -> TyId {
        let dry = self.fx.last().map(|c| c.dry).unwrap_or(false);
        if let Some(c) = self.fx.last_mut() {
            c.dry = true;
        }
        let narrow = self.fx.last().map(|c| c.narrow.clone());
        self.inferring += 1;
        let t = match &e.kind {
            A::Null => T_NULL,
            _ => self.expr(e, None).ty,
        };
        self.inferring -= 1;
        if let Some(c) = self.fx.last_mut() {
            c.dry = dry;
            if let Some(n) = narrow {
                c.narrow = n;
            }
        }
        t
    }

    fn report_unbound(&mut self, what: &str, names: &[String], binds: &HashMap<String, TyId>, span: Span, example: String) -> bool {
        let missing: Vec<&String> = names.iter().filter(|n| !binds.contains_key(*n)).collect();
        if missing.is_empty() {
            return false;
        }
        let list: Vec<String> = missing.iter().map(|m| format!("`{}`", m)).collect();
        self.emit(
            Diagnostic::error(span, format!("cannot tell which type to use for {} in {}", list.join(", "), what))
                .help(format!("give the value a declared type, for example `{}`", example)),
        );
        true
    }

    pub fn generic_call(&mut self, gid: u32, recv: Option<Expr>, args: &[ast::Expr], span: Span, name_span: Span, expected: Option<TyId>) -> Expr {
        let decl = self.generic_fns[gid as usize].decl.clone();
        let names: Vec<String> = decl.tparams.iter().map(|t| t.name.clone()).collect();
        let mut binds: HashMap<String, TyId> = HashMap::new();
        let offset = recv.is_some() as usize;
        if let (Some(r), Some(p)) = (&recv, decl.params.first()) {
            self.unify(&p.ty, r.ty, &names, &mut binds);
        }
        for (i, a) in args.iter().enumerate() {
            let Some(p) = decl.params.get(i + offset) else { continue };
            if mentions(&p.ty, &names) {
                let t = self.infer_arg(a);
                self.unify(&p.ty, t, &names, &mut binds);
            }
        }
        if let (Some(r), Some(exp)) = (&decl.ret, expected) {
            if names.iter().any(|n| !binds.contains_key(n)) {
                self.unify(r, exp, &names, &mut binds);
            }
        }
        self.def_link(name_span, decl.name.span);
        if self.report_unbound(&format!("this call to `{}`", decl.name.name), &names, &binds, span, "[int] items = []".into()) {
            for a in args {
                self.expr(a, None);
            }
            return Self::err_expr();
        }
        let targs: Vec<TyId> = names.iter().map(|n| binds[n]).collect();
        let fid = self.instantiate_fn(gid, targs, span);
        self.direct_call(fid, recv, args, span, name_span)
    }

    pub fn generic_target(&mut self, name: &ast::Ident, targs: &[TypeExpr], expected: Option<TyId>, inputs: Inputs) -> Option<TyId> {
        let m = self.cur_module();
        let gid = self.lookup_generic_type(m, &name.name)?;
        if self.types_named(m, &name.name) {
            return None;
        }
        let def = self.generic_types[gid as usize].def.clone();
        self.def_link(name.span, def.name().span);
        if !targs.is_empty() {
            let te = TypeExpr {
                kind: TypeExprKind::Named(name.name.clone(), targs.to_vec()),
                span: name.span,
            };
            return Some(self.resolve_type(&te));
        }
        if let Some(exp) = expected.map(|t| self.types.unwrap_optional(t)) {
            if self.instance_of.get(&exp).map(|(g, _)| *g) == Some(gid) {
                return Some(exp);
            }
        }
        let names: Vec<String> = def.tparams().iter().map(|t| t.name.clone()).collect();
        let mut binds = HashMap::new();
        let (fields, params): (&[ast::Field], &[ast::Param]) = match &*def {
            Def::Type { fields, .. } => (fields, &[]),
            Def::Struct { fields, params, .. } => (fields, params),
            _ => (&[], &[]),
        };
        let inputs_kind = match &inputs {
            Inputs::Positional(a) => Inputs::Positional(a),
            Inputs::Named(g) => Inputs::Named(g),
        };
        match inputs {
            Inputs::Positional(args) => {
                for (i, a) in args.iter().enumerate() {
                    let te = if params.is_empty() {
                        fields.get(i).map(|f| &f.ty)
                    } else {
                        params.get(i).map(|p| &p.ty)
                    };
                    if let Some(te) = te {
                        if mentions(te, &names) {
                            let t = self.infer_arg(a);
                            self.unify(te, t, &names, &mut binds);
                        }
                    }
                }
            }
            Inputs::Named(given) => {
                for (n, e) in given {
                    if let Some(f) = fields.iter().find(|f| f.name.name == n.name) {
                        if mentions(&f.ty, &names) {
                            let t = self.infer_arg(e);
                            self.unify(&f.ty, t, &names, &mut binds);
                        }
                    }
                }
            }
        }
        if let Inputs::Named(given) = &inputs_kind {
            for f in fields {
                if mentions(&f.ty, &names)
                    && f.default.is_none()
                    && !given.iter().any(|(n, _)| n.name == f.name.name)
                    && names.iter().any(|n| !binds.contains_key(n) && mentions(&f.ty, std::slice::from_ref(n)))
                {
                    self.error(name.span, format!("missing field `{}` in `{}`", f.name.name, name.name));
                    return Some(T_ERROR);
                }
            }
        }
        let example = format!("{}<{}>", name.name, names.join(", "));
        if self.report_unbound(&format!("`{}`", name.name), &names, &binds, name.span, example) {
            return Some(T_ERROR);
        }
        let targs: Vec<TyId> = names.iter().map(|n| binds[n]).collect();
        Some(self.instantiate_type(gid, targs, name.span))
    }
}

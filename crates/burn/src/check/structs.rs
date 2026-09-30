use super::*;
use crate::ast::ExprKind as A;
use crate::diag::Diagnostic;

fn literal_type(e: &ast::Expr) -> Option<TyId> {
    match &e.kind {
        A::Int(_) => Some(T_INT),
        A::Float(_) => Some(T_FLOAT),
        A::Str(_) | A::Template(_) => Some(T_STR),
        A::Bool(_) => Some(T_BOOL),
        A::Unary(ast::UnOp::Neg, x) => literal_type(x).filter(|t| *t == T_INT || *t == T_FLOAT),
        _ => None,
    }
}

impl<'a> Checker<'a> {
    #[allow(clippy::too_many_arguments)]
    fn operator_span(&self, name: Span) -> Option<Span> {
        let f = self.sm.file(name.file);
        let bytes = f.src.as_bytes();
        let mut i = name.start as usize;
        while i > 0 && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            i -= 1;
        }
        if i >= 2 && bytes[i - 1] == b':' && bytes[i - 2] == b':' {
            return Some(Span::new(name.file, i - 2, i));
        }
        if i >= 1 && bytes[i - 1] == b':' {
            return Some(Span::new(name.file, i - 1, i));
        }
        None
    }

    fn comma_before(&self, name: Span) -> Option<Span> {
        let f = self.sm.file(name.file);
        let bytes = f.src.as_bytes();
        let mut i = name.start as usize;
        while i > 0 && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            i -= 1;
        }
        (i >= 1 && bytes[i - 1] == b',').then(|| Span::new(name.file, i - 1, i))
    }

    fn struct_or_error(&mut self, mi: usize, s: &ast::Ident) -> bool {
        let saved = self.diags.len();
        let r = match self.lookup_type_name(mi, &s.name, s.span) {
            Some(t) => match self.types.get(t) {
                Ty::Record(r) => !self.types.records[*r as usize].is_abstract,
                _ => false,
            },
            None => true,
        };
        self.diags.truncate(saved);
        r
    }

    fn wrong_operator(&mut self, s: &ast::Ident, msg: String, want: &str) {
        let mut d = Diagnostic::error(s.span, msg);
        if let Some(op) = self.operator_span(s.span) {
            d = d.fix(format!("write `{} {}`", want, s.name), op, want);
        }
        self.emit(d);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_struct(
        &mut self,
        mi: usize,
        ri: u32,
        name: &ast::Ident,
        kind: ast::StructKind,
        params: &[ast::Param],
        heads: (&Option<ast::SuperRef>, &[ast::SuperRef], usize),
        statics: &[ast::StaticVal],
        pdecl: Vec<(String, TyId, Span)>,
    ) {
        let mut impls = Vec::new();
        let mut parent: Option<u32> = None;
        let mut super_args = None;
        let mut super_span = name.span;
        let (extends, supers, colon_extra) = heads;
        let mut extends_struct = false;
        let all = extends
            .iter()
            .map(|e| (e, true, false))
            .chain(supers.iter().enumerate().map(|(i, e)| (e, false, i < colon_extra)));
        for ((s, args), single, listed) in all {
            if listed {
                if extends_struct && !self.struct_or_error(mi, s) {
                    let mut d = Diagnostic::error(s.span, "a struct can extend only one struct").help("interfaces and abstract structs are listed after `::`");
                    if let Some(comma) = self.comma_before(s.span).filter(|_| supers.first().map(|x| x.0.span) == Some(s.span)) {
                        d = d.fix("list them after `::`", comma, " ::");
                    }
                    self.emit(d);
                    extends_struct = false;
                } else if !extends_struct {
                    let _ = self.lookup_type_name(mi, &s.name, s.span).map(|it| {
                        if let Ty::Interface(ii) = self.types.get(it).clone() {
                            impls.push(ii);
                        }
                    });
                    continue;
                }
            }
            let Some(it) = self.lookup_type_name(mi, &s.name, s.span) else {
                let cands: Vec<String> = self.mods[mi].types.keys().cloned().collect();
                let mut d = Diagnostic::error(s.span, format!("unknown struct or interface `{}`", s.name));
                if let Some(c) = suggest(&s.name, cands.iter().map(|x| x.as_str())) {
                    d = d.fix(format!("a type with a similar name exists: `{}`", c), s.span, c);
                }
                self.emit(d);
                continue;
            };
            match self.types.get(it).clone() {
                Ty::Interface(ii) => {
                    if single {
                        self.wrong_operator(s, format!("`{}` is an interface, so it is used with `::`", s.name), "::");
                    }
                    if args.is_some() {
                        self.emit(
                            Diagnostic::error(s.span, format!("interface `{}` has no constructor", s.name))
                                .help(format!("write `:: {}` without arguments", s.name)),
                        );
                    }
                    impls.push(ii);
                }
                Ty::Record(pri) => {
                    let prec = &self.types.records[pri as usize];
                    let (is_class, is_abstract, is_static) = (prec.is_class, prec.is_abstract, prec.is_static);
                    if !is_class {
                        self.emit(
                            Diagnostic::error(s.span, format!("`{}` is a type, not a struct, so it cannot be extended", s.name))
                                .help(format!("declare it with `def struct {}(...)` to make it extendable", s.name)),
                        );
                        continue;
                    }
                    if is_static {
                        self.error(s.span, format!("`{}` is a static struct and has no objects, so it cannot be extended", s.name));
                        continue;
                    }
                    if kind == ast::StructKind::Static {
                        self.error(s.span, "static structs cannot extend other structs");
                        continue;
                    }
                    if single && !is_abstract {
                        extends_struct = true;
                    }
                    if single && is_abstract {
                        self.wrong_operator(s, format!("`{}` is an abstract struct, so it is used with `::`", s.name), "::");
                    } else if !single && !is_abstract {
                        if extends.is_none() && supers.first().map(|x| x.0.span) == Some(s.span) {
                            self.wrong_operator(s, format!("`{}` is a struct, so it is extended with `:`", s.name), ":");
                        } else {
                            self.emit(
                                Diagnostic::error(s.span, format!("`{}` is a struct, so it is extended with `:`", s.name))
                                    .help("write the struct to extend first: `def struct Name : Parent :: Interface`"),
                            );
                        }
                    }
                    if let Some(p) = parent {
                        let pn = self.types.records[p as usize].name.clone();
                        self.emit(
                            Diagnostic::error(
                                s.span,
                                format!("a struct extends at most one struct, and `{}` already extends `{}`", name.name, pn),
                            )
                            .help("share code through interfaces, or extend one struct from the other"),
                        );
                        continue;
                    }
                    parent = Some(pri);
                    super_args = args.clone();
                    super_span = s.span;
                }
                Ty::Error => {}
                _ => {
                    let t = self.show(it);
                    self.error(s.span, format!("`{}` is not a struct or an interface", t));
                }
            }
        }
        if kind == ast::StructKind::Static {
            if let Some(p) = params.first() {
                self.error_note(
                    p.name.span,
                    "static structs have no objects, so they take no constructor parameters",
                    "use `static var` values or a normal struct instead",
                );
            }
            if !impls.is_empty() {
                self.error(name.span, "static structs cannot implement interfaces");
                impls.clear();
            }
        }
        let rec = &mut self.types.records[ri as usize];
        rec.implements = impls;
        rec.parent = parent;
        self.struct_decls.insert(
            ri,
            StructDecl {
                module: mi,
                params: pdecl,
                super_args,
                super_span,
                own_start: 0,
                param_fields: vec![true; params.len()],
                statics: statics.to_vec(),
            },
        );
    }

    pub fn link_structs(&mut self) {
        let n = self.types.records.len();
        let mut state = vec![0u8; n];
        for ri in 0..n as u32 {
            if self.struct_decls.contains_key(&ri) {
                self.link_struct(ri, &mut state);
            }
        }
    }

    fn link_struct(&mut self, ri: u32, state: &mut Vec<u8>) {
        match state[ri as usize] {
            2 => return,
            1 => {
                let rec = &self.types.records[ri as usize];
                let (span, name) = (rec.span, rec.name.clone());
                self.error(span, format!("`{}` extends itself", name));
                self.types.records[ri as usize].parent = None;
                return;
            }
            _ => {}
        }
        state[ri as usize] = 1;
        if let Some(p) = self.types.records[ri as usize].parent {
            self.link_struct(p, state);
            if state[ri as usize] == 1 && self.types.records[ri as usize].parent.is_some() {
                let prec = self.types.records[p as usize].clone();
                let own = self.types.records[ri as usize].fields.clone();
                let nparams = self.struct_decls.get(&ri).map(|d| d.params.len()).unwrap_or(0);
                let mut fields = prec.fields.clone();
                let mut param_fields = vec![true; nparams];
                for (i, f) in own.into_iter().enumerate() {
                    if let Some(pi) = prec.field_index(&f.name) {
                        if i < nparams {
                            param_fields[i] = false;
                            if prec.fields[pi].ty != f.ty {
                                let (a, b) = (self.show(f.ty), self.show(prec.fields[pi].ty));
                                self.error(
                                    f.span,
                                    format!("parameter `{}` has type {} but the field it fills in `{}` has type {}", f.name, a, prec.name, b),
                                );
                            }
                        } else {
                            self.error(f.span, format!("field `{}` is already defined in `{}`", f.name, prec.name));
                        }
                        continue;
                    }
                    fields.push(f);
                }
                let mut implements = prec.implements.clone();
                for i in self.types.records[ri as usize].implements.clone() {
                    if !implements.contains(&i) {
                        implements.push(i);
                    }
                }
                let rec = &mut self.types.records[ri as usize];
                rec.fields = fields;
                rec.implements = implements;
                if let Some(d) = self.struct_decls.get_mut(&ri) {
                    d.own_start = prec.fields.len();
                    d.param_fields = param_fields;
                }
                let pdecl = self.struct_decls.get(&p).cloned();
                let decl = self.struct_decls.get(&ri).cloned();
                if let (Some(pd), Some(d)) = (pdecl, decl) {
                    if d.super_args.is_none() && !pd.params.is_empty() {
                        let names: Vec<String> = pd.params.iter().map(|p| p.0.clone()).collect();
                        self.error_note(
                            d.super_span,
                            format!("`{}` needs constructor arguments", prec.name),
                            format!("write `: {}({})`", prec.name, names.join(", ")),
                        );
                    }
                }
            }
        }
        state[ri as usize] = 2;
    }

    pub fn declare_struct_members(&mut self, mi: usize, ri: u32, name: &ast::Ident, methods: &[(Vis, ast::FunDecl)], statics: &[ast::StaticVal]) {
        let t = self.types.records[ri as usize].ty;
        for (vis, mdecl) in methods {
            let mname = mdecl.name.name.clone();
            let dup = {
                let rec = &self.types.records[ri as usize];
                rec.methods.contains_key(&mname)
                    || rec.statics.contains_key(&mname)
                    || rec.field_index(&mname).is_some()
                    || self.abstract_sigs.contains_key(&(ri, mname.clone()))
                    || (mname == "init" && rec.init.is_some() && !mdecl.is_static)
            };
            if dup {
                self.error(mdecl.name.span, format!("`{}` is already defined in struct `{}`", mname, name.name));
                continue;
            }
            if mdecl.is_abstract {
                let params: Vec<TyId> = mdecl.params.iter().map(|p| self.resolve_type_in(&p.ty, mi)).collect();
                let ret = mdecl.ret.as_ref().map(|r| self.resolve_type_in(r, mi)).unwrap_or(T_VOID);
                self.abstract_sigs.insert(
                    (ri, mname),
                    AbstractSig {
                        params,
                        ret,
                        is_async: mdecl.is_async,
                        span: mdecl.name.span,
                    },
                );
                continue;
            }
            let is_init = mname == "init" && !mdecl.is_static;
            if is_init {
                if let Some(p) = mdecl.params.first() {
                    self.error_note(
                        p.name.span,
                        "`init` takes no parameters",
                        format!(
                            "constructor parameters go after the struct name: `def struct {}({}: ...)`",
                            name.name, p.name.name
                        ),
                    );
                }
            }
            let self_ty = if mdecl.is_static { None } else { Some(t) };
            let fid = self.declare_fun(mi, mdecl, self_ty, *vis == Vis::Priv, true);
            self.funcs[fid as usize].name = format!("{}.{}", name.name, mname);
            self.funcs[fid as usize].is_static = mdecl.is_static;
            let rec = &mut self.types.records[ri as usize];
            if mdecl.is_static {
                rec.statics.insert(mname, fid);
            } else if is_init {
                rec.init = Some(fid);
                self.funcs[fid as usize].ret = Some(T_VOID);
            } else {
                rec.methods.insert(mname, fid);
            }
        }
        for sv in statics {
            let n = sv.name.name.clone();
            let dup = {
                let rec = &self.types.records[ri as usize];
                rec.methods.contains_key(&n) || rec.statics.contains_key(&n) || rec.field_index(&n).is_some() || rec.static_vals.contains_key(&n)
            };
            if dup {
                self.error(sv.name.span, format!("`{}` is already defined in struct `{}`", n, name.name));
                continue;
            }
            let ty = match &sv.ty {
                Some(te) => Some(self.resolve_type_in(te, mi)),
                None => literal_type(&sv.init),
            };
            let g = self.globals.len() as u32;
            self.globals.push(GlobalInfo {
                name: format!("{}.{}", name.name, n),
                ty,
                is_const: sv.is_const,
                module: mi,
                span: sv.name.span,
            });
            if sv.vis == Vis::Priv {
                self.static_private.insert(g);
            }
            self.types.records[ri as usize].static_vals.insert(n, g);
        }
        let rec = &self.types.records[ri as usize];
        if !rec.is_abstract && !rec.is_static {
            let params = self.struct_decls.get(&ri).map(|d| d.params.clone()).unwrap_or_default();
            let fid = self.funcs.len() as FuncId;
            self.funcs.push(FuncInfo {
                name: format!("new {}", name.name),
                params,
                ret: Some(t),
                is_async: false,
                decl: None,
                module: mi,
                self_ty: None,
                state: FnState::Pending,
                hir: None,
                span: name.span,
                private: false,
                is_static: true,
                annotations: Vec::new(),
                deprecated: None,
                external: None,
            });
            self.types.records[ri as usize].ctor = Some(fid);
        }
    }

    fn struct_order(&self) -> Vec<u32> {
        let mut order: Vec<(usize, u32)> = self.struct_decls.keys().map(|ri| (self.types.ancestors(*ri).len(), *ri)).collect();
        order.sort();
        order.into_iter().map(|x| x.1).collect()
    }

    fn method_sig(&mut self, ri: u32, name: &str) -> Option<(Vec<TyId>, TyId, bool, Span)> {
        if let Some(fid) = self.types.records[ri as usize].methods.get(name).copied() {
            let ret = self.func_ret(fid);
            let info = &self.funcs[fid as usize];
            return Some((info.params.iter().skip(1).map(|p| p.1).collect(), ret, info.is_async, info.span));
        }
        self.abstract_sigs
            .get(&(ri, name.to_string()))
            .map(|s| (s.params.clone(), s.ret, s.is_async, s.span))
    }

    pub fn check_structs(&mut self) {
        let order = self.struct_order();
        for &ri in &order {
            let Some(p) = self.types.records[ri as usize].parent else { continue };
            let prec = self.types.records[p as usize].clone();
            let mut names: Vec<String> = prec.methods.keys().cloned().collect();
            names.sort();
            for n in names {
                let own = self.types.records[ri as usize].methods.contains_key(&n) || self.abstract_sigs.contains_key(&(ri, n.clone()));
                if !own {
                    let fid = prec.methods[&n];
                    self.types.records[ri as usize].methods.insert(n, fid);
                }
            }
            let inherited: Vec<(String, AbstractSig)> = self
                .abstract_sigs
                .iter()
                .filter(|((r, _), _)| *r == p)
                .map(|((_, n), s)| (n.clone(), s.clone()))
                .collect();
            for (n, sig) in inherited {
                let own = self.types.records[ri as usize].methods.contains_key(&n) || self.abstract_sigs.contains_key(&(ri, n.clone()));
                if !own {
                    self.abstract_sigs.insert((ri, n), sig);
                }
            }
            for (n, fid) in prec.statics.iter() {
                self.types.records[ri as usize].statics.entry(n.clone()).or_insert(*fid);
            }
            for (n, g) in prec.static_vals.iter() {
                self.types.records[ri as usize].static_vals.entry(n.clone()).or_insert(*g);
            }
        }
        let parents: HashSet<u32> = order.iter().filter_map(|r| self.types.records[*r as usize].parent).collect();
        for &ri in &order {
            if !self.types.records[ri as usize].is_abstract && !parents.contains(&ri) {
                continue;
            }
            let rec = self.types.records[ri as usize].clone();
            let mut names: Vec<(String, u32)> = rec
                .methods
                .iter()
                .map(|(n, f)| (n.clone(), self.funcs[*f as usize].params.len() as u32))
                .chain(
                    self.abstract_sigs
                        .iter()
                        .filter(|((r, _), _)| *r == ri)
                        .map(|((_, n), s)| (n.clone(), s.params.len() as u32 + 1)),
                )
                .collect();
            names.sort();
            for (n, argc) in names {
                let slot = self.slots.len() as u32;
                self.slots.push(IfaceSlot {
                    name: format!("{}.{}", rec.name, n),
                    argc,
                    impls: Vec::new(),
                });
                self.vslots.insert((ri, n), slot);
                self.vslot_set.insert(slot);
            }
        }
        for &ri in &order {
            let rec = self.types.records[ri as usize].clone();
            if !rec.is_abstract && !rec.is_static {
                let mut chain = vec![ri];
                chain.extend(self.types.ancestors(ri));
                for a in chain {
                    let mut names: Vec<(String, u32)> = self.vslots.iter().filter(|((r, _), _)| *r == a).map(|((_, n), s)| (n.clone(), *s)).collect();
                    names.sort();
                    for (n, slot) in names {
                        if let Some(fid) = rec.methods.get(&n) {
                            self.slots[slot as usize].impls.push((rec.ty, *fid));
                        }
                    }
                }
            }
            let Some(p) = rec.parent else { continue };
            let mut own: Vec<(String, FuncId)> = rec
                .methods
                .iter()
                .filter(|(_, f)| self.funcs[**f as usize].self_ty == Some(rec.ty))
                .map(|(n, f)| (n.clone(), *f))
                .collect();
            own.sort();
            for (n, fid) in own {
                let Some((pp, pr, pa, _)) = self.method_sig(p, &n) else { continue };
                let ret = self.func_ret(fid);
                let info = &self.funcs[fid as usize];
                let ps: Vec<TyId> = info.params.iter().skip(1).map(|x| x.1).collect();
                let (fspan, is_async) = (info.span, info.is_async);
                if ps != pp || ret != pr || is_async != pa {
                    let want = self.method_sig_str(&pp, pr, pa);
                    let got = self.method_sig_str(&ps, ret, is_async);
                    let pname = self.types.records[p as usize].name.clone();
                    self.error_detail(
                        fspan,
                        format!("`{}.{}` does not match `{}.{}`", rec.name, n, pname, n),
                        format!("expected `{}` but found `{}`", want, got),
                    );
                }
            }
            if rec.is_abstract {
                continue;
            }
            let mut missing: Vec<(String, AbstractSig)> = self
                .abstract_sigs
                .iter()
                .filter(|((r, _), _)| *r == ri)
                .map(|((_, n), s)| (n.clone(), s.clone()))
                .collect();
            missing.sort_by(|a, b| a.0.cmp(&b.0));
            for (n, sig) in missing {
                let owner = self
                    .types
                    .ancestors(ri)
                    .into_iter()
                    .rev()
                    .find(|a| self.abstract_sigs.contains_key(&(*a, n.clone())))
                    .map(|a| self.types.records[a as usize].name.clone())
                    .unwrap_or_default();
                let want = self.method_sig_str(&sig.params, sig.ret, sig.is_async);
                self.error_note(
                    rec.span,
                    format!("struct `{}` does not implement abstract method `{}.{}`", rec.name, owner, n),
                    format!("add `{}` with a body", want.replacen("fun(", &format!("fun {}(", n), 1)),
                );
            }
        }
    }

    pub fn build_ctors(&mut self) {
        for ri in self.struct_order() {
            if self.types.records[ri as usize].ctor.is_some() {
                self.build_ctor(ri);
            }
        }
    }

    fn build_ctor(&mut self, ri: u32) {
        let rec = self.types.records[ri as usize].clone();
        let fid = rec.ctor.unwrap();
        let t = rec.ty;
        let decl = self.struct_decls[&ri].clone();
        let mut ctx = self.new_ctx(fid, decl.module, false);
        for (_, ty, _) in &decl.params {
            ctx.locals.push(*ty);
        }
        self.fx.push(ctx);
        let mut pre = Vec::new();
        let slots: Vec<u32> = (0..decl.params.len() as u32).collect();
        let vals = self.ctor_values(ri, &slots, &mut pre);
        let obj = self.new_local(t);
        let mut body = pre;
        body.push(Stmt::Expr(Expr::new(
            ExprKind::SetLocal(obj, Box::new(Expr::new(ExprKind::NewStruct(t, vals), t))),
            t,
        )));
        let mut chain = vec![ri];
        chain.extend(self.types.ancestors(ri));
        for c in chain.into_iter().rev() {
            let r = &self.types.records[c as usize];
            if let Some(init) = r.init {
                let ct = r.ty;
                body.push(Stmt::Expr(Expr::new(ExprKind::Call(init, vec![Expr::new(ExprKind::Local(obj), ct)]), T_VOID)));
            }
        }
        body.push(Stmt::Return(Some(Expr::new(ExprKind::Local(obj), t))));
        let ctx = self.fx.pop().unwrap();
        let end_loc = self.loc(rec.span);
        let info = &mut self.funcs[fid as usize];
        info.hir = Some(hir::Func {
            name: info.name.clone(),
            params: info.params.len() as u32,
            locals: ctx.locals,
            ret: t,
            body,
            is_async: false,
            span: info.span,
            end_loc,
            annotations: Vec::new(),
            external: None,
        });
        info.state = FnState::Done;
    }

    fn ctor_values(&mut self, ri: u32, slots: &[u32], pre: &mut Vec<Stmt>) -> Vec<Expr> {
        let rec = self.types.records[ri as usize].clone();
        let decl = self.struct_decls[&ri].clone();
        let mut scope = HashMap::new();
        for ((n, _, sp), s) in decl.params.iter().zip(slots) {
            scope.insert(
                n.clone(),
                LocalSym {
                    slot: *s,
                    is_const: false,
                    span: *sp,
                },
            );
        }
        let saved_scopes = std::mem::replace(&mut self.ctx().scopes, vec![scope]);
        let saved_module = self.ctx().module;
        self.ctx().module = decl.module;
        let mut vals = Vec::new();
        if let Some(p) = rec.parent {
            let pdecl = self.struct_decls[&p].clone();
            let pname = self.types.records[p as usize].name.clone();
            let args = decl.super_args.clone().unwrap_or_default();
            if decl.super_args.is_some() && args.len() != pdecl.params.len() {
                let n = pdecl.params.len();
                self.error(
                    decl.super_span,
                    format!("`{}` expects {} argument{} but got {}", pname, n, if n == 1 { "" } else { "s" }, args.len()),
                );
            }
            let mut pslots = Vec::new();
            for (i, (_, pt, _)) in pdecl.params.iter().enumerate() {
                let v = match args.get(i) {
                    Some(a) => self.expr_to(a, *pt),
                    None => Self::err_expr(),
                };
                let s = self.new_local(*pt);
                pre.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(s, Box::new(v)), *pt)));
                pslots.push(s);
            }
            for a in args.iter().skip(pdecl.params.len()) {
                self.expr(a, None);
            }
            vals.extend(self.ctor_values(p, &pslots, pre));
        }
        for (((_, ty, _), s), is_field) in decl.params.iter().zip(slots).zip(&decl.param_fields) {
            if *is_field {
                vals.push(Expr::new(ExprKind::Local(*s), *ty));
            }
        }
        let start = decl.own_start + decl.param_fields.iter().filter(|x| **x).count();
        for f in rec.fields.iter().skip(start) {
            let v = match &f.default {
                Some(d) => self.expr_to(d, f.ty),
                None => match self.zero(f.ty) {
                    Some(z) => z,
                    None => {
                        let ts = self.show(f.ty);
                        self.error_note(
                            f.span,
                            format!("field `{}` needs a default value", f.name),
                            format!(
                                "write `{} {} = ...`, or make it a constructor parameter: `def struct {}({}: {})`",
                                ts, f.name, rec.name, f.name, ts
                            ),
                        );
                        Self::err_expr()
                    }
                },
            };
            vals.push(v);
        }
        self.ctx().scopes = saved_scopes;
        self.ctx().module = saved_module;
        vals
    }

    pub fn static_inits(&mut self, name: &ast::Ident) -> Vec<Stmt> {
        let m = self.cur_module();
        let t = match self.mods[m].types.get(&name.name) {
            Some(e) if e.span == name.span => e.sym,
            _ => return Vec::new(),
        };
        let ri = match self.types.get(t) {
            Ty::Record(r) => *r,
            _ => return Vec::new(),
        };
        let statics = match self.struct_decls.get(&ri) {
            Some(d) => d.statics.clone(),
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        for sv in &statics {
            let g = match self.types.records[ri as usize].static_vals.get(&sv.name.name) {
                Some(g) if self.globals[*g as usize].span == sv.name.span => *g,
                _ => continue,
            };
            let declared = sv.ty.as_ref().map(|te| self.resolve_type_in(te, m));
            let h = self.expr(&sv.init, declared);
            if h.ty == T_VOID {
                self.error(sv.init.span, "cannot assign the result of a function that returns nothing");
            }
            let target = declared.unwrap_or(if h.ty == T_NULL { T_ANY } else { h.ty });
            let v = self.coerce(h, target, sv.init.span);
            self.globals[g as usize].ty = Some(target);
            let ts = self.show(target);
            self.hover(
                sv.name.span,
                format!("static {} {}.{}: {}", if sv.is_const { "const" } else { "var" }, name.name, sv.name.name, ts),
            );
            out.push(Stmt::Expr(Expr::new(ExprKind::SetGlobal(g, Box::new(v)), target)));
        }
        out
    }

    pub fn static_owner_rec(&self) -> Option<TyId> {
        let c = self.fx.last()?;
        if self.ext_funcs.contains(&c.func) {
            return None;
        }
        c.self_ty.or_else(|| self.static_owner())
    }

    pub fn static_value(&mut self, rec: &RecordDef, name: &str, span: Span) -> Option<(u32, TyId)> {
        let g = *rec.static_vals.get(name)?;
        if self.static_private.contains(&g) {
            let inside = self
                .static_owner_rec()
                .map(|o| o == rec.ty || self.types.implements(o, rec.ty))
                .unwrap_or(false);
            if !inside {
                self.error(span, format!("`{}` is private to struct `{}`", name, rec.name));
            }
        }
        let gi = &self.globals[g as usize];
        let (gspan, gty, gconst) = (gi.span, gi.ty, gi.is_const);
        self.def_link(span, gspan);
        match gty {
            Some(t) => {
                let ts = self.show(t);
                let text = self.with_doc(format!("static {} {}.{}: {}", if gconst { "const" } else { "var" }, rec.name, name, ts), gspan);
                self.hover(span, text);
                Some((g, t))
            }
            None => {
                self.error_note(
                    span,
                    format!("`{}.{}` is used before it is initialized", rec.name, name),
                    "give it a type, e.g. `static var x: int = ...`, or move the use after the struct",
                );
                Some((g, T_ERROR))
            }
        }
    }

    pub fn new_expr(&mut self, ty: &ast::Ident, args: &[ast::Expr], span: Span) -> Expr {
        let m = self.cur_module();
        let t = match self.lookup_type_name(m, &ty.name, ty.span) {
            Some(t) => t,
            None => {
                self.error(ty.span, format!("unknown struct `{}`", ty.name));
                for a in args {
                    self.expr(a, None);
                }
                return Self::err_expr();
            }
        };
        let ri = match self.types.get(t) {
            Ty::Record(r) => *r,
            Ty::Error => return Self::err_expr(),
            _ => {
                let s = self.show(t);
                self.error(ty.span, format!("`{}` is not a struct", s));
                for a in args {
                    self.expr(a, None);
                }
                return Self::err_expr();
            }
        };
        let rec = self.types.records[ri as usize].clone();
        if !rec.is_class {
            return self.construct_record(t, args, span, ty.span);
        }
        let head = format!(
            "{}struct {}",
            if rec.is_abstract {
                "abstract "
            } else if rec.is_static {
                "static "
            } else {
                ""
            },
            rec.name
        );
        let dep = self.deprecated_types.get(&t).cloned();
        let text = self.with_doc_dep(head, rec.span, dep.as_deref());
        self.hover(ty.span, text);
        let fail = |c: &mut Self| {
            for a in args {
                c.expr(a, None);
            }
            Self::err_expr()
        };
        if rec.is_abstract {
            self.error_note(
                ty.span,
                format!("cannot create an object of abstract struct `{}`", rec.name),
                "create an object of a struct that extends it instead",
            );
            return fail(self);
        }
        if rec.is_static {
            self.error_note(
                ty.span,
                format!("`{}` is a static struct and has no objects", rec.name),
                format!("use its members directly, e.g. `{}.name`", rec.name),
            );
            return fail(self);
        }
        match rec.ctor {
            Some(c) => self.direct_call(c, None, args, span, ty.span),
            None => self.construct_record(t, args, span, ty.span),
        }
    }

    pub fn virtual_call(&mut self, ri: u32, slot: u32, name: &ast::Ident, recv: Expr, args: &[ast::Expr], span: Span) -> Expr {
        let rec = self.types.records[ri as usize].clone();
        let (params, ret, is_async, def_span) = self.method_sig(ri, &name.name).unwrap_or((Vec::new(), T_ERROR, false, rec.span));
        if let Some(fid) = rec.methods.get(&name.name).copied() {
            self.warn_deprecated_func(fid, name.span);
            let info = &self.funcs[fid as usize];
            if info.private && info.self_ty != self.static_owner_rec() {
                self.error(name.span, format!("method `{}` is private", name.name));
            }
        }
        if is_async {
            self.error(name.span, "async methods cannot be called through an abstract struct yet");
        }
        self.def_link(name.span, def_span);
        let sig = self.method_sig_str(&params, ret, is_async);
        let text = self.with_doc(format!("{}.{}: {}", rec.name, name.name, sig), def_span);
        self.hover(name.span, text);
        let mut ps = vec![rec.ty];
        ps.extend(params);
        let recv = self.alive_wrap(recv, name.span);
        let hargs = self.check_args(&ps, args, Some(recv), span, &format!("`{}`", name.name));
        self.invalidate_globals();
        Expr::new(ExprKind::CallIface(slot, hargs), ret)
    }

    pub fn struct_method(&mut self, ri: u32, name: &ast::Ident, recv: Expr, args: &[ast::Expr], span: Span) -> Option<Expr> {
        let rec = &self.types.records[ri as usize];
        if let Some(slot) = self.vslots.get(&(ri, name.name.clone())).copied() {
            return Some(self.virtual_call(ri, slot, name, recv, args, span));
        }
        let fid = rec.methods.get(&name.name).copied()?;
        Some(self.direct_call(fid, Some(recv), args, span, name.span))
    }

    pub fn fact_entries(&self) -> Vec<(usize, &Fact)> {
        match self.fx.last() {
            Some(c) => c
                .narrow
                .keys()
                .filter(|k| **k >= FACT_KEY && **k < GLOBAL_KEY)
                .map(|k| ((k - FACT_KEY) as usize, &self.facts[(k - FACT_KEY) as usize]))
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn ext_method(&self, key: u32, name: &str) -> Option<FuncId> {
        let mut best: Option<(usize, FuncId)> = None;
        for (i, f) in self.fact_entries() {
            if let Fact::Ext { var, name: n, fid } = f {
                if *var == key && n == name && best.map(|b| i > b.0).unwrap_or(true) {
                    best = Some((i, *fid));
                }
            }
        }
        best.map(|b| b.1)
    }

    pub fn dead_fact(&self, key: u32) -> Option<Span> {
        if !self.any_dead {
            return None;
        }
        self.fact_entries().into_iter().find_map(|(_, f)| match f {
            Fact::Dead { var, span } if *var == key => Some(*span),
            _ => None,
        })
    }

    pub fn clear_facts(&mut self, key: u32) {
        if self.facts.is_empty() {
            return;
        }
        let facts = &self.facts;
        if let Some(c) = self.fx.last_mut() {
            c.narrow.retain(|k, _| {
                if *k < FACT_KEY || *k >= GLOBAL_KEY {
                    return true;
                }
                match &facts[(*k - FACT_KEY) as usize] {
                    Fact::Ext { var, .. } | Fact::Dead { var, .. } => *var != key,
                }
            });
        }
    }

    pub fn add_fact(&mut self, f: Fact, value: TyId) {
        let i = self.facts.len() as u32;
        self.facts.push(f);
        self.ctx().narrow.insert(FACT_KEY + i, value);
    }

    pub fn check_alive(&mut self, key: u32, name: &str, span: Span) {
        if let Some(d) = self.dead_fact(key) {
            let (l, _) = self.sm.file(d.file).line_col(d.start as usize);
            self.error_note(
                span,
                format!("`{}` was destroyed on line {} and can no longer be used", name, l),
                format!("assign a new object to `{}` before using it again", name),
            );
        }
    }

    pub fn alive_wrap(&mut self, o: Expr, span: Span) -> Expr {
        if !self.has_destroy {
            return o;
        }
        if let ExprKind::Local(0) = o.kind {
            if self.fx.last().and_then(|c| c.self_ty).is_some() {
                return o;
            }
        }
        let t = o.ty;
        let l = self.loc_expr(span);
        Expr::new(ExprKind::Rt(RtFn::Alive, vec![o, l]), t)
    }

    pub fn extend_stmt(&mut self, target: &ast::Ident, func: &ast::FunDecl) -> Vec<Stmt> {
        let texpr = ast::Expr {
            kind: A::Ident(target.name.clone()),
            span: target.span,
        };
        let key = self.local_of(&texpr);
        let h = self.ident_expr(&target.name, target.span);
        if h.ty == T_ERROR {
            return Vec::new();
        }
        let t = h.ty;
        let ri = match self.types.get(t) {
            Ty::Record(r) if self.types.records[*r as usize].is_class && !self.types.records[*r as usize].is_static => *r,
            _ => {
                let s = self.show(t);
                self.error(
                    target.span,
                    format!("functions can only be added to struct objects, but `{}` is {}", target.name, s),
                );
                return Vec::new();
            }
        };
        let Some(key) = key else {
            self.error(target.span, "functions can only be added to objects stored in variables");
            return Vec::new();
        };
        let fname = func.name.name.clone();
        let rec = self.types.records[ri as usize].clone();
        if rec.methods.contains_key(&fname) || self.abstract_sigs.contains_key(&(ri, fname.clone())) || rec.field_index(&fname).is_some() {
            self.error_note(
                func.name.span,
                format!("{} already has a member named `{}`", rec.name, fname),
                "functions added to an object cannot replace its members; pick another name",
            );
            return Vec::new();
        }
        let k = (func.name.span.file, func.name.span.start);
        let fid = match self.lambdas.get(&k) {
            Some(f) => *f,
            None => {
                let m = self.cur_module();
                let fid = self.declare_fun(m, func, Some(t), false, true);
                self.funcs[fid as usize].name = format!("{}.{}", target.name, fname);
                self.lambdas.insert(k, fid);
                self.ext_funcs.insert(fid);
                fid
            }
        };
        self.add_fact(Fact::Ext { var: key, name: fname, fid }, fid);
        Vec::new()
    }

    pub fn destroy_stmt(&mut self, target: &ast::Ident, args: &[ast::Expr], span: Span) -> Vec<Stmt> {
        let texpr = ast::Expr {
            kind: A::Ident(target.name.clone()),
            span: target.span,
        };
        let key = self.local_of(&texpr);
        let h = self.ident_expr(&target.name, target.span);
        let fail = |c: &mut Self| {
            for a in args {
                c.expr(a, None);
            }
            Vec::new()
        };
        if h.ty == T_ERROR {
            return fail(self);
        }
        let t = h.ty;
        let ri = match self.types.get(t).clone() {
            Ty::Record(r) if self.types.records[r as usize].is_class && !self.types.records[r as usize].is_static => r,
            Ty::Optional(_) => {
                self.error_note(target.span, format!("`{}` may be null", target.name), "check it with `if (x != null)` first");
                return fail(self);
            }
            _ => {
                let s = self.show(t);
                self.error(target.span, format!("only struct objects can be destroyed, but `{}` is {}", target.name, s));
                return fail(self);
            }
        };
        let Some(key) = key else {
            self.error(target.span, "only objects stored in variables can be destroyed");
            return fail(self);
        };
        let mut out = Vec::new();
        let dname = ast::Ident {
            name: "destroy".into(),
            span: target.span,
        };
        let has_dtor = self.types.records[ri as usize].methods.contains_key("destroy") || self.vslots.contains_key(&(ri, "destroy".to_string()));
        if has_dtor {
            if let Some(call) = self.struct_method(ri, &dname, h.clone(), args, span) {
                out.push(Stmt::Expr(call));
            }
        } else if !args.is_empty() {
            let rn = self.types.records[ri as usize].name.clone();
            self.error_note(
                span,
                format!("`{}` has no `destroy` function, so `destroy {}()` takes no arguments", rn, target.name),
                format!("add `fun destroy(...)` to `{}` to run code when its objects are destroyed", rn),
            );
            fail(self);
        }
        let l = self.loc_expr(span);
        out.push(Stmt::Expr(Expr::new(ExprKind::Rt(RtFn::Destroy, vec![h, l]), T_VOID)));
        self.any_dead = true;
        self.add_fact(Fact::Dead { var: key, span }, 0);
        out
    }
}

fn devirt_stmts(stmts: &mut [Stmt], map: &HashMap<u32, FuncId>, ctors: &HashMap<FuncId, (TyId, Vec<Expr>)>) {
    for s in stmts {
        match s {
            Stmt::Expr(e) => devirt_expr(e, map, ctors),
            Stmt::If(c, a, b) => {
                devirt_expr(c, map, ctors);
                devirt_stmts(a, map, ctors);
                devirt_stmts(b, map, ctors);
            }
            Stmt::Loop { cond, body, step } => {
                if let Some(c) = cond {
                    devirt_expr(c, map, ctors);
                }
                devirt_stmts(body, map, ctors);
                devirt_stmts(step, map, ctors);
            }
            Stmt::Return(Some(e)) => devirt_expr(e, map, ctors),
            _ => {}
        }
    }
}

fn devirt_expr(e: &mut Expr, map: &HashMap<u32, FuncId>, ctors: &HashMap<FuncId, (TyId, Vec<Expr>)>) {
    match &mut e.kind {
        ExprKind::SetLocal(_, x)
        | ExprKind::SetGlobal(_, x)
        | ExprKind::Unary(_, x)
        | ExprKind::Conv(_, x)
        | ExprKind::GetField(x, _)
        | ExprKind::ArrLen(x)
        | ExprKind::BoxVal(x) => devirt_expr(x, map, ctors),
        ExprKind::Binary(_, a, b) | ExprKind::And(a, b) | ExprKind::Or(a, b) | ExprKind::SetField(a, _, b) | ExprKind::Index(a, b, _) => {
            devirt_expr(a, map, ctors);
            devirt_expr(b, map, ctors);
        }
        ExprKind::SetIndex(a, b, c, _) => {
            devirt_expr(a, map, ctors);
            devirt_expr(b, map, ctors);
            devirt_expr(c, map, ctors);
        }
        ExprKind::Call(f, xs) => {
            xs.iter_mut().for_each(|x| devirt_expr(x, map, ctors));
            if let Some((t, vals)) = ctors.get(f) {
                let mut args = std::mem::take(xs).into_iter();
                let fields = vals
                    .iter()
                    .map(|v| match v.kind {
                        ExprKind::Local(_) => args.next().unwrap(),
                        _ => v.clone(),
                    })
                    .collect();
                e.kind = ExprKind::NewStruct(*t, fields);
            }
        }
        ExprKind::Rt(_, xs) | ExprKind::Spawn(_, xs) | ExprKind::NewStruct(_, xs) | ExprKind::NewArray(_, xs) => {
            xs.iter_mut().for_each(|x| devirt_expr(x, map, ctors))
        }
        ExprKind::CallIndirect(f, xs) => {
            devirt_expr(f, map, ctors);
            xs.iter_mut().for_each(|x| devirt_expr(x, map, ctors));
        }
        ExprKind::CallIface(slot, xs) => {
            xs.iter_mut().for_each(|x| devirt_expr(x, map, ctors));
            if let Some(f) = map.get(slot) {
                let args = std::mem::take(xs);
                e.kind = ExprKind::Call(*f, args);
            }
        }
        ExprKind::Seq(ss, x) => {
            devirt_stmts(ss, map, ctors);
            devirt_expr(x, map, ctors);
        }
        _ => {}
    }
}

fn simple_ctor(h: &hir::Func) -> Option<Vec<Expr>> {
    let [Stmt::Expr(set), Stmt::Return(Some(ret))] = h.body.as_slice() else {
        return None;
    };
    let (ExprKind::SetLocal(obj, v), ExprKind::Local(r)) = (&set.kind, &ret.kind) else {
        return None;
    };
    if obj != r {
        return None;
    }
    let ExprKind::NewStruct(_, vals) = &v.kind else { return None };
    let mut next = 0;
    for v in vals {
        match &v.kind {
            ExprKind::Local(k) if *k == next => next += 1,
            ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) | ExprKind::Str(_) | ExprKind::Null | ExprKind::TypeId(_) => {}
            ExprKind::NewArray(_, items) if items.is_empty() => {}
            _ => return None,
        }
    }
    if next != h.params {
        return None;
    }
    Some(vals.clone())
}

impl<'a> Checker<'a> {
    pub fn devirtualize(&mut self) {
        let mut map = HashMap::new();
        for slot in &self.vslot_set {
            let impls = &self.slots[*slot as usize].impls;
            if let Some((_, f)) = impls.first() {
                if impls.iter().all(|(_, g)| g == f) && !self.funcs[*f as usize].is_async {
                    map.insert(*slot, *f);
                }
            }
        }
        let mut ctors = HashMap::new();
        for r in &self.types.records {
            let Some(c) = r.ctor else { continue };
            let Some(h) = &self.funcs[c as usize].hir else { continue };
            if let Some(vals) = simple_ctor(h) {
                ctors.insert(c, (r.ty, vals));
            }
        }
        if map.is_empty() && ctors.is_empty() {
            return;
        }
        for f in &mut self.funcs {
            if let Some(h) = &mut f.hir {
                devirt_stmts(&mut h.body, &map, &ctors);
            }
        }
    }
}

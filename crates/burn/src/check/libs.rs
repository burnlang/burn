use super::*;
use burn_runtime::meta::Desc;

pub fn library_module(bytes: &[u8]) -> Result<bvm::Module, String> {
    if bvm::archive::is_archive(bytes) {
        let a = bvm::archive::Archive::decode(bytes)?;
        return a.link_with(&bvm::LinkOptions {
            allow_unresolved: true,
            skip_mixins: false,
        });
    }
    bvm::parse(bytes)
}

pub const REF_COUNTED: &str = "RefCounted";

pub fn ref_counted(m: &bvm::Module) -> bool {
    m.annotations.iter().any(|a| a.target == bvm::Target::Module && a.name == REF_COUNTED)
}

pub fn library_modules(bytes: &[u8]) -> Result<Vec<bvm::Module>, String> {
    if bvm::archive::is_archive(bytes) {
        let a = bvm::archive::Archive::decode(bytes)?;
        return Ok(a.ordered());
    }
    Ok(vec![bvm::parse(bytes)?])
}

struct LibTypes<'m> {
    m: &'m bvm::Module,
    map: HashMap<u32, TyId>,
    module: usize,
    span: Span,
}

impl<'a> Checker<'a> {
    pub fn import_libraries(&mut self, loaded: &Loaded) {
        for (mi, m) in loaded.modules.iter().enumerate() {
            for (li, span) in &m.libs {
                let lib = &loaded.libs[*li];
                let idx = match self.libs.iter().position(|l| l.path == lib.path) {
                    Some(i) => i,
                    None => {
                        self.libs.push(hir::Library {
                            name: lib.name.clone(),
                            path: lib.path.clone(),
                            bytes: lib.bytes.clone(),
                        });
                        self.libs.len() - 1
                    }
                };
                if let Ok(mods) = library_modules(&lib.bytes) {
                    if !mods.iter().all(ref_counted) {
                        self.emit(
                            Diagnostic::error(*span, format!("`{}` was compiled by an older version of Burn", lib.path.display()))
                                .help("compile it again with this version: Burn now frees memory by itself, and libraries have to follow the same rules"),
                        );
                        continue;
                    }
                }
                match library_module(&lib.bytes) {
                    Ok(module) => self.import_library(mi, idx as u32, &module, *span),
                    Err(e) => self.error(*span, format!("cannot load `{}`: {}", lib.path.display(), e)),
                }
            }
        }
    }

    fn import_library(&mut self, mi: usize, lib: u32, m: &bvm::Module, span: Span) {
        let mut lt = LibTypes {
            m,
            map: HashMap::new(),
            module: mi,
            span,
        };
        let mut count = 0;
        for (i, f) in m.funcs.iter().enumerate() {
            if f.external {
                continue;
            }
            let Some(name) = bvm::link::export_name(m, i as u32) else { continue };
            let Some(sig) = &f.sig else {
                self.warn(
                    span,
                    format!("`{}` in the library has no type signature, so it cannot be called from Burn", name),
                );
                continue;
            };
            let mut params = Vec::new();
            let mut ok = true;
            for (k, t) in sig.params.iter().enumerate() {
                match self.lib_type(&mut lt, *t) {
                    Some(ty) => {
                        let pname = f.names.get(k).cloned().unwrap_or_else(|| format!("arg{}", k));
                        params.push((pname, ty, span));
                    }
                    None => ok = false,
                }
            }
            let ret = self.lib_type(&mut lt, sig.ret);
            let (true, Some(ret)) = (ok, ret) else {
                self.warn(
                    span,
                    format!("`{}` in the library uses a type Burn cannot represent, so it is not imported", name),
                );
                continue;
            };
            let fid = self.funcs.len() as FuncId;
            let external = Some(hir::External::Lib { lib, name: name.clone() });
            let hir = hir::Func {
                name: name.clone(),
                params: params.len() as u32,
                locals: params.iter().map(|p| p.1).collect(),
                ret,
                body: Vec::new(),
                is_async: false,
                span,
                end_loc: 0,
                annotations: Vec::new(),
                external: external.clone(),
            };
            self.funcs.push(FuncInfo {
                name: name.clone(),
                params,
                ret: Some(ret),
                is_async: false,
                decl: None,
                module: mi,
                self_ty: None,
                state: FnState::Done,
                hir: Some(hir),
                span,
                private: true,
                is_static: true,
                annotations: Vec::new(),
                deprecated: None,
                external,
                tenv: None,
            });
            if let Some(prev) = self.mods[mi].values.get(&name) {
                let ps = prev.span;
                let (l, _) = self.sm.file(ps.file).line_col(ps.start as usize);
                self.error(span, format!("the library defines `{}`, which is already defined on line {}", name, l));
                continue;
            }
            self.mods[mi].values.insert(
                name,
                Entry {
                    sym: ValSym::Func(fid),
                    vis: Vis::Priv,
                    span,
                },
            );
            count += 1;
        }
        if count == 0 {
            self.warn(span, "the library exports no functions (mark them with @Export and give them a signature)");
        }
    }

    fn lib_type(&mut self, lt: &mut LibTypes, t: u32) -> Option<TyId> {
        if let Some(x) = lt.map.get(&t) {
            return Some(*x);
        }
        let d = lt.m.types.get(t as usize)?.clone();
        let r = match d {
            Desc::Error => T_ERROR,
            Desc::Void => T_VOID,
            Desc::Null => T_NULL,
            Desc::Int => T_INT,
            Desc::Float => T_FLOAT,
            Desc::Bool => T_BOOL,
            Desc::Str => T_STR,
            Desc::Any => T_ANY,
            Desc::Num(n) => crate::types::num_ty(n),
            Desc::Func => return None,
            Desc::Array(e) => {
                let e = self.lib_type(lt, e)?;
                self.types.array(e)
            }
            Desc::Map(k, v) => {
                let k = self.lib_type(lt, k)?;
                let v = self.lib_type(lt, v)?;
                self.types.map_of(k, v)
            }
            Desc::Optional(x) => {
                let x = self.lib_type(lt, x)?;
                self.types.optional(x)
            }
            Desc::Future(x) => {
                let x = self.lib_type(lt, x)?;
                self.types.future(x)
            }
            Desc::Enum { name, variants } => {
                if let Some(e) = self.mods[lt.module].types.get(&name) {
                    e.sym
                } else {
                    let ei = self.types.new_enum(EnumDef {
                        name: name.clone(),
                        variants: variants.iter().map(|v| (v.clone(), lt.span)).collect(),
                        module: lt.module as u32,
                        span: lt.span,
                        ty: 0,
                    });
                    let ty = self.types.enums[ei as usize].ty;
                    self.mods[lt.module].types.insert(
                        name,
                        Entry {
                            sym: ty,
                            vis: Vis::Priv,
                            span: lt.span,
                        },
                    );
                    ty
                }
            }
            Desc::Interface { name } => {
                if let Some(e) = self.mods[lt.module].types.get(&name) {
                    e.sym
                } else {
                    let ii = self.types.new_iface(IfaceDef {
                        name: name.clone(),
                        methods: Vec::new(),
                        module: lt.module as u32,
                        span: lt.span,
                        ty: 0,
                    });
                    let ty = self.types.ifaces[ii as usize].ty;
                    self.mods[lt.module].types.insert(
                        name,
                        Entry {
                            sym: ty,
                            vis: Vis::Priv,
                            span: lt.span,
                        },
                    );
                    ty
                }
            }
            Desc::Record { name, fields, class, .. } => {
                if !name.is_empty() {
                    if let Some(e) = self.mods[lt.module].types.get(&name) {
                        let existing = e.sym;
                        let same = self
                            .types
                            .record_of(existing)
                            .map(|r| r.fields.len() == fields.len() && r.fields.iter().zip(&fields).all(|(a, b)| a.name == b.0))
                            .unwrap_or(false);
                        if same {
                            lt.map.insert(t, existing);
                            return Some(existing);
                        }
                        return None;
                    }
                }
                let ri = self.types.new_record(RecordDef {
                    name: name.clone(),
                    fields: Vec::new(),
                    is_class: class,
                    anon: name.is_empty(),
                    module: lt.module as u32,
                    span: lt.span,
                    ..Default::default()
                });
                let ty = self.types.records[ri as usize].ty;
                lt.map.insert(t, ty);
                if !name.is_empty() {
                    self.mods[lt.module].types.insert(
                        name,
                        Entry {
                            sym: ty,
                            vis: Vis::Priv,
                            span: lt.span,
                        },
                    );
                }
                let mut out = Vec::new();
                for (fname, ft) in fields {
                    let ft = self.lib_type(lt, ft)?;
                    out.push(FieldDef {
                        name: fname,
                        ty: ft,
                        default: None,
                        private: false,
                        span: lt.span,
                    });
                }
                self.types.records[ri as usize].fields = out;
                ty
            }
        };
        lt.map.insert(t, r);
        Some(r)
    }
}

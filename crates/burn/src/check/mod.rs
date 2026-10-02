pub mod annot;
pub mod builtins;
pub mod closures;
pub mod expr;
pub mod init_order;
pub mod libs;
pub mod matching;
pub mod nullsafe;
pub mod stmt;
pub mod structs;

use crate::ast::{self, Def, ItemKind, TypeExpr, TypeExprKind, Vis};
use crate::diag::{Diagnostic, Severity};
use crate::hir::{self, Expr, ExprKind, FuncId, IfaceSlot, Stmt};
use crate::loader::Loaded;
use crate::source::{FileId, SourceMap, Span};
use crate::types::*;
use burn_runtime::RtFn;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub const GLOBAL_KEY: u32 = 1 << 31;
pub const FACT_KEY: u32 = 1 << 30;

#[derive(Clone, Debug)]
pub enum Fact {
    Ext { var: u32, name: String, fid: FuncId },
    Dead { var: u32, span: Span },
}

#[derive(Clone, Debug)]
pub struct StructDecl {
    pub module: usize,
    pub params: Vec<(String, TyId, Span)>,
    pub super_args: Option<Vec<ast::Expr>>,
    pub super_span: Span,
    pub own_start: usize,
    pub param_fields: Vec<bool>,
    pub statics: Vec<ast::StaticVal>,
}

#[derive(Clone, Debug)]
pub struct AbstractSig {
    pub params: Vec<TyId>,
    pub ret: TyId,
    pub is_async: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValSym {
    Func(FuncId),
    Global(u32),
}

#[derive(Clone, Debug)]
pub struct Entry<T> {
    pub sym: T,
    pub vis: Vis,
    pub span: Span,
}

pub struct ModScope {
    pub file: FileId,
    pub values: HashMap<String, Entry<ValSym>>,
    pub types: HashMap<String, Entry<TyId>>,
    pub imports: Vec<usize>,
    pub init: FuncId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FnState {
    Pending,
    InProgress,
    Done,
}

pub struct FuncInfo {
    pub name: String,
    pub params: Vec<(String, TyId, Span)>,
    pub ret: Option<TyId>,
    pub is_async: bool,
    pub decl: Option<Rc<ast::FunDecl>>,
    pub module: usize,
    pub self_ty: Option<TyId>,
    pub state: FnState,
    pub hir: Option<hir::Func>,
    pub span: Span,
    pub private: bool,
    pub is_static: bool,
    pub annotations: Vec<hir::Annotation>,
    pub deprecated: Option<String>,
    pub external: Option<hir::External>,
}

pub struct GlobalInfo {
    pub name: String,
    pub ty: Option<TyId>,
    pub is_const: bool,
    pub module: usize,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct LocalSym {
    pub slot: u32,
    pub is_const: bool,
    pub span: Span,
}

pub struct FnCtx {
    pub func: FuncId,
    pub module: usize,
    pub locals: Vec<TyId>,
    pub scopes: Vec<HashMap<String, LocalSym>>,
    pub narrow: HashMap<u32, TyId>,
    pub ret: Option<TyId>,
    pub returns: Vec<TyId>,
    pub loops: u32,
    pub self_ty: Option<TyId>,
    pub is_init: bool,
    pub dry: bool,
    pub lambda: bool,
    pub cell_names: HashSet<String>,
    pub cells: HashMap<u32, TyId>,
    pub captures: Vec<(u32, u32, bool)>,
}

#[derive(Clone, Debug)]
pub struct LocalInfo {
    pub name: String,
    pub ty: TyId,
    pub decl: Span,
    pub scope: Span,
}

#[derive(Default)]
pub struct Index {
    pub hovers: Vec<(Span, String)>,
    pub defs: Vec<(Span, Span)>,
    pub locals: Vec<LocalInfo>,
    pub expr_types: Vec<(Span, TyId)>,
}

pub struct CheckResult {
    pub program: Option<hir::Program>,
    pub diags: Vec<Diagnostic>,
    pub index: Index,
    pub types: Types,
    pub globals: Vec<(String, TyId, usize)>,
    pub funcs: Vec<(String, String, Span, usize)>,
    pub type_names: Vec<(String, TyId, Span, usize)>,
}

#[derive(Default, Clone)]
pub struct CheckOptions {
    pub skip_before: Option<(FileId, u32)>,
    pub want_index: bool,
    pub repl_echo: bool,
}

pub struct Checker<'a> {
    pub sm: &'a SourceMap,
    pub types: Types,
    pub diags: Vec<Diagnostic>,
    pub mods: Vec<ModScope>,
    pub funcs: Vec<FuncInfo>,
    pub globals: Vec<GlobalInfo>,
    pub strings: Vec<String>,
    string_ids: HashMap<String, u32>,
    pub locs: Vec<String>,
    loc_ids: HashMap<(u32, u32), u32>,
    pub slots: Vec<IfaceSlot>,
    pub fx: Vec<FnCtx>,
    lambdas: HashMap<(u32, u32), FuncId>,
    anon: HashMap<Vec<(String, TyId)>, TyId>,
    pub index: Index,
    pub opts: CheckOptions,
    iface_slot_base: HashMap<u32, u32>,
    pub annotation_types: HashSet<TyId>,
    pub type_anns: HashMap<TyId, Vec<hir::Annotation>>,
    pub deprecated_types: HashMap<TyId, String>,
    pub libs: Vec<hir::Library>,
    pub struct_decls: HashMap<u32, StructDecl>,
    pub abstract_sigs: HashMap<(u32, String), AbstractSig>,
    pub vslots: HashMap<(u32, String), u32>,
    pub vslot_set: HashSet<u32>,
    pub facts: Vec<Fact>,
    pub any_dead: bool,
    pub has_destroy: bool,
    pub ext_funcs: HashSet<FuncId>,
    pub static_private: HashSet<u32>,
    pub init_spans: Vec<(usize, usize, usize, Span)>,
    pub closures: HashMap<FuncId, closures::ClosureInfo>,
    pub adapters: HashMap<FuncId, FuncId>,
    pub cell_types: HashMap<TyId, TyId>,
}

pub fn check(loaded: &Loaded, opts: CheckOptions) -> CheckResult {
    let mut c = Checker {
        sm: &loaded.sm,
        types: Types::new(),
        diags: Vec::new(),
        mods: Vec::new(),
        funcs: Vec::new(),
        globals: Vec::new(),
        strings: Vec::new(),
        string_ids: HashMap::new(),
        locs: Vec::new(),
        loc_ids: HashMap::new(),
        slots: Vec::new(),
        fx: Vec::new(),
        lambdas: HashMap::new(),
        anon: HashMap::new(),
        index: Index::default(),
        opts,
        iface_slot_base: HashMap::new(),
        annotation_types: HashSet::new(),
        type_anns: HashMap::new(),
        deprecated_types: HashMap::new(),
        libs: Vec::new(),
        struct_decls: HashMap::new(),
        abstract_sigs: HashMap::new(),
        vslots: HashMap::new(),
        vslot_set: HashSet::new(),
        facts: Vec::new(),
        any_dead: false,
        has_destroy: loaded.modules.iter().any(|m| m.ast.has_destroy),
        ext_funcs: HashSet::new(),
        static_private: HashSet::new(),
        init_spans: Vec::new(),
        closures: HashMap::new(),
        adapters: HashMap::new(),
        cell_types: HashMap::new(),
    };
    c.run(loaded);
    let has_errors = loaded.diags.iter().chain(c.diags.iter()).any(|d| d.severity == Severity::Error);
    let mut program = None;
    if !has_errors {
        program = Some(c.build_program(loaded));
    }
    let globals = c.globals.iter().map(|g| (g.name.clone(), g.ty.unwrap_or(T_ERROR), g.module)).collect();
    let funcs = c
        .funcs
        .iter()
        .map(|f| {
            let ps: Vec<String> = f
                .params
                .iter()
                .filter(|p| p.0 != "self")
                .map(|p| format!("{}: {}", p.0, c.types.display(p.1)))
                .collect();
            let ret = f
                .ret
                .map(|r| if r == T_VOID { String::new() } else { format!(": {}", c.types.display(r)) })
                .unwrap_or_default();
            let sig = format!("{}fun {}({}){}", if f.is_async { "async " } else { "" }, f.name, ps.join(", "), ret);
            (f.name.clone(), sig, f.span, f.module)
        })
        .collect();
    let mut type_names = Vec::new();
    for (mi, m) in c.mods.iter().enumerate() {
        for (n, e) in &m.types {
            type_names.push((n.clone(), e.sym, e.span, mi));
        }
    }
    let mut diags = c.diags;
    diags.sort_by_key(|d| (d.span.file, d.span.start));
    CheckResult {
        program,
        diags,
        index: c.index,
        types: c.types,
        globals,
        funcs,
        type_names,
    }
}

pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut rows: Vec<Vec<usize>> = vec![(0..=b.len()).collect()];
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = if a[i - 1].eq_ignore_ascii_case(&b[j - 1]) { 0 } else { 1 };
            cur[j] = (rows[i - 1][j] + 1).min(cur[j - 1] + 1).min(rows[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1].eq_ignore_ascii_case(&b[j - 2]) && a[i - 2].eq_ignore_ascii_case(&b[j - 1]) {
                cur[j] = cur[j].min(rows[i - 2][j - 2] + 1);
            }
        }
        rows.push(cur);
    }
    rows[a.len()][b.len()]
}

pub fn suggest<'b>(name: &str, candidates: impl Iterator<Item = &'b str>) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        if c == name || c.starts_with("__") {
            continue;
        }
        let d = edit_distance(name, c);
        let limit = (name.len() / 3).max(1);
        if d <= limit && best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c.to_string())
}

impl<'a> Checker<'a> {
    pub fn is_dry(&self) -> bool {
        self.fx.last().map(|f| f.dry).unwrap_or(false)
    }

    pub fn error(&mut self, span: Span, msg: impl Into<String>) {
        if !self.is_dry() {
            self.diags.push(Diagnostic::error(span, msg));
        }
    }

    pub fn error_note(&mut self, span: Span, msg: impl Into<String>, note: impl Into<String>) {
        if !self.is_dry() {
            self.diags.push(Diagnostic::error(span, msg).help(note));
        }
    }

    pub fn error_detail(&mut self, span: Span, msg: impl Into<String>, note: impl Into<String>) {
        if !self.is_dry() {
            self.diags.push(Diagnostic::error(span, msg).note(note));
        }
    }

    pub fn wrap_suffix(&self, span: Span, suffix: &str) -> String {
        let text = self.src_text(span);
        if text
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '(' | ')' | '[' | ']' | '"'))
        {
            format!("{}{}", text, suffix)
        } else {
            format!("({}){}", text, suffix)
        }
    }

    pub fn params_close(&self, name: Span) -> Option<Span> {
        if (name.file as usize) >= self.sm.files.len() {
            return None;
        }
        let src = self.sm.file(name.file).src.as_bytes();
        let mut i = name.end as usize;
        while i < src.len() && src[i] == b' ' {
            i += 1;
        }
        if src.get(i) != Some(&b'(') {
            return None;
        }
        let mut depth = 0;
        while i < src.len() {
            match src[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Span::new(name.file, i + 1, i + 1));
                    }
                }
                b'\n' | b'{' => return None,
                _ => {}
            }
            i += 1;
        }
        None
    }

    pub fn keyword_before(&self, decl: Span, word: &str) -> Option<Span> {
        if (decl.file as usize) >= self.sm.files.len() {
            return None;
        }
        let f = self.sm.file(decl.file);
        let (line, _) = f.line_col(decl.start as usize);
        let start = f.line_start(line);
        let before = &f.src[start..decl.start as usize];
        let i = before.rfind(word)?;
        let ok_left = i == 0 || !before.as_bytes()[i - 1].is_ascii_alphanumeric();
        let ok_right = before.as_bytes().get(i + word.len()).map(|c| !c.is_ascii_alphanumeric()).unwrap_or(true);
        (ok_left && ok_right).then(|| Span::new(decl.file, start + i, start + i + word.len()))
    }

    pub fn error_fix(&mut self, span: Span, msg: impl Into<String>, replacement: &str) {
        let d = Diagnostic::error(span, msg).fix(format!("did you mean `{}`?", replacement), span, replacement);
        self.emit(d);
    }

    pub fn emit(&mut self, d: Diagnostic) {
        if !self.is_dry() {
            self.diags.push(d);
        }
    }

    pub fn src_text(&self, span: Span) -> String {
        if (span.file as usize) < self.sm.files.len() {
            self.sm.file(span.file).text(span).to_string()
        } else {
            String::new()
        }
    }

    pub fn warn(&mut self, span: Span, msg: impl Into<String>) {
        if !self.is_dry() {
            self.diags.push(Diagnostic::warning(span, msg));
        }
    }

    pub fn with_doc(&self, text: String, decl: Span) -> String {
        self.with_doc_dep(text, decl, None)
    }

    pub fn with_doc_dep(&self, text: String, decl: Span, deprecated: Option<&str>) -> String {
        if !self.opts.want_index || (decl.file as usize) >= self.sm.files.len() {
            return text;
        }
        let src = &self.sm.file(decl.file).src;
        let mut doc = crate::doc::comment::doc_before(src, decl.start as usize)
            .map(|raw| crate::doc::comment::parse(&raw))
            .unwrap_or_default();
        if doc.deprecated.is_none() {
            doc.deprecated = deprecated.map(|d| d.to_string());
        }
        let md = crate::doc::comment::to_markdown(&doc);
        if md.is_empty() {
            text
        } else {
            format!("{}\u{1}{}", text, md)
        }
    }

    pub fn hover(&mut self, span: Span, text: String) {
        if self.opts.want_index && !self.is_dry() {
            self.index.hovers.push((span, text));
        }
    }

    pub fn def_link(&mut self, use_span: Span, def: Span) {
        if self.opts.want_index && !self.is_dry() {
            self.index.defs.push((use_span, def));
        }
    }

    pub fn note_type(&mut self, span: Span, ty: TyId) {
        if self.opts.want_index && !self.is_dry() {
            self.index.expr_types.push((span, ty));
        }
    }

    pub fn show(&self, t: TyId) -> String {
        self.types.display(t)
    }

    pub fn string_id(&mut self, s: &str) -> u32 {
        if let Some(i) = self.string_ids.get(s) {
            return *i;
        }
        let i = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.string_ids.insert(s.to_string(), i);
        i
    }

    pub fn str_lit(&mut self, s: &str) -> Expr {
        let id = self.string_id(s);
        Expr::new(ExprKind::Str(id), T_STR)
    }

    pub fn loc(&mut self, span: Span) -> u32 {
        let key = (span.file, span.start);
        if let Some(i) = self.loc_ids.get(&key) {
            return *i;
        }
        let s = if (span.file as usize) < self.sm.files.len() {
            self.sm.location(span)
        } else {
            "<unknown>".into()
        };
        let i = self.locs.len() as u32;
        self.locs.push(s);
        self.loc_ids.insert(key, i);
        i
    }

    pub fn loc_expr(&mut self, span: Span) -> Expr {
        let l = self.loc(span);
        Expr::new(ExprKind::LocId(l), T_INT)
    }

    pub fn tid(t: TyId) -> Expr {
        Expr::new(ExprKind::TypeId(t), T_INT)
    }

    pub fn ctx(&mut self) -> &mut FnCtx {
        self.fx.last_mut().expect("no function context")
    }

    pub fn cur_module(&self) -> usize {
        self.fx.last().map(|f| f.module).unwrap_or(0)
    }

    pub fn new_local(&mut self, ty: TyId) -> u32 {
        let c = self.ctx();
        c.locals.push(ty);
        (c.locals.len() - 1) as u32
    }

    pub fn declare_local(&mut self, name: &str, ty: TyId, span: Span, is_const: bool, scope_span: Span) -> u32 {
        let slot = self.new_local(ty);
        let c = self.ctx();
        c.scopes.last_mut().unwrap().insert(name.to_string(), LocalSym { slot, is_const, span });
        c.narrow.remove(&slot);
        if self.opts.want_index && !self.is_dry() {
            self.index.locals.push(LocalInfo {
                name: name.to_string(),
                ty,
                decl: span,
                scope: scope_span,
            });
        }
        slot
    }

    pub fn lookup_local(&mut self, name: &str) -> Option<LocalSym> {
        if let Some(l) = self.peek_local(name) {
            return Some(l);
        }
        self.capture(name)
    }

    pub fn enclosing_self(&self) -> Option<TyId> {
        for c in self.fx.iter().rev() {
            if let Some(t) = c.self_ty {
                return Some(t);
            }
            if !c.lambda {
                return None;
            }
        }
        None
    }

    pub fn self_expr(&mut self, st: TyId) -> Expr {
        if self.fx.last().and_then(|c| c.self_ty).is_some() {
            return Expr::new(ExprKind::Local(0), st);
        }
        match self.lookup_local("self") {
            Some(l) => Expr::new(ExprKind::Local(l.slot), st),
            None => Expr::new(ExprKind::Local(0), st),
        }
    }

    pub fn peek_local(&self, name: &str) -> Option<LocalSym> {
        let c = self.fx.last()?;
        for s in c.scopes.iter().rev() {
            if let Some(l) = s.get(name) {
                return Some(l.clone());
            }
        }
        None
    }

    pub fn visible<T: Clone>(&self, module: usize, name: &str, get: impl Fn(&ModScope) -> &HashMap<String, Entry<T>>) -> Result<Option<Entry<T>>, Vec<String>> {
        if let Some(e) = get(&self.mods[module]).get(name) {
            return Ok(Some(e.clone()));
        }
        let mut found: Vec<(usize, Entry<T>)> = Vec::new();
        for imp in &self.mods[module].imports {
            if let Some(e) = get(&self.mods[*imp]).get(name) {
                if e.vis != Vis::Priv {
                    found.push((*imp, e.clone()));
                }
            }
        }
        match found.len() {
            0 => Ok(None),
            1 => Ok(Some(found.pop().unwrap().1)),
            _ => Err(found.iter().map(|(m, _)| self.sm.file(self.mods[*m].file).name.clone()).collect()),
        }
    }

    pub fn lookup_value(&mut self, name: &str, span: Span) -> Option<ValSym> {
        let m = self.cur_module();
        match self.visible(m, name, |s| &s.values) {
            Ok(v) => v.map(|e| e.sym),
            Err(mods) => {
                self.error(span, format!("`{}` is ambiguous: it is defined in {}", name, mods.join(" and ")));
                None
            }
        }
    }

    pub fn lookup_value_entry(&self, module: usize, name: &str) -> Option<Entry<ValSym>> {
        self.visible(module, name, |s| &s.values).ok().flatten()
    }

    pub fn is_private_elsewhere(&self, module: usize, name: &str) -> bool {
        self.mods[module].imports.iter().any(|i| {
            self.mods[*i].values.get(name).map(|e| e.vis == Vis::Priv).unwrap_or(false)
                || self.mods[*i].types.get(name).map(|e| e.vis == Vis::Priv).unwrap_or(false)
        })
    }

    pub fn lookup_type_name(&mut self, module: usize, name: &str, span: Span) -> Option<TyId> {
        match self.visible(module, name, |s| &s.types) {
            Ok(v) => v.map(|e| {
                self.def_link(span, e.span);
                self.warn_deprecated_type(e.sym, span);
                e.sym
            }),
            Err(mods) => {
                self.error(span, format!("type `{}` is ambiguous: it is defined in {}", name, mods.join(" and ")));
                Some(T_ERROR)
            }
        }
    }

    pub fn primitive(name: &str) -> Option<TyId> {
        Some(match name {
            "int" | "Int" | "i64" | "long" => T_INT,
            "float" | "Float" | "double" | "Double" | "f64" | "number" => T_FLOAT,
            "string" | "String" | "str" | "Str" => T_STR,
            "bool" | "Bool" | "boolean" | "Boolean" => T_BOOL,
            "void" | "Void" | "Unit" => T_VOID,
            "any" | "Any" | "object" => T_ANY,
            "null" => T_NULL,
            "array" | "Array" => T_ARR_ANY,
            "map" | "Map" => T_MAP_STR_ANY,
            _ => return None,
        })
    }

    pub fn resolve_type_in(&mut self, te: &TypeExpr, module: usize) -> TyId {
        match &te.kind {
            TypeExprKind::Named(name, args) => {
                if !args.is_empty() {
                    let a: Vec<TyId> = args.iter().map(|x| self.resolve_type_in(x, module)).collect();
                    return match (name.as_str(), a.len()) {
                        ("Future" | "Task" | "Promise", 1) => self.types.future(a[0]),
                        ("Array" | "List" | "array", 1) => self.types.array(a[0]),
                        ("Map" | "map" | "Dict", 2) => {
                            self.check_map_key(a[0], te.span);
                            self.types.map_of(a[0], a[1])
                        }
                        ("Optional" | "Option", 1) => self.types.optional(a[0]),
                        _ => {
                            self.error(te.span, format!("type `{}` does not take {} type argument(s)", name, a.len()));
                            T_ERROR
                        }
                    };
                }
                if let Some(t) = self.lookup_type_name(module, name, te.span) {
                    return t;
                }
                if let Some(p) = Self::primitive(name) {
                    return p;
                }
                let cands: Vec<String> = self.mods[module]
                    .types
                    .keys()
                    .cloned()
                    .chain(["int", "float", "string", "bool", "void", "any"].iter().map(|s| s.to_string()))
                    .collect();
                match suggest(name, cands.iter().map(|s| s.as_str())) {
                    Some(s) => self.error_fix(te.span, format!("unknown type `{}`", name), &s),
                    None => {
                        if self.is_private_elsewhere(module, name) {
                            self.error(te.span, format!("type `{}` is private to its module", name))
                        } else {
                            self.error(te.span, format!("unknown type `{}`", name))
                        }
                    }
                }
                T_ERROR
            }
            TypeExprKind::Array(e) => {
                let e = self.resolve_type_in(e, module);
                self.types.array(e)
            }
            TypeExprKind::Optional(e) => {
                let e = self.resolve_type_in(e, module);
                if e == T_VOID {
                    self.error(te.span, "`void?` is not a valid type");
                    return T_ERROR;
                }
                self.types.optional(e)
            }
            TypeExprKind::Map(k, v) => {
                let k = self.resolve_type_in(k, module);
                let v = self.resolve_type_in(v, module);
                self.check_map_key(k, te.span);
                self.types.map_of(k, v)
            }
            TypeExprKind::Func(ps, r) => {
                let ps: Vec<TyId> = ps.iter().map(|p| self.resolve_type_in(p, module)).collect();
                let r = r.as_ref().map(|r| self.resolve_type_in(r, module)).unwrap_or(T_VOID);
                self.types.func(ps, r)
            }
        }
    }

    fn check_map_key(&mut self, k: TyId, span: Span) {
        if !matches!(self.types.get(k), Ty::Str | Ty::Int | Ty::Bool | Ty::Enum(_) | Ty::Any | Ty::Error) {
            let s = self.show(k);
            self.error(span, format!("map keys must be string, int, bool, enum or any, not {}", s));
        }
    }

    pub fn resolve_type(&mut self, te: &TypeExpr) -> TyId {
        let m = self.cur_module();
        self.resolve_type_in(te, m)
    }

    fn run(&mut self, loaded: &Loaded) {
        for m in loaded.modules.iter() {
            let init = self.funcs.len() as FuncId;
            self.funcs.push(FuncInfo {
                name: format!("<init {}>", self.sm.file(m.file).name),
                params: Vec::new(),
                ret: Some(T_VOID),
                is_async: false,
                decl: None,
                module: self.mods.len(),
                self_ty: None,
                state: FnState::Done,
                hir: None,
                span: Span::new(m.file, 0, 0),
                private: false,
                is_static: true,
                annotations: Vec::new(),
                deprecated: None,
                external: None,
            });
            self.mods.push(ModScope {
                file: m.file,
                values: HashMap::new(),
                types: HashMap::new(),
                imports: m.imports.iter().map(|(i, _)| *i).collect(),
                init,
            });
        }
        let mut aliases: Vec<(usize, String, TypeExpr, Span)> = Vec::new();
        for (mi, m) in loaded.modules.iter().enumerate() {
            for item in &m.ast.items {
                if let ItemKind::Def(d) = &item.kind {
                    self.declare_def(mi, d, item.vis, &mut aliases);
                }
            }
        }
        for _ in 0..8 {
            let mut changed = false;
            for (mi, name, te, _) in &aliases {
                let before = self.mods[*mi].types.get(name).map(|e| e.sym);
                let saved = self.diags.len();
                let t = self.resolve_type_in(te, *mi);
                self.diags.truncate(saved);
                if Some(t) != before && t != T_ERROR {
                    self.mods[*mi].types.get_mut(name).unwrap().sym = t;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (mi, _name, te, _) in &aliases {
            self.resolve_type_in(te, *mi);
        }
        for (mi, m) in loaded.modules.iter().enumerate() {
            for item in &m.ast.items {
                if let ItemKind::Def(d) = &item.kind {
                    self.fill_def(mi, d);
                }
            }
        }
        self.link_structs();
        for (mi, m) in loaded.modules.iter().enumerate() {
            for item in &m.ast.items {
                if let ItemKind::Def(d) = &item.kind {
                    self.def_annotations(mi, d, &item.annotations);
                }
            }
        }
        self.import_libraries(loaded);
        for (mi, m) in loaded.modules.iter().enumerate() {
            for item in &m.ast.items {
                match &item.kind {
                    ItemKind::Fun(f) => {
                        let fid = self.declare_fun(mi, f, None, item.vis == Vis::Priv, false);
                        let info = &mut self.funcs[fid as usize];
                        if mi == loaded.root && item.vis == Vis::Pub && !f.bodyless && !info.annotations.iter().any(|a| a.name == "Export") {
                            info.annotations.push(hir::Annotation {
                                name: "Export".into(),
                                ty: None,
                                args: Vec::new(),
                                span: f.name.span,
                            });
                        }
                        self.add_value(mi, &f.name.name, ValSym::Func(fid), item.vis, f.name.span);
                    }
                    ItemKind::Def(Def::Struct { name, methods, statics, .. }) => {
                        let t = self.mods[mi].types.get(&name.name).map(|e| e.sym).unwrap_or(T_ERROR);
                        if let Ty::Record(ri) = self.types.get(t).clone() {
                            if self.types.records[ri as usize].span != name.span {
                                continue;
                            }
                            self.declare_struct_members(mi, ri, name, methods, statics);
                        }
                    }
                    ItemKind::Stmt(s) => {
                        if let ast::StmtKind::Var { name, is_const, .. } = &s.kind {
                            let gi = self.globals.len() as u32;
                            self.globals.push(GlobalInfo {
                                name: name.name.clone(),
                                ty: None,
                                is_const: *is_const,
                                module: mi,
                                span: name.span,
                            });
                            self.add_value(mi, &name.name, ValSym::Global(gi), item.vis, name.span);
                        }
                    }
                    _ => {}
                }
            }
        }
        self.check_structs();
        self.check_conformance(loaded);
        self.check_mixins();
        self.build_ctors();
        for &mi in &loaded.order {
            self.check_init(mi, &loaded.modules[mi].ast);
        }
        let mut i = 0;
        while i < self.funcs.len() {
            if self.funcs[i].state == FnState::Pending {
                self.check_func(i as FuncId);
            }
            i += 1;
        }
        self.check_init_order(loaded);
        let root = loaded.root;
        if let Some(Entry {
            sym: ValSym::Func(f), span, ..
        }) = self.mods[root].values.get("main").cloned()
        {
            if !self.funcs[f as usize].params.is_empty() {
                self.error(span, "`main` must not take parameters (use `args()` to read command line arguments)");
            }
        }
    }

    fn add_value(&mut self, mi: usize, name: &str, sym: ValSym, vis: Vis, span: Span) {
        if let Some(prev) = self.mods[mi].values.get(name) {
            let ps = prev.span;
            let (l, _) = self.sm.file(ps.file).line_col(ps.start as usize);
            self.error(span, format!("`{}` is already defined on line {}", name, l));
            return;
        }
        self.mods[mi].values.insert(name.to_string(), Entry { sym, vis, span });
    }

    fn add_type(&mut self, mi: usize, name: &ast::Ident, t: TyId, vis: Vis) {
        if Self::primitive(&name.name).is_some() {
            self.error(name.span, format!("`{}` is a built-in type name", name.name));
        }
        if let Some(prev) = self.mods[mi].types.get(&name.name) {
            let (l, _) = self.sm.file(prev.span.file).line_col(prev.span.start as usize);
            self.error(name.span, format!("type `{}` is already defined on line {}", name.name, l));
            return;
        }
        self.mods[mi].types.insert(name.name.clone(), Entry { sym: t, vis, span: name.span });
    }

    fn declare_def(&mut self, mi: usize, d: &Def, vis: Vis, aliases: &mut Vec<(usize, String, TypeExpr, Span)>) {
        match d {
            Def::Type { name, .. } | Def::Struct { name, .. } | Def::Annotation { name, .. } => {
                let kind = match d {
                    Def::Struct { kind, .. } => Some(*kind),
                    _ => None,
                };
                let ri = self.types.new_record(RecordDef {
                    name: name.name.clone(),
                    is_class: kind.is_some(),
                    is_abstract: kind == Some(ast::StructKind::Abstract),
                    is_static: kind == Some(ast::StructKind::Static),
                    module: mi as u32,
                    span: name.span,
                    ..Default::default()
                });
                let t = self.types.records[ri as usize].ty;
                self.add_type(mi, name, t, vis);
                if matches!(d, Def::Annotation { .. }) {
                    if annot::BUILTIN.contains(&name.name.as_str()) {
                        self.error(name.span, format!("@{} is a built-in annotation", name.name));
                    }
                    self.annotation_types.insert(t);
                }
            }
            Def::Interface { name, .. } => {
                let ii = self.types.new_iface(IfaceDef {
                    name: name.name.clone(),
                    methods: Vec::new(),
                    module: mi as u32,
                    span: name.span,
                    ty: 0,
                });
                let t = self.types.ifaces[ii as usize].ty;
                self.add_type(mi, name, t, vis);
            }
            Def::Enum { name, variants } => {
                let mut seen: HashMap<&str, Span> = HashMap::new();
                for v in variants {
                    if seen.insert(&v.name, v.span).is_some() {
                        self.error(v.span, format!("duplicate enum variant `{}`", v.name));
                    }
                }
                let ei = self.types.new_enum(EnumDef {
                    name: name.name.clone(),
                    variants: variants.iter().map(|v| (v.name.clone(), v.span)).collect(),
                    module: mi as u32,
                    span: name.span,
                    ty: 0,
                });
                let t = self.types.enums[ei as usize].ty;
                self.add_type(mi, name, t, vis);
            }
            Def::Alias { name, ty } => {
                self.add_type(mi, name, T_ERROR, vis);
                aliases.push((mi, name.name.clone(), ty.clone(), name.span));
            }
        }
    }

    fn fill_def(&mut self, mi: usize, d: &Def) {
        let name = d.name();
        let t = match self.mods[mi].types.get(&name.name) {
            Some(e) if e.span == name.span => e.sym,
            _ => return,
        };
        match d {
            Def::Type { fields, .. } | Def::Struct { fields, .. } | Def::Annotation { fields, .. } => {
                let ri = match self.types.get(t) {
                    Ty::Record(r) => *r,
                    _ => return,
                };
                let mut out = Vec::new();
                let mut pdecl = Vec::new();
                if let Def::Struct { params, .. } = d {
                    for p in params {
                        let pt = self.resolve_type_in(&p.ty, mi);
                        if pt == T_VOID {
                            self.error(p.ty.span, "parameters cannot have type void");
                        }
                        if out.iter().any(|x: &FieldDef| x.name == p.name.name) {
                            self.error(p.name.span, format!("duplicate parameter `{}`", p.name.name));
                            continue;
                        }
                        out.push(FieldDef {
                            name: p.name.name.clone(),
                            ty: pt,
                            default: None,
                            private: false,
                            span: p.name.span,
                        });
                        pdecl.push((p.name.name.clone(), pt, p.name.span));
                    }
                }
                for f in fields {
                    let ft = self.resolve_type_in(&f.ty, mi);
                    if ft == T_VOID {
                        self.error(f.ty.span, "fields cannot have type void");
                    }
                    if out.iter().any(|x: &FieldDef| x.name == f.name.name) {
                        self.error(f.name.span, format!("duplicate field `{}`", f.name.name));
                        continue;
                    }
                    out.push(FieldDef {
                        name: f.name.name.clone(),
                        ty: ft,
                        default: f.default.clone(),
                        private: f.vis == Vis::Priv,
                        span: f.name.span,
                    });
                }
                self.types.records[ri as usize].fields = out;
                if let Def::Struct {
                    name,
                    kind,
                    params,
                    extends,
                    supers,
                    colon_extra,
                    statics,
                    ..
                } = d
                {
                    self.fill_struct(mi, ri, name, *kind, params, (extends, supers, *colon_extra), statics, pdecl);
                }
            }
            Def::Interface { methods, .. } => {
                let ii = match self.types.get(t) {
                    Ty::Interface(i) => *i,
                    _ => return,
                };
                let mut out: Vec<IfaceMethod> = Vec::new();
                for m in methods {
                    if out.iter().any(|x| x.name == m.name.name) {
                        self.error(m.name.span, format!("duplicate interface method `{}`", m.name.name));
                        continue;
                    }
                    let params: Vec<TyId> = m.params.iter().map(|p| self.resolve_type_in(&p.ty, mi)).collect();
                    let ret = m.ret.as_ref().map(|r| self.resolve_type_in(r, mi)).unwrap_or(T_VOID);
                    let slot = self.slots.len() as u32;
                    self.slots.push(IfaceSlot {
                        name: format!("{}.{}", name.name, m.name.name),
                        argc: params.len() as u32 + 1,
                        impls: Vec::new(),
                    });
                    out.push(IfaceMethod {
                        name: m.name.name.clone(),
                        params,
                        ret,
                        is_async: m.is_async,
                        span: m.name.span,
                        slot,
                    });
                }
                self.iface_slot_base.insert(ii, 0);
                self.types.ifaces[ii as usize].methods = out;
            }
            _ => {}
        }
    }

    fn def_annotations(&mut self, mi: usize, d: &Def, anns: &[ast::Annotation]) {
        let name = d.name();
        let t = match self.mods[mi].types.get(&name.name) {
            Some(e) if e.span == name.span => e.sym,
            _ => return,
        };
        let site = annot::Site::Type {
            class: matches!(d, Def::Struct { .. }),
        };
        let resolved = self.resolve_annotations(mi, anns, site);
        if let Some(msg) = Self::deprecation_note(&resolved) {
            self.deprecated_types.insert(t, msg);
        }
        if !resolved.is_empty() {
            self.type_anns.insert(t, resolved);
        }
        let fields: &[ast::Field] = match d {
            Def::Type { fields, .. } | Def::Struct { fields, .. } | Def::Annotation { fields, .. } => fields,
            _ => &[],
        };
        for f in fields {
            if !f.annotations.is_empty() {
                self.resolve_annotations(mi, &f.annotations, annot::Site::Field);
            }
        }
        if let Def::Struct { param_anns, .. } = d {
            for a in param_anns {
                if !a.is_empty() {
                    self.resolve_annotations(mi, a, annot::Site::Field);
                }
            }
        }
    }

    fn declare_fun(&mut self, mi: usize, f: &ast::FunDecl, self_ty: Option<TyId>, private: bool, method: bool) -> FuncId {
        let mut params = Vec::new();
        if let Some(st) = self_ty {
            params.push(("self".to_string(), st, f.name.span));
        }
        for p in &f.params {
            let t = self.resolve_type_in(&p.ty, mi);
            if t == T_VOID {
                self.error(p.ty.span, "parameters cannot have type void");
            }
            if params.iter().any(|(n, _, _)| *n == p.name.name) {
                self.error(p.name.span, format!("duplicate parameter `{}`", p.name.name));
            }
            params.push((p.name.name.clone(), t, p.name.span));
        }
        let ret = f.ret.as_ref().map(|r| self.resolve_type_in(r, mi));
        let annotations = self.resolve_annotations(mi, &f.annotations, annot::Site::Func { method, bodyless: f.bodyless });
        let deprecated = Self::deprecation_note(&annotations);
        let external = annotations.iter().find(|a| a.name == "Native").map(|a| hir::External::Native {
            name: a.str_arg("name").unwrap_or(&f.name.name).to_string(),
        });
        if f.bodyless && external.is_none() {
            self.error_note(
                f.name.span,
                format!("function `{}` has no body", f.name.name),
                "only functions provided by the host can be declared without a body; mark them with @Native",
            );
        }
        if external.is_some() && f.ret.is_none() {
            let mut d = Diagnostic::error(f.name.span, format!("@Native function `{}` needs a return type", f.name.name));
            match self.params_close(f.name.span) {
                Some(at) => d = d.maybe_fix("if it returns nothing, say so", at, ": void"),
                None => d = d.help("write `: void` if it returns nothing"),
            }
            self.emit(d);
        }
        let fid = self.funcs.len() as FuncId;
        self.funcs.push(FuncInfo {
            name: f.name.name.clone(),
            params,
            ret,
            is_async: f.is_async,
            decl: Some(Rc::new(f.clone())),
            module: mi,
            self_ty,
            state: FnState::Pending,
            hir: None,
            span: f.name.span,
            private,
            is_static: self_ty.is_none(),
            annotations,
            deprecated,
            external,
        });
        fid
    }

    fn check_conformance(&mut self, _loaded: &Loaded) {
        for ri in 0..self.types.records.len() {
            let rec = self.types.records[ri].clone();
            if rec.is_abstract {
                continue;
            }
            for ii in rec.implements.iter() {
                let iface = self.types.ifaces[*ii as usize].clone();
                for m in &iface.methods {
                    match rec.methods.get(&m.name) {
                        Some(fid) => {
                            let fid = *fid;
                            let ret = self.func_ret(fid);
                            let info = &self.funcs[fid as usize];
                            let ps: Vec<TyId> = info.params.iter().skip(1).map(|p| p.1).collect();
                            let fspan = info.span;
                            let is_async = info.is_async;
                            if ps != m.params || ret != m.ret || is_async != m.is_async {
                                let want = self.method_sig_str(&m.params, m.ret, m.is_async);
                                let got = self.method_sig_str(&ps, ret, is_async);
                                self.error_detail(
                                    fspan,
                                    format!("method `{}` does not match interface `{}`", m.name, iface.name),
                                    format!("expected `{}` but found `{}`", want, got),
                                );
                            }
                            self.slots[m.slot as usize].impls.push((rec.ty, fid));
                        }
                        None => {
                            let want = self.method_sig_str(&m.params, m.ret, m.is_async);
                            self.error_note(
                                rec.span,
                                format!("struct `{}` does not implement `{}.{}`", rec.name, iface.name, m.name),
                                format!("add `{}`", want.replacen("fun(", &format!("fun {}(", m.name), 1)),
                            );
                        }
                    }
                }
            }
        }
    }

    fn method_sig_str(&self, ps: &[TyId], ret: TyId, is_async: bool) -> String {
        let p: Vec<String> = ps.iter().map(|t| self.show(*t)).collect();
        let r = if ret == T_VOID { String::new() } else { format!(": {}", self.show(ret)) };
        format!("{}fun({}){}", if is_async { "async " } else { "" }, p.join(", "), r)
    }

    pub fn func_ret(&mut self, fid: FuncId) -> TyId {
        if let Some(r) = self.funcs[fid as usize].ret {
            return r;
        }
        match self.funcs[fid as usize].state {
            FnState::Pending => {
                self.check_func(fid);
                self.funcs[fid as usize].ret.unwrap_or(T_ERROR)
            }
            FnState::InProgress => {
                let span = self.funcs[fid as usize].span;
                let name = self.funcs[fid as usize].name.clone();
                if !self.is_dry() {
                    self.error_note(
                        span,
                        format!("cannot infer the return type of `{}` because it calls itself", name),
                        "add a return type, for example `fun f(): int`",
                    );
                }
                T_ERROR
            }
            FnState::Done => T_ERROR,
        }
    }

    pub fn func_type(&mut self, fid: FuncId) -> TyId {
        let ret = self.func_ret(fid);
        let ps: Vec<TyId> = self.funcs[fid as usize].params.iter().map(|p| p.1).collect();
        let ret = if self.funcs[fid as usize].is_async { self.types.future(ret) } else { ret };
        self.types.func(ps, ret)
    }

    fn new_ctx(&self, fid: FuncId, module: usize, dry: bool) -> FnCtx {
        let info = &self.funcs[fid as usize];
        FnCtx {
            func: fid,
            module,
            locals: Vec::new(),
            scopes: vec![HashMap::new()],
            narrow: HashMap::new(),
            ret: info.ret,
            returns: Vec::new(),
            loops: 0,
            self_ty: info.self_ty,
            is_init: false,
            dry,
            lambda: self.closures.contains_key(&fid),
            cell_names: HashSet::new(),
            cells: HashMap::new(),
            captures: Vec::new(),
        }
    }

    pub fn check_func(&mut self, fid: FuncId) {
        let decl = match self.funcs[fid as usize].decl.clone() {
            Some(d) => d,
            None => return,
        };
        if decl.bodyless {
            let info = &mut self.funcs[fid as usize];
            let ret = info.ret.unwrap_or(T_VOID);
            info.ret = Some(ret);
            info.hir = Some(hir::Func {
                name: info.name.clone(),
                params: info.params.len() as u32,
                locals: info.params.iter().map(|p| p.1).collect(),
                ret,
                body: Vec::new(),
                is_async: info.is_async,
                span: info.span,
                end_loc: 0,
                annotations: info.annotations.clone(),
                external: info.external.clone(),
            });
            info.state = FnState::Done;
            return;
        }
        self.funcs[fid as usize].state = FnState::InProgress;
        let module = self.funcs[fid as usize].module;
        let cell_names = closures::cell_names(&decl.body.stmts);
        let is_lambda = self.closures.contains_key(&fid);
        if self.funcs[fid as usize].ret.is_none() {
            let mut ctx = self.new_ctx(fid, module, true);
            self.bind_params(&mut ctx, fid, &decl, true);
            ctx.cell_names = cell_names.clone();
            if is_lambda {
                ctx.locals.push(T_INT);
            }
            self.fx.push(ctx);
            self.block_stmts(&decl.body.stmts);
            let ctx = self.fx.pop().unwrap();
            let ret = self.join_returns(&ctx.returns, decl.name.span);
            self.funcs[fid as usize].ret = Some(ret);
        }
        let mut ctx = self.new_ctx(fid, module, false);
        self.bind_params(&mut ctx, fid, &decl, false);
        ctx.cell_names = cell_names;
        if is_lambda {
            ctx.locals.push(T_INT);
        }
        let nparams = self.funcs[fid as usize].params.len();
        self.fx.push(ctx);
        let mut prefix = Vec::new();
        for i in 0..nparams {
            let pname = self.funcs[fid as usize].params[i].0.clone();
            if pname != "self" && self.ctx().cell_names.contains(&pname) {
                let pt = self.ctx().locals[i];
                let ct = self.make_cell_local(i as u32);
                let cell = Expr::new(ExprKind::NewStruct(ct, vec![Expr::new(ExprKind::Local(i as u32), pt)]), ct);
                prefix.push(Stmt::Expr(Expr::new(ExprKind::SetLocal(i as u32, Box::new(cell)), pt)));
            }
        }
        let mut body = self.block_stmts(&decl.body.stmts);
        let ret = self.funcs[fid as usize].ret.unwrap_or(T_VOID);
        if ret != T_VOID && ret != T_ERROR && !stmt::diverges(&body) {
            let end = Span::new(decl.body.span.file, decl.body.span.end as usize - 1, decl.body.span.end as usize);
            let l = self.loc_expr(end);
            body.push(Stmt::Expr(Expr::new(ExprKind::Rt(RtFn::ErrReturn, vec![l]), T_VOID)));
        }
        let ctx = self.fx.pop().unwrap();
        let mut locals = ctx.locals;
        prefix.extend(body);
        let mut body = prefix;
        self.finish_cells(&mut body, &mut locals, &ctx.cells);
        if is_lambda {
            let mut fields = Vec::new();
            let mut caps = Vec::new();
            for (ps, inner, by_cell) in &ctx.captures {
                let ty = locals[*inner as usize];
                fields.push(ty);
                caps.push((*ps, *by_cell, ty));
            }
            let cty = self.closure_record(fields);
            locals[nparams] = cty;
            let mut loads: Vec<Stmt> = caps
                .iter()
                .enumerate()
                .map(|(k, (_, _, ty))| {
                    let env = Expr::new(ExprKind::Local(nparams as u32), cty);
                    let get = Expr::new(ExprKind::GetField(Box::new(env), k as u32 + 1), *ty);
                    Stmt::Expr(Expr::new(ExprKind::SetLocal(ctx.captures[k].1, Box::new(get)), *ty))
                })
                .collect();
            loads.extend(body);
            body = loads;
            self.closures.insert(fid, closures::ClosureInfo { ty: cty, captures: caps });
        }
        let end_loc = self.loc(decl.span);
        let info = &mut self.funcs[fid as usize];
        info.hir = Some(hir::Func {
            name: info.name.clone(),
            params: info.params.len() as u32 + is_lambda as u32,
            locals,
            ret,
            body,
            is_async: info.is_async,
            span: info.span,
            end_loc,
            annotations: info.annotations.clone(),
            external: info.external.clone(),
        });
        info.state = FnState::Done;
        let sig = {
            let info = &self.funcs[fid as usize];
            let ps: Vec<String> = info
                .params
                .iter()
                .filter(|p| p.0 != "self")
                .map(|p| format!("{}: {}", p.0, self.types.display(p.1)))
                .collect();
            let r = if ret == T_VOID {
                String::new()
            } else {
                format!(": {}", self.types.display(ret))
            };
            format!("{}fun {}({}){}", if info.is_async { "async " } else { "" }, info.name, ps.join(", "), r)
        };
        let sig = self.with_doc(sig, decl.name.span);
        self.hover(decl.name.span, sig);
    }

    fn bind_params(&mut self, ctx: &mut FnCtx, fid: FuncId, decl: &ast::FunDecl, dry: bool) {
        let params = self.funcs[fid as usize].params.clone();
        for (i, (name, ty, span)) in params.iter().enumerate() {
            ctx.locals.push(*ty);
            ctx.scopes[0].insert(
                name.clone(),
                LocalSym {
                    slot: i as u32,
                    is_const: false,
                    span: *span,
                },
            );
            if self.opts.want_index && !dry && name != "self" {
                self.index.locals.push(LocalInfo {
                    name: name.clone(),
                    ty: *ty,
                    decl: *span,
                    scope: decl.span,
                });
                self.index.hovers.push((*span, format!("{}: {}", name, self.types.display(*ty))));
            }
        }
    }

    pub fn join_returns(&mut self, rs: &[TyId], span: Span) -> TyId {
        let mut cur: Option<TyId> = None;
        let mut nullable = false;
        for r in rs {
            let r = *r;
            if r == T_NULL {
                nullable = true;
                continue;
            }
            if r == T_ERROR {
                continue;
            }
            cur = Some(match cur {
                None => r,
                Some(c) if c == r => c,
                Some(c) if (c == T_INT && r == T_FLOAT) || (c == T_FLOAT && r == T_INT) => T_FLOAT,
                Some(c) => {
                    let u = self.types.unwrap_optional(c);
                    if u == r {
                        c
                    } else if self.types.unwrap_optional(r) == c {
                        r
                    } else if let Some(t) = self.common_type(c, r) {
                        t
                    } else {
                        let (a, b) = (self.show(c), self.show(r));
                        let fname = self.sm.file(span.file).text(span).to_string();
                        let mut d = Diagnostic::error(span, format!("`{}` returns {} on one path and {} on another", fname, a, b));
                        if let Some(at) = self.params_close(span) {
                            d = d.maybe_fix("declare what it returns, `any` if both are intended", at, ": any");
                        }
                        self.emit(d);
                        T_ERROR
                    }
                }
            });
        }
        match cur {
            None => {
                if nullable {
                    let fname = self.sm.file(span.file).text(span).to_string();
                    self.emit(
                        Diagnostic::error(span, format!("cannot tell what `{}` returns: it only returns `null`", fname))
                            .help("declare the return type, for example `fun find(): string?`"),
                    );
                    T_ERROR
                } else {
                    T_VOID
                }
            }
            Some(t) => {
                if nullable {
                    self.types.optional(t)
                } else {
                    t
                }
            }
        }
    }

    fn check_init(&mut self, mi: usize, ast: &ast::Module) {
        let fid = self.mods[mi].init;
        let mut ctx = self.new_ctx(fid, mi, false);
        ctx.is_init = true;
        ctx.ret = Some(T_VOID);
        let top: Vec<ast::Stmt> = ast
            .items
            .iter()
            .filter_map(|i| match &i.kind {
                ItemKind::Stmt(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        ctx.cell_names = closures::cell_names(&top);
        self.fx.push(ctx);
        let mut body = Vec::new();
        let skip = self.opts.skip_before;
        let file = self.mods[mi].file;
        let n = ast.items.len();
        for (idx, item) in ast.items.iter().enumerate() {
            if let ItemKind::Def(Def::Struct { name, statics, .. }) = &item.kind {
                if !statics.is_empty() {
                    let skipped = matches!(skip, Some((f, off)) if f == file && item.span.start < off);
                    if skipped {
                        self.ctx().dry = true;
                    }
                    let out = self.static_inits(name);
                    if skipped {
                        self.ctx().dry = false;
                    } else {
                        let start = body.len();
                        body.extend(out);
                        self.init_spans.push((mi, start, body.len(), item.span));
                    }
                }
            }
            if let ItemKind::Stmt(s) = &item.kind {
                let skipped = matches!(skip, Some((f, off)) if f == file && item.span.start < off);
                let echo = self.opts.repl_echo && idx + 1 == n && !skipped && matches!(skip, Some((f, _)) if f == file);
                if skipped {
                    self.ctx().dry = true;
                }
                let out = if echo {
                    if let ast::StmtKind::Expr(e) = &s.kind {
                        self.repl_echo(e)
                    } else {
                        self.stmt(s)
                    }
                } else {
                    self.stmt(s)
                };
                if skipped {
                    self.ctx().dry = false;
                } else {
                    let start = body.len();
                    body.extend(out);
                    self.init_spans.push((mi, start, body.len(), item.span));
                }
            }
        }
        let ctx = self.fx.pop().unwrap();
        let mut locals = ctx.locals;
        self.finish_cells(&mut body, &mut locals, &ctx.cells);
        let end_loc = self.loc(Span::new(file, 0, 0));
        let info = &mut self.funcs[fid as usize];
        info.hir = Some(hir::Func {
            name: info.name.clone(),
            params: 0,
            locals,
            ret: T_VOID,
            body,
            is_async: false,
            span: info.span,
            end_loc,
            annotations: Vec::new(),
            external: None,
        });
    }

    fn repl_echo(&mut self, e: &ast::Expr) -> Vec<Stmt> {
        let h = self.expr(e, None);
        if h.ty == T_VOID || h.ty == T_ERROR || matches!(e.kind, ast::ExprKind::Assign { .. }) {
            return vec![Stmt::Expr(h)];
        }
        let s = self.stringify(h);
        vec![Stmt::Expr(Expr::new(ExprKind::Rt(RtFn::Print, vec![s]), T_VOID))]
    }

    fn build_program(&mut self, loaded: &Loaded) -> hir::Program {
        self.devirtualize();
        let entry = self.funcs.len() as FuncId;
        let mut body = Vec::new();
        for &mi in &loaded.order {
            body.push(Stmt::Expr(Expr::new(ExprKind::Call(self.mods[mi].init, vec![]), T_VOID)));
        }
        if let Some(Entry { sym: ValSym::Func(f), .. }) = self.mods[loaded.root].values.get("main").cloned() {
            if self.funcs[f as usize].params.is_empty() {
                let ret = self.funcs[f as usize].ret.unwrap_or(T_VOID);
                let ty = if self.funcs[f as usize].is_async { self.types.future(ret) } else { ret };
                let call = if self.funcs[f as usize].is_async {
                    let sp = Expr::new(ExprKind::Spawn(f, vec![]), ty);
                    Expr::new(ExprKind::Rt(RtFn::Await, vec![sp]), ret)
                } else {
                    Expr::new(ExprKind::Call(f, vec![]), ty)
                };
                body.push(Stmt::Expr(call));
            }
        }
        let mut funcs: Vec<hir::Func> = Vec::with_capacity(self.funcs.len() + 1);
        for f in &self.funcs {
            funcs.push(f.hir.clone().unwrap_or(hir::Func {
                name: f.name.clone(),
                params: f.params.len() as u32,
                locals: f.params.iter().map(|p| p.1).collect(),
                ret: f.ret.unwrap_or(T_VOID),
                body: Vec::new(),
                is_async: f.is_async,
                span: f.span,
                end_loc: 0,
                annotations: f.annotations.clone(),
                external: f.external.clone(),
            }));
        }
        funcs.push(hir::Func {
            name: "<entry>".into(),
            params: 0,
            locals: vec![],
            ret: T_VOID,
            body,
            is_async: false,
            span: Span::default(),
            end_loc: 0,
            annotations: Vec::new(),
            external: None,
        });
        let types = self.types.clone();
        let globals = self
            .globals
            .iter()
            .map(|g| hir::Global {
                name: g.name.clone(),
                ty: g.ty.unwrap_or(T_ERROR),
                module: loaded.modules[g.module].key.clone(),
            })
            .collect();
        let inits = loaded.order.iter().map(|&mi| (loaded.modules[mi].key.clone(), self.mods[mi].init)).collect();
        let main = match self.mods[loaded.root].values.get("main") {
            Some(Entry { sym: ValSym::Func(f), .. }) => Some(*f),
            _ => None,
        };

        hir::Program {
            types,
            funcs,
            globals,
            entry,
            strings: self.strings.clone(),
            locs: self.locs.clone(),
            slots: self.slots.clone(),
            inits,
            main,
            type_annotations: {
                let mut v: Vec<(TyId, Vec<hir::Annotation>)> = self.type_anns.iter().map(|(t, a)| (*t, a.clone())).collect();
                v.sort_by_key(|x| x.0);
                v
            },
            libs: self.libs.clone(),
            name: std::path::Path::new(&self.sm.file(loaded.modules[loaded.root].file).name)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }
}

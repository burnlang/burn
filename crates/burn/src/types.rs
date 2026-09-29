use crate::source::Span;
use burn_runtime::meta::{self, Desc};
use std::collections::HashMap;

pub type TyId = u32;

pub const T_ERROR: TyId = meta::TID_ERROR;
pub const T_VOID: TyId = meta::TID_VOID;
pub const T_NULL: TyId = meta::TID_NULL;
pub const T_INT: TyId = meta::TID_INT;
pub const T_FLOAT: TyId = meta::TID_FLOAT;
pub const T_BOOL: TyId = meta::TID_BOOL;
pub const T_STR: TyId = meta::TID_STR;
pub const T_ANY: TyId = meta::TID_ANY;
pub const T_ARR_ANY: TyId = meta::TID_ARR_ANY;
pub const T_MAP_STR_ANY: TyId = meta::TID_MAP_STR_ANY;
pub const T_ARR_STR: TyId = meta::TID_ARR_STR;
pub const T_ARR_INT: TyId = meta::TID_ARR_INT;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Error,
    Void,
    Null,
    Int,
    Float,
    Bool,
    Str,
    Any,
    Array(TyId),
    Map(TyId, TyId),
    Optional(TyId),
    Func(Vec<TyId>, TyId),
    Future(TyId),
    Record(u32),
    Interface(u32),
    Enum(u32),
}

#[derive(Clone, Debug)]
pub struct FieldDef {
    pub name: String,
    pub ty: TyId,
    pub default: Option<crate::ast::Expr>,
    pub private: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct RecordDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub is_class: bool,
    pub anon: bool,
    pub implements: Vec<u32>,
    pub methods: HashMap<String, u32>,
    pub statics: HashMap<String, u32>,
    pub init: Option<u32>,
    pub module: u32,
    pub span: Span,
    pub ty: TyId,
}

impl RecordDef {
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }
}

#[derive(Clone, Debug)]
pub struct IfaceMethod {
    pub name: String,
    pub params: Vec<TyId>,
    pub ret: TyId,
    pub is_async: bool,
    pub span: Span,
    pub slot: u32,
}

#[derive(Clone, Debug)]
pub struct IfaceDef {
    pub name: String,
    pub methods: Vec<IfaceMethod>,
    pub module: u32,
    pub span: Span,
    pub ty: TyId,
}

#[derive(Clone, Debug)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<(String, Span)>,
    pub module: u32,
    pub span: Span,
    pub ty: TyId,
}

#[derive(Clone)]
pub struct Types {
    kinds: Vec<Ty>,
    map: HashMap<Ty, TyId>,
    pub records: Vec<RecordDef>,
    pub ifaces: Vec<IfaceDef>,
    pub enums: Vec<EnumDef>,
}

impl Default for Types {
    fn default() -> Self {
        Types::new()
    }
}

impl Types {
    pub fn new() -> Types {
        let mut t = Types { kinds: Vec::new(), map: HashMap::new(), records: Vec::new(), ifaces: Vec::new(), enums: Vec::new() };
        for k in [Ty::Error, Ty::Void, Ty::Null, Ty::Int, Ty::Float, Ty::Bool, Ty::Str, Ty::Any] {
            t.intern(k);
        }
        t.intern(Ty::Array(T_ANY));
        t.intern(Ty::Map(T_STR, T_ANY));
        t.intern(Ty::Array(T_STR));
        t.intern(Ty::Array(T_INT));
        t
    }

    pub fn intern(&mut self, k: Ty) -> TyId {
        if let Some(id) = self.map.get(&k) {
            return *id;
        }
        let id = self.kinds.len() as TyId;
        self.kinds.push(k.clone());
        self.map.insert(k, id);
        id
    }

    pub fn get(&self, id: TyId) -> &Ty {
        &self.kinds[id as usize]
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    pub fn array(&mut self, e: TyId) -> TyId {
        self.intern(Ty::Array(e))
    }

    pub fn map_of(&mut self, k: TyId, v: TyId) -> TyId {
        self.intern(Ty::Map(k, v))
    }

    pub fn optional(&mut self, t: TyId) -> TyId {
        match self.get(t) {
            Ty::Optional(_) | Ty::Any | Ty::Null | Ty::Error => t,
            _ => self.intern(Ty::Optional(t)),
        }
    }

    pub fn future(&mut self, t: TyId) -> TyId {
        self.intern(Ty::Future(t))
    }

    pub fn func(&mut self, params: Vec<TyId>, ret: TyId) -> TyId {
        self.intern(Ty::Func(params, ret))
    }

    pub fn new_record(&mut self, mut def: RecordDef) -> u32 {
        let idx = self.records.len() as u32;
        let ty = self.intern(Ty::Record(idx));
        def.ty = ty;
        self.records.push(def);
        idx
    }

    pub fn new_iface(&mut self, mut def: IfaceDef) -> u32 {
        let idx = self.ifaces.len() as u32;
        def.ty = self.intern(Ty::Interface(idx));
        self.ifaces.push(def);
        idx
    }

    pub fn new_enum(&mut self, mut def: EnumDef) -> u32 {
        let idx = self.enums.len() as u32;
        def.ty = self.intern(Ty::Enum(idx));
        self.enums.push(def);
        idx
    }

    pub fn record_of(&self, t: TyId) -> Option<&RecordDef> {
        match self.get(t) {
            Ty::Record(i) => Some(&self.records[*i as usize]),
            _ => None,
        }
    }

    pub fn is_unboxed(&self, t: TyId) -> bool {
        matches!(self.get(t), Ty::Int | Ty::Float | Ty::Bool | Ty::Enum(_) | Ty::Func(..) | Ty::Void)
    }

    pub fn is_nullable(&self, t: TyId) -> bool {
        matches!(self.get(t), Ty::Optional(_) | Ty::Any | Ty::Null | Ty::Error)
    }

    pub fn is_numeric(&self, t: TyId) -> bool {
        matches!(self.get(t), Ty::Int | Ty::Float)
    }

    pub fn unwrap_optional(&self, t: TyId) -> TyId {
        match self.get(t) {
            Ty::Optional(i) => *i,
            _ => t,
        }
    }

    pub fn implements(&self, class: TyId, iface: TyId) -> bool {
        match (self.get(class), self.get(iface)) {
            (Ty::Record(r), Ty::Interface(i)) => self.records[*r as usize].implements.contains(i),
            _ => false,
        }
    }

    pub fn display(&self, t: TyId) -> String {
        match self.get(t) {
            Ty::Error => "<error>".into(),
            Ty::Void => "void".into(),
            Ty::Null => "null".into(),
            Ty::Int => "int".into(),
            Ty::Float => "float".into(),
            Ty::Bool => "bool".into(),
            Ty::Str => "string".into(),
            Ty::Any => "any".into(),
            Ty::Array(e) => format!("[{}]", self.display(*e)),
            Ty::Map(k, v) => format!("{{{}: {}}}", self.display(*k), self.display(*v)),
            Ty::Optional(i) => {
                if matches!(self.get(*i), Ty::Func(..)) {
                    format!("({})?", self.display(*i))
                } else {
                    format!("{}?", self.display(*i))
                }
            }
            Ty::Func(ps, r) => {
                let p: Vec<String> = ps.iter().map(|x| self.display(*x)).collect();
                if *r == T_VOID {
                    format!("fun({})", p.join(", "))
                } else {
                    format!("fun({}): {}", p.join(", "), self.display(*r))
                }
            }
            Ty::Future(i) => format!("Future<{}>", self.display(*i)),
            Ty::Record(i) => {
                let r = &self.records[*i as usize];
                if r.anon {
                    let parts: Vec<String> = r.fields.iter().map(|f| format!("{}: {}", f.name, self.display(f.ty))).collect();
                    format!("{{ {} }}", parts.join(", "))
                } else {
                    r.name.clone()
                }
            }
            Ty::Interface(i) => self.ifaces[*i as usize].name.clone(),
            Ty::Enum(i) => self.enums[*i as usize].name.clone(),
        }
    }

    pub fn descs(&self) -> Vec<Desc> {
        let mut out = Vec::with_capacity(self.kinds.len());
        for k in &self.kinds {
            out.push(match k {
                Ty::Error => Desc::Error,
                Ty::Void => Desc::Void,
                Ty::Null => Desc::Null,
                Ty::Int => Desc::Int,
                Ty::Float => Desc::Float,
                Ty::Bool => Desc::Bool,
                Ty::Str => Desc::Str,
                Ty::Any => Desc::Any,
                Ty::Array(e) => Desc::Array(*e),
                Ty::Map(a, b) => Desc::Map(*a, *b),
                Ty::Optional(i) => Desc::Optional(*i),
                Ty::Func(..) => Desc::Func,
                Ty::Future(i) => Desc::Future(*i),
                Ty::Record(i) => {
                    let r = &self.records[*i as usize];
                    Desc::Record {
                        name: if r.anon { String::new() } else { r.name.clone() },
                        fields: r.fields.iter().map(|f| (f.name.clone(), f.ty)).collect(),
                        class: r.is_class,
                        implements: r.implements.iter().map(|i| self.ifaces[*i as usize].ty).collect(),
                    }
                }
                Ty::Interface(i) => Desc::Interface { name: self.ifaces[*i as usize].name.clone() },
                Ty::Enum(i) => {
                    let e = &self.enums[*i as usize];
                    Desc::Enum { name: e.name.clone(), variants: e.variants.iter().map(|v| v.0.clone()).collect() }
                }
            });
        }
        out
    }
}

use crate::source::Span;
pub use burn_runtime::meta::Num;
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
pub const T_I8: TyId = 12;
pub const T_U8: TyId = 13;
pub const T_I16: TyId = 14;
pub const T_U16: TyId = 15;
pub const T_I32: TyId = 16;
pub const T_U32: TyId = 17;
pub const T_U64: TyId = 18;
pub const T_F32: TyId = 19;

pub fn num_ty(n: Num) -> TyId {
    T_I8 + Num::ALL.iter().position(|k| *k == n).unwrap() as TyId
}

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
    Num(Num),
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

#[derive(Clone, Debug, Default)]
pub struct RecordDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub is_class: bool,
    pub is_abstract: bool,
    pub is_static: bool,
    pub parent: Option<u32>,
    pub ctor: Option<u32>,
    pub static_vals: HashMap<String, u32>,
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
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub name: String,
    pub span: Span,
    pub record: u32,
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
        let mut t = Types {
            kinds: Vec::new(),
            map: HashMap::new(),
            records: Vec::new(),
            ifaces: Vec::new(),
            enums: Vec::new(),
        };
        for k in [Ty::Error, Ty::Void, Ty::Null, Ty::Int, Ty::Float, Ty::Bool, Ty::Str, Ty::Any] {
            t.intern(k);
        }
        t.intern(Ty::Array(T_ANY));
        t.intern(Ty::Map(T_STR, T_ANY));
        t.intern(Ty::Array(T_STR));
        t.intern(Ty::Array(T_INT));
        for n in Num::ALL {
            t.intern(Ty::Num(n));
        }
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

    pub fn variants_of(&self, t: TyId) -> Option<&IfaceDef> {
        match self.get(t) {
            Ty::Interface(i) if !self.ifaces[*i as usize].variants.is_empty() => Some(&self.ifaces[*i as usize]),
            _ => None,
        }
    }

    pub fn variant_of(&self, t: TyId) -> Option<(&IfaceDef, usize)> {
        let r = match self.get(t) {
            Ty::Record(r) => *r,
            _ => return None,
        };
        let rec = &self.records[r as usize];
        rec.implements.iter().find_map(|i| {
            let e = &self.ifaces[*i as usize];
            e.variants.iter().position(|v| v.record == r).map(|k| (e, k))
        })
    }

    pub fn record_of(&self, t: TyId) -> Option<&RecordDef> {
        match self.get(t) {
            Ty::Record(i) => Some(&self.records[*i as usize]),
            _ => None,
        }
    }

    pub fn is_unboxed(&self, t: TyId) -> bool {
        matches!(self.get(t), Ty::Int | Ty::Float | Ty::Num(_) | Ty::Bool | Ty::Enum(_) | Ty::Func(..) | Ty::Void)
    }

    pub fn is_nullable(&self, t: TyId) -> bool {
        matches!(self.get(t), Ty::Optional(_) | Ty::Any | Ty::Null | Ty::Error)
    }

    pub fn is_numeric(&self, t: TyId) -> bool {
        matches!(self.get(t), Ty::Int | Ty::Float | Ty::Num(_))
    }

    pub fn num_of(&self, t: TyId) -> Option<Num> {
        match self.get(t) {
            Ty::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn int_range(&self, t: TyId) -> Option<(u32, bool)> {
        match self.get(t) {
            Ty::Int => Some((64, true)),
            Ty::Num(n) if !n.is_float() => Some((n.bits(), n.signed())),
            _ => None,
        }
    }

    pub fn is_integer(&self, t: TyId) -> bool {
        self.int_range(t).is_some()
    }

    pub fn is_floating(&self, t: TyId) -> bool {
        matches!(self.get(t), Ty::Float | Ty::Num(Num::F32))
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
            (Ty::Record(r), Ty::Record(p)) => self.extends(*r, *p),
            _ => false,
        }
    }

    pub fn extends(&self, child: u32, ancestor: u32) -> bool {
        let mut cur = self.records[child as usize].parent;
        let mut steps = 0;
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            steps += 1;
            if steps > self.records.len() {
                return false;
            }
            cur = self.records[c as usize].parent;
        }
        false
    }

    pub fn ancestors(&self, r: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut cur = self.records[r as usize].parent;
        while let Some(c) = cur {
            if out.contains(&c) || c == r {
                break;
            }
            out.push(c);
            cur = self.records[c as usize].parent;
        }
        out
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
            Ty::Num(n) => n.name().into(),
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
                Ty::Num(n) => Desc::Num(*n),
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
                        implements: r
                            .implements
                            .iter()
                            .map(|i| self.ifaces[*i as usize].ty)
                            .chain(self.ancestors(*i).into_iter().map(|a| self.records[a as usize].ty))
                            .collect(),
                    }
                }
                Ty::Interface(i) => Desc::Interface {
                    name: self.ifaces[*i as usize].name.clone(),
                },
                Ty::Enum(i) => {
                    let e = &self.enums[*i as usize];
                    Desc::Enum {
                        name: e.name.clone(),
                        variants: e.variants.iter().map(|v| v.0.clone()).collect(),
                    }
                }
            });
        }
        out
    }
}

use crate::prelude::*;
use core::sync::atomic::{AtomicPtr, Ordering};

pub const TID_ERROR: u32 = 0;
pub const TID_VOID: u32 = 1;
pub const TID_NULL: u32 = 2;
pub const TID_INT: u32 = 3;
pub const TID_FLOAT: u32 = 4;
pub const TID_BOOL: u32 = 5;
pub const TID_STR: u32 = 6;
pub const TID_ANY: u32 = 7;
pub const TID_ARR_ANY: u32 = 8;
pub const TID_MAP_STR_ANY: u32 = 9;
pub const TID_ARR_STR: u32 = 10;
pub const TID_ARR_INT: u32 = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Num {
    I8 = 1,
    U8 = 2,
    I16 = 3,
    U16 = 4,
    I32 = 5,
    U32 = 6,
    U64 = 7,
    F32 = 8,
}

impl Num {
    pub const ALL: [Num; 8] = [Num::I8, Num::U8, Num::I16, Num::U16, Num::I32, Num::U32, Num::U64, Num::F32];

    pub fn from_code(c: u8) -> Option<Num> {
        Num::ALL.into_iter().find(|n| *n as u8 == c)
    }

    pub fn name(self) -> &'static str {
        match self {
            Num::I8 => "int8",
            Num::U8 => "uint8",
            Num::I16 => "int16",
            Num::U16 => "uint16",
            Num::I32 => "int32",
            Num::U32 => "uint32",
            Num::U64 => "uint64",
            Num::F32 => "float32",
        }
    }

    pub fn bits(self) -> u32 {
        match self {
            Num::I8 | Num::U8 => 8,
            Num::I16 | Num::U16 => 16,
            Num::I32 | Num::U32 | Num::F32 => 32,
            Num::U64 => 64,
        }
    }

    pub fn signed(self) -> bool {
        matches!(self, Num::I8 | Num::I16 | Num::I32 | Num::F32)
    }

    pub fn is_float(self) -> bool {
        self == Num::F32
    }

    pub fn packed(self) -> bool {
        self != Num::U64
    }

    pub fn width(self) -> usize {
        if self.packed() {
            (self.bits() / 8) as usize
        } else {
            8
        }
    }

    pub fn min_value(self) -> i128 {
        if self.signed() {
            -(1i128 << (self.bits() - 1))
        } else {
            0
        }
    }

    pub fn max_value(self) -> i128 {
        if self.signed() {
            (1i128 << (self.bits() - 1)) - 1
        } else {
            (1i128 << self.bits()) - 1
        }
    }

    pub fn fits(self, v: i128) -> bool {
        v >= self.min_value() && v <= self.max_value()
    }

    pub fn wrap(self, v: u64) -> u64 {
        match self {
            Num::I8 => v as i8 as i64 as u64,
            Num::U8 => v as u8 as u64,
            Num::I16 => v as i16 as i64 as u64,
            Num::U16 => v as u16 as u64,
            Num::I32 => v as i32 as i64 as u64,
            Num::U32 => v as u32 as u64,
            Num::U64 => v,
            Num::F32 => (f64::from_bits(v) as f32 as f64).to_bits(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Desc {
    Error,
    Void,
    Null,
    Int,
    Float,
    Bool,
    Str,
    Any,
    Num(Num),
    Array(u32),
    Map(u32, u32),
    Optional(u32),
    Func,
    Future(u32),
    Record {
        name: String,
        fields: Vec<(String, u32)>,
        class: bool,
        implements: Vec<u32>,
    },
    Interface {
        name: String,
    },
    Enum {
        name: String,
        variants: Vec<String>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct Meta {
    pub types: Vec<Desc>,
    pub locs: Vec<String>,
    pub info: Vec<u8>,
}

pub const I_MANAGED: u8 = 1;
pub const I_TRACK: u8 = 2;
pub const I_BOX_TRACK: u8 = 4;

fn is_managed(d: &Desc) -> bool {
    matches!(
        d,
        Desc::Str
            | Desc::Any
            | Desc::Array(_)
            | Desc::Map(..)
            | Desc::Optional(_)
            | Desc::Func
            | Desc::Future(_)
            | Desc::Record { .. }
            | Desc::Interface { .. }
    )
}

fn any_like(d: &Desc) -> bool {
    matches!(d, Desc::Any | Desc::Interface { .. } | Desc::Func)
}

fn children(d: &Desc) -> Vec<u32> {
    match d {
        Desc::Array(e) | Desc::Optional(e) | Desc::Future(e) => vec![*e],
        Desc::Map(k, v) => vec![*k, *v],
        Desc::Record { fields, .. } => fields.iter().map(|f| f.1).collect(),
        _ => Vec::new(),
    }
}

pub fn type_info(types: &[Desc]) -> Vec<u8> {
    let n = types.len();
    let get = |t: u32| types.get(t as usize).unwrap_or(&Desc::Error);
    let mut out = vec![0u8; n];
    let mut seen = vec![0u32; n];
    let mut stamp = 0u32;
    for t in 0..n {
        let d = &types[t];
        if !is_managed(d) {
            continue;
        }
        out[t] |= I_MANAGED;
        stamp += 1;
        let mut stack: Vec<u32> = children(d);
        let mut reaches_any = any_like(d);
        let mut reaches_self = false;
        while let Some(c) = stack.pop() {
            if c as usize >= n {
                continue;
            }
            if c as usize == t {
                reaches_self = true;
            }
            if seen[c as usize] == stamp {
                continue;
            }
            seen[c as usize] = stamp;
            let cd = get(c);
            if any_like(cd) {
                reaches_any = true;
            }
            stack.extend(children(cd));
        }
        let container = matches!(d, Desc::Array(_) | Desc::Map(..) | Desc::Record { .. } | Desc::Future(_));
        if container && (reaches_any || reaches_self) {
            out[t] |= I_TRACK;
        }
        if reaches_any {
            out[t] |= I_BOX_TRACK;
        }
    }
    out
}

pub fn builtin_descs() -> Vec<Desc> {
    vec![
        Desc::Error,
        Desc::Void,
        Desc::Null,
        Desc::Int,
        Desc::Float,
        Desc::Bool,
        Desc::Str,
        Desc::Any,
        Desc::Array(TID_ANY),
        Desc::Map(TID_STR, TID_ANY),
        Desc::Array(TID_STR),
        Desc::Array(TID_INT),
    ]
}

static META: AtomicPtr<Meta> = AtomicPtr::new(core::ptr::null_mut());

pub fn set_meta(mut m: Meta) {
    m.info = type_info(&m.types);
    let b = Box::into_raw(Box::new(m));
    META.store(b, Ordering::Release);
}

pub fn current_meta() -> *mut Meta {
    META.load(Ordering::Acquire)
}

pub fn restore_meta(m: *mut Meta) {
    META.store(m, Ordering::Release);
}

pub fn meta() -> &'static Meta {
    let p = META.load(Ordering::Acquire);
    if p.is_null() {
        set_meta(Meta {
            types: builtin_descs(),
            locs: Vec::new(),
            info: Vec::new(),
        });
        return meta();
    }
    unsafe { &*p }
}

#[inline]
pub fn desc(tid: u32) -> &'static Desc {
    static ERR: Desc = Desc::Error;
    meta().types.get(tid as usize).unwrap_or(&ERR)
}

#[inline]
pub fn info(tid: u32) -> u8 {
    meta().info.get(tid as usize).copied().unwrap_or(0)
}

#[inline]
pub fn managed(tid: u32) -> bool {
    info(tid) & I_MANAGED != 0
}

pub fn loc(i: u64) -> Option<&'static str> {
    meta().locs.get(i as usize).map(|s| s.as_str())
}

pub fn is_unboxed(tid: u32) -> bool {
    matches!(
        desc(tid),
        Desc::Int | Desc::Float | Desc::Num(_) | Desc::Bool | Desc::Enum { .. } | Desc::Func | Desc::Void
    )
}

pub fn implements(class_tid: u32, iface: u32) -> bool {
    match desc(class_tid) {
        Desc::Record { implements, .. } => implements.contains(&iface),
        _ => false,
    }
}

pub fn type_name(tid: u32) -> String {
    match desc(tid) {
        Desc::Error => "<error>".into(),
        Desc::Void => "void".into(),
        Desc::Null => "null".into(),
        Desc::Int => "int".into(),
        Desc::Float => "float".into(),
        Desc::Bool => "bool".into(),
        Desc::Str => "string".into(),
        Desc::Any => "any".into(),
        Desc::Num(n) => n.name().into(),
        Desc::Array(e) => format!("[{}]", type_name(*e)),
        Desc::Map(k, v) => format!("{{{}: {}}}", type_name(*k), type_name(*v)),
        Desc::Optional(t) => format!("{}?", type_name(*t)),
        Desc::Func => "fun".into(),
        Desc::Future(t) => format!("Future<{}>", type_name(*t)),
        Desc::Record { name, fields, .. } => {
            if name.is_empty() {
                let parts: Vec<String> = fields.iter().map(|(n, t)| format!("{}: {}", n, type_name(*t))).collect();
                format!("{{{}}}", parts.join(", "))
            } else {
                name.clone()
            }
        }
        Desc::Interface { name } => name.clone(),
        Desc::Enum { name, .. } => name.clone(),
    }
}

struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v)
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
    fn s(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes())
    }
}

struct R<'a>(&'a [u8], usize);

impl<'a> R<'a> {
    fn u8(&mut self) -> u8 {
        let v = self.0[self.1];
        self.1 += 1;
        v
    }
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.0[self.1..self.1 + 4].try_into().unwrap());
        self.1 += 4;
        v
    }
    fn s(&mut self) -> String {
        let n = self.u32() as usize;
        let v = String::from_utf8_lossy(&self.0[self.1..self.1 + n]).into_owned();
        self.1 += n;
        v
    }
}

pub fn encode(m: &Meta) -> Vec<u8> {
    let mut w = W(Vec::new());
    w.u32(m.types.len() as u32);
    for d in &m.types {
        match d {
            Desc::Error => w.u8(0),
            Desc::Void => w.u8(1),
            Desc::Null => w.u8(2),
            Desc::Int => w.u8(3),
            Desc::Float => w.u8(4),
            Desc::Bool => w.u8(5),
            Desc::Str => w.u8(6),
            Desc::Any => w.u8(7),
            Desc::Array(e) => {
                w.u8(8);
                w.u32(*e)
            }
            Desc::Map(k, v) => {
                w.u8(9);
                w.u32(*k);
                w.u32(*v)
            }
            Desc::Optional(t) => {
                w.u8(10);
                w.u32(*t)
            }
            Desc::Func => w.u8(11),
            Desc::Future(t) => {
                w.u8(12);
                w.u32(*t)
            }
            Desc::Record {
                name,
                fields,
                class,
                implements,
            } => {
                w.u8(13);
                w.s(name);
                w.u8(*class as u8);
                w.u32(fields.len() as u32);
                for (n, t) in fields {
                    w.s(n);
                    w.u32(*t);
                }
                w.u32(implements.len() as u32);
                for i in implements {
                    w.u32(*i);
                }
            }
            Desc::Interface { name } => {
                w.u8(14);
                w.s(name)
            }
            Desc::Num(n) => {
                w.u8(16);
                w.u8(*n as u8)
            }
            Desc::Enum { name, variants } => {
                w.u8(15);
                w.s(name);
                w.u32(variants.len() as u32);
                for v in variants {
                    w.s(v);
                }
            }
        }
    }
    w.u32(m.locs.len() as u32);
    for l in &m.locs {
        w.s(l);
    }
    w.0
}

pub fn decode(b: &[u8]) -> Meta {
    let mut r = R(b, 0);
    let n = r.u32();
    let mut types = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let d = match r.u8() {
            0 => Desc::Error,
            1 => Desc::Void,
            2 => Desc::Null,
            3 => Desc::Int,
            4 => Desc::Float,
            5 => Desc::Bool,
            6 => Desc::Str,
            7 => Desc::Any,
            8 => Desc::Array(r.u32()),
            9 => {
                let k = r.u32();
                Desc::Map(k, r.u32())
            }
            10 => Desc::Optional(r.u32()),
            11 => Desc::Func,
            12 => Desc::Future(r.u32()),
            13 => {
                let name = r.s();
                let class = r.u8() != 0;
                let nf = r.u32();
                let mut fields = Vec::new();
                for _ in 0..nf {
                    let n = r.s();
                    fields.push((n, r.u32()));
                }
                let ni = r.u32();
                let mut implements = Vec::new();
                for _ in 0..ni {
                    implements.push(r.u32());
                }
                Desc::Record {
                    name,
                    fields,
                    class,
                    implements,
                }
            }
            14 => Desc::Interface { name: r.s() },
            16 => Desc::Num(Num::from_code(r.u8()).unwrap_or(Num::U64)),
            _ => {
                let name = r.s();
                let nv = r.u32();
                let mut variants = Vec::new();
                for _ in 0..nv {
                    variants.push(r.s());
                }
                Desc::Enum { name, variants }
            }
        };
        types.push(d);
    }
    let nl = r.u32();
    let mut locs = Vec::with_capacity(nl as usize);
    for _ in 0..nl {
        locs.push(r.s());
    }
    Meta { types, locs, info: Vec::new() }
}

use crate::module::{Annotation, Function, Import, Module, Sig, Table, Target, Value, FIRST_USER_TYPE};
use crate::op::{rt_by_name, rt_name, Cmp, Op};
use burn_runtime::meta::Desc;
use std::collections::HashMap;

pub const MAGIC: &[u8; 4] = b"BVM\0";

pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

struct W {
    b: Vec<u8>,
}

impl W {
    fn u8(&mut self, v: u8) {
        self.b.push(v)
    }
    fn u16(&mut self, v: u16) {
        self.b.extend_from_slice(&v.to_le_bytes())
    }
    fn u32(&mut self, v: u32) {
        self.b.extend_from_slice(&v.to_le_bytes())
    }
    fn u64(&mut self, v: u64) {
        self.b.extend_from_slice(&v.to_le_bytes())
    }
    fn s(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.b.extend_from_slice(s.as_bytes())
    }
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

type R<T> = Result<T, String>;

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        if self.b.len() - self.i < n {
            return Err(format!("the file ends early (at byte {})", self.i));
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn u8(&mut self) -> R<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> R<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> R<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> R<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn count(&mut self, min_size: usize) -> R<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_size) > self.b.len() - self.i {
            return Err(format!("a count of {} at byte {} is larger than the file", n, self.i - 4));
        }
        Ok(n)
    }
    fn s(&mut self) -> R<String> {
        let n = self.count(1)?;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| format!("invalid UTF-8 in a string before byte {}", self.i))
    }
}

fn cmp_code(c: Cmp) -> u8 {
    Cmp::ALL.iter().position(|x| *x == c).unwrap() as u8
}

fn cmp_of(v: u8) -> R<Cmp> {
    Cmp::ALL.get(v as usize).copied().ok_or_else(|| format!("bad comparison code {}", v))
}

fn write_desc(w: &mut W, d: &Desc) {
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

fn read_desc(r: &mut Rd) -> R<Desc> {
    Ok(match r.u8()? {
        0 => Desc::Error,
        1 => Desc::Void,
        2 => Desc::Null,
        3 => Desc::Int,
        4 => Desc::Float,
        5 => Desc::Bool,
        6 => Desc::Str,
        7 => Desc::Any,
        8 => Desc::Array(r.u32()?),
        9 => {
            let k = r.u32()?;
            Desc::Map(k, r.u32()?)
        }
        10 => Desc::Optional(r.u32()?),
        11 => Desc::Func,
        12 => Desc::Future(r.u32()?),
        16 => {
            let c = r.u8()?;
            Desc::Num(burn_runtime::meta::Num::from_code(c).ok_or_else(|| format!("bad number type code {}", c))?)
        }
        13 => {
            let name = r.s()?;
            let class = r.u8()? != 0;
            let nf = r.count(8)?;
            let mut fields = Vec::with_capacity(nf);
            for _ in 0..nf {
                let n = r.s()?;
                fields.push((n, r.u32()?));
            }
            let ni = r.count(4)?;
            let mut implements = Vec::with_capacity(ni);
            for _ in 0..ni {
                implements.push(r.u32()?);
            }
            Desc::Record {
                name,
                fields,
                class,
                implements,
            }
        }
        14 => Desc::Interface { name: r.s()? },
        15 => {
            let name = r.s()?;
            let nv = r.count(4)?;
            let mut variants = Vec::with_capacity(nv);
            for _ in 0..nv {
                variants.push(r.s()?);
            }
            Desc::Enum { name, variants }
        }
        k => return Err(format!("unknown type kind {}", k)),
    })
}

fn write_op(w: &mut W, op: &Op, rt: &mut HashMap<&'static str, u32>, rt_list: &mut Vec<&'static str>) {
    let code: u8 = match op {
        Op::Const(_) => 0,
        Op::Str(_) => 1,
        Op::FuncRef(_) => 2,
        Op::Load(_) => 3,
        Op::Store(_) => 4,
        Op::Tee(_) => 5,
        Op::GLoad(_) => 6,
        Op::GStore(_) => 7,
        Op::GTee(_) => 8,
        Op::Pop => 9,
        Op::Dup => 10,
        Op::Swap => 11,
        Op::IAdd => 12,
        Op::ISub => 13,
        Op::IMul => 14,
        Op::IDiv(_) => 15,
        Op::IAddOv(_) => 59,
        Op::ISubOv(_) => 60,
        Op::IMulOv(_) => 61,
        Op::INegOv(_) => 62,
        Op::IRem(_) => 16,
        Op::INeg => 17,
        Op::And => 18,
        Op::Or => 19,
        Op::Xor => 20,
        Op::Shl => 21,
        Op::Shr => 22,
        Op::UShr => 23,
        Op::FAdd => 24,
        Op::FSub => 25,
        Op::FMul => 26,
        Op::FDiv => 27,
        Op::FRem => 28,
        Op::FNeg => 29,
        Op::ICmp(_) => 30,
        Op::UCmp(_) => 31,
        Op::FCmp(_) => 32,
        Op::Not => 33,
        Op::I2F => 34,
        Op::F2I => 35,
        Op::Jmp(_) => 36,
        Op::Jz(_) => 37,
        Op::Jnz(_) => 38,
        Op::JzKeep(_) => 39,
        Op::JnzKeep(_) => 40,
        Op::Call(_) => 41,
        Op::CallInd(_) => 42,
        Op::Dispatch(_, _) => 43,
        Op::Ret => 44,
        Op::RetVoid => 45,
        Op::Rt(_) => 46,
        Op::Host(_) => 47,
        Op::Spawn(_, _, _) => 48,
        Op::NewRecord(_, _) => 49,
        Op::GetField(_) => 50,
        Op::SetField(_) => 51,
        Op::NewArray(_, _) => 52,
        Op::Index(_) => 53,
        Op::SetIndex(_) => 54,
        Op::Len => 55,
        Op::Unbox => 56,
        Op::TypeConst(_) => 57,
        Op::LocConst(_) => 58,
        Op::IncLocal(..)
        | Op::JCmpLL(..)
        | Op::JCmpLC(..)
        | Op::LoadField(..)
        | Op::Load2(..)
        | Op::LoadK(..)
        | Op::CallSelf
        | Op::LoopJmp(_)
        | Op::IncLocalOv(..) => {
            panic!("{} is internal and is never written to a module", op.mnemonic())
        }
    };
    w.u8(code);
    match *op {
        Op::Const(v) => w.u64(v),
        Op::Str(x)
        | Op::TypeConst(x)
        | Op::LocConst(x)
        | Op::FuncRef(x)
        | Op::Load(x)
        | Op::Store(x)
        | Op::Tee(x)
        | Op::GLoad(x)
        | Op::GStore(x)
        | Op::GTee(x)
        | Op::IDiv(x)
        | Op::IAddOv(x)
        | Op::ISubOv(x)
        | Op::IMulOv(x)
        | Op::INegOv(x)
        | Op::IRem(x)
        | Op::Jmp(x)
        | Op::Jz(x)
        | Op::Jnz(x)
        | Op::JzKeep(x)
        | Op::JnzKeep(x)
        | Op::Call(x)
        | Op::CallInd(x)
        | Op::Host(x)
        | Op::GetField(x)
        | Op::SetField(x)
        | Op::Index(x)
        | Op::SetIndex(x) => w.u32(x),
        Op::ICmp(c) | Op::UCmp(c) | Op::FCmp(c) => w.u8(cmp_code(c)),
        Op::Dispatch(a, b) | Op::NewRecord(a, b) | Op::NewArray(a, b) => {
            w.u32(a);
            w.u32(b)
        }
        Op::Spawn(a, b, c) => {
            w.u32(a);
            w.u32(b);
            w.u32(c)
        }
        Op::Rt(f) => {
            let name = rt_name(f);
            let next = rt_list.len() as u32;
            let i = *rt.entry(name).or_insert_with(|| {
                rt_list.push(name);
                next
            });
            w.u32(i)
        }
        _ => {}
    }
}

fn read_op(r: &mut Rd, rt: &[Op]) -> R<Op> {
    let code = r.u8()?;
    Ok(match code {
        0 => Op::Const(r.u64()?),
        1 => Op::Str(r.u32()?),
        2 => Op::FuncRef(r.u32()?),
        3 => Op::Load(r.u32()?),
        4 => Op::Store(r.u32()?),
        5 => Op::Tee(r.u32()?),
        6 => Op::GLoad(r.u32()?),
        7 => Op::GStore(r.u32()?),
        8 => Op::GTee(r.u32()?),
        9 => Op::Pop,
        10 => Op::Dup,
        11 => Op::Swap,
        12 => Op::IAdd,
        13 => Op::ISub,
        14 => Op::IMul,
        15 => Op::IDiv(r.u32()?),
        16 => Op::IRem(r.u32()?),
        17 => Op::INeg,
        18 => Op::And,
        19 => Op::Or,
        20 => Op::Xor,
        21 => Op::Shl,
        22 => Op::Shr,
        23 => Op::UShr,
        24 => Op::FAdd,
        25 => Op::FSub,
        26 => Op::FMul,
        27 => Op::FDiv,
        28 => Op::FRem,
        29 => Op::FNeg,
        30 => Op::ICmp(cmp_of(r.u8()?)?),
        31 => Op::UCmp(cmp_of(r.u8()?)?),
        32 => Op::FCmp(cmp_of(r.u8()?)?),
        33 => Op::Not,
        34 => Op::I2F,
        35 => Op::F2I,
        36 => Op::Jmp(r.u32()?),
        37 => Op::Jz(r.u32()?),
        38 => Op::Jnz(r.u32()?),
        39 => Op::JzKeep(r.u32()?),
        40 => Op::JnzKeep(r.u32()?),
        41 => Op::Call(r.u32()?),
        42 => Op::CallInd(r.u32()?),
        43 => {
            let a = r.u32()?;
            Op::Dispatch(a, r.u32()?)
        }
        44 => Op::Ret,
        45 => Op::RetVoid,
        46 => {
            let i = r.u32()? as usize;
            *rt.get(i).ok_or_else(|| format!("runtime function reference {} is out of range", i))?
        }
        47 => Op::Host(r.u32()?),
        48 => {
            let a = r.u32()?;
            let b = r.u32()?;
            Op::Spawn(a, b, r.u32()?)
        }
        49 => {
            let a = r.u32()?;
            Op::NewRecord(a, r.u32()?)
        }
        50 => Op::GetField(r.u32()?),
        51 => Op::SetField(r.u32()?),
        52 => {
            let a = r.u32()?;
            Op::NewArray(a, r.u32()?)
        }
        53 => Op::Index(r.u32()?),
        54 => Op::SetIndex(r.u32()?),
        55 => Op::Len,
        56 => Op::Unbox,
        57 => Op::TypeConst(r.u32()?),
        58 => Op::LocConst(r.u32()?),
        59 => Op::IAddOv(r.u32()?),
        60 => Op::ISubOv(r.u32()?),
        61 => Op::IMulOv(r.u32()?),
        62 => Op::INegOv(r.u32()?),
        c => return Err(format!("unknown opcode {} at byte {}", c, r.i - 1)),
    })
}

pub fn encode(m: &Module) -> Vec<u8> {
    let mut rt: HashMap<&'static str, u32> = HashMap::new();
    let mut rt_list: Vec<&'static str> = Vec::new();
    let mut code = W { b: Vec::new() };
    code.u32(m.funcs.len() as u32);
    for f in &m.funcs {
        code.s(&f.name);
        code.u8(f.external as u8 | (f.sig.is_some() as u8) << 1);
        if let Some(sig) = &f.sig {
            code.u32(sig.params.len() as u32);
            for t in &sig.params {
                code.u32(*t);
            }
            code.u32(sig.ret);
        }
        code.u32(f.params);
        code.u32(f.locals);
        code.u32(f.names.len() as u32);
        for n in &f.names {
            code.s(n);
        }
        code.u32(f.code.len() as u32);
        for op in &f.code {
            write_op(&mut code, op, &mut rt, &mut rt_list);
        }
    }
    let mut w = W { b: Vec::new() };
    w.b.extend_from_slice(MAGIC);
    w.u16(crate::FORMAT_VERSION);
    w.u16(0);
    w.s(&m.name);
    let user = m.types.get(FIRST_USER_TYPE as usize..).unwrap_or(&[]);
    w.u32(user.len() as u32);
    for d in user {
        write_desc(&mut w, d);
    }
    for list in [&m.strings, &m.locs, &m.globals] {
        w.u32(list.len() as u32);
        for s in list {
            w.s(s);
        }
    }
    w.u32(m.imports.len() as u32);
    for i in &m.imports {
        w.s(&i.name);
        w.u32(i.argc);
    }
    w.u32(m.tables.len() as u32);
    for t in &m.tables {
        w.s(&t.name);
        w.u32(t.argc);
        w.u32(t.entries.len() as u32);
        for (tid, f) in &t.entries {
            w.u32(*tid);
            w.u32(*f);
        }
    }
    w.u32(m.entry.unwrap_or(u32::MAX));
    w.u32(rt_list.len() as u32);
    for n in &rt_list {
        w.s(n);
    }
    w.b.extend_from_slice(&code.b);
    w.u32(m.annotations.len() as u32);
    for a in &m.annotations {
        let (kind, idx) = match a.target {
            Target::Module => (0, 0),
            Target::Func(i) => (1, i),
            Target::Type(i) => (2, i),
            Target::Global(i) => (3, i),
        };
        w.u8(kind);
        w.u32(idx);
        w.s(&a.name);
        w.u32(a.args.len() as u32);
        for (k, v) in &a.args {
            w.s(k);
            match v {
                Value::Int(i) => {
                    w.u8(0);
                    w.u64(*i as u64)
                }
                Value::Float(f) => {
                    w.u8(1);
                    w.u64(f.to_bits())
                }
                Value::Bool(b) => {
                    w.u8(2);
                    w.u8(*b as u8)
                }
                Value::Str(s) => {
                    w.u8(3);
                    w.s(s)
                }
            }
        }
    }
    w.b
}

pub fn decode(bytes: &[u8]) -> Result<Module, String> {
    let mut r = Rd { b: bytes, i: 0 };
    if r.take(4).ok() != Some(MAGIC.as_slice()) {
        return Err("not a bvm module (the file does not start with BVM\\0)".into());
    }
    let version = r.u16()?;
    if version == 0 || version > crate::FORMAT_VERSION {
        return Err(format!(
            "the module uses bytecode format {}, but this bvm reads formats 1 to {}",
            version,
            crate::FORMAT_VERSION
        ));
    }
    let v2 = version >= 2;
    let _flags = r.u16()?;
    let mut m = Module::new();
    if v2 {
        m.name = r.s()?;
    }
    let nt = r.count(1)?;
    for _ in 0..nt {
        m.types.push(read_desc(&mut r)?);
    }
    let mut lists: [Vec<String>; 3] = Default::default();
    for l in lists.iter_mut() {
        let n = r.count(4)?;
        for _ in 0..n {
            l.push(r.s()?);
        }
    }
    let [strings, locs, globals] = lists;
    m.strings = strings;
    m.locs = locs;
    m.globals = globals;
    let ni = r.count(8)?;
    for _ in 0..ni {
        let name = r.s()?;
        m.imports.push(Import { name, argc: r.u32()? });
    }
    let ntab = r.count(12)?;
    for _ in 0..ntab {
        let name = r.s()?;
        let argc = r.u32()?;
        let ne = r.count(8)?;
        let mut entries = Vec::with_capacity(ne);
        for _ in 0..ne {
            let t = r.u32()?;
            entries.push((t, r.u32()?));
        }
        m.tables.push(Table { name, argc, entries });
    }
    let entry = r.u32()?;
    m.entry = if entry == u32::MAX { None } else { Some(entry) };
    let nrt = r.count(4)?;
    let mut rt = Vec::with_capacity(nrt);
    for _ in 0..nrt {
        let n = r.s()?;
        rt.push(Op::Rt(
            rt_by_name(&n).ok_or_else(|| format!("the module calls runtime function {}, which this bvm does not have", n))?,
        ));
    }
    let nf = r.count(16)?;
    for _ in 0..nf {
        let name = r.s()?;
        let (external, sig) = if v2 {
            let flags = r.u8()?;
            if flags > 3 {
                return Err(format!("bad function flags {}", flags));
            }
            let sig = if flags & 2 != 0 {
                let np = r.count(4)?;
                let mut params = Vec::with_capacity(np);
                for _ in 0..np {
                    params.push(r.u32()?);
                }
                Some(Sig { params, ret: r.u32()? })
            } else {
                None
            };
            (flags & 1 != 0, sig)
        } else {
            (false, None)
        };
        let params = r.u32()?;
        let locals = r.u32()?;
        let nn = r.count(4)?;
        let mut names = Vec::with_capacity(nn);
        for _ in 0..nn {
            names.push(r.s()?);
        }
        let nc = r.count(1)?;
        let mut code = Vec::with_capacity(nc);
        for _ in 0..nc {
            code.push(read_op(&mut r, &rt)?);
        }
        m.funcs.push(Function {
            name,
            params,
            locals,
            names,
            code,
            external,
            sig,
        });
    }
    if v2 {
        let na = r.count(10)?;
        for _ in 0..na {
            let kind = r.u8()?;
            let idx = r.u32()?;
            let target = match kind {
                0 => Target::Module,
                1 => Target::Func(idx),
                2 => Target::Type(idx),
                3 => Target::Global(idx),
                k => return Err(format!("bad annotation target kind {}", k)),
            };
            let name = r.s()?;
            let nargs = r.count(5)?;
            let mut args = Vec::with_capacity(nargs);
            for _ in 0..nargs {
                let k = r.s()?;
                let v = match r.u8()? {
                    0 => Value::Int(r.u64()? as i64),
                    1 => Value::Float(f64::from_bits(r.u64()?)),
                    2 => Value::Bool(r.u8()? != 0),
                    3 => Value::Str(r.s()?),
                    t => return Err(format!("bad annotation value tag {}", t)),
                };
                args.push((k, v));
            }
            m.annotations.push(Annotation { target, name, args });
        }
    }
    if r.i != bytes.len() {
        return Err(format!("{} unexpected bytes at the end of the module", bytes.len() - r.i));
    }
    Ok(m)
}

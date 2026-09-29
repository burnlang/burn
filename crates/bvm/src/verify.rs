use crate::module::{Function, Module};
use crate::op::{rt_name, Op};
use burn_runtime::meta::Desc;
use burn_runtime::RtFn;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct VerifyError {
    pub func: Option<String>,
    pub at: Option<usize>,
    pub msg: String,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (&self.func, self.at) {
            (Some(name), Some(at)) => write!(f, "in function {} at instruction {}: {}", name, at, self.msg),
            (Some(name), None) => write!(f, "in function {}: {}", name, self.msg),
            _ => write!(f, "{}", self.msg),
        }
    }
}

impl std::error::Error for VerifyError {}

pub const RESERVED_RT: &[RtFn] = &[RtFn::Init, RtFn::SetArgs, RtFn::Spawn];

pub const MAX_STACK: u32 = 1 << 16;

fn err(msg: String) -> VerifyError {
    VerifyError { func: None, at: None, msg }
}

pub fn verify(m: &Module) -> Result<(), VerifyError> {
    analyze(m).map(|_| ())
}

pub fn analyze(m: &Module) -> Result<Vec<u32>, VerifyError> {
    let nt = m.types.len() as u32;
    for (i, d) in m.types.iter().enumerate() {
        let refs: Vec<u32> = match d {
            Desc::Array(e) | Desc::Optional(e) | Desc::Future(e) => vec![*e],
            Desc::Map(k, v) => vec![*k, *v],
            Desc::Record { fields, implements, .. } => fields.iter().map(|f| f.1).chain(implements.iter().copied()).collect(),
            _ => Vec::new(),
        };
        if let Some(bad) = refs.into_iter().find(|t| *t >= nt) {
            return Err(err(format!("type #{} refers to type #{}, which does not exist", i, bad)));
        }
    }
    let nf = m.funcs.len() as u32;
    if let Some(e) = m.entry {
        match m.funcs.get(e as usize) {
            None => return Err(err(format!("entry function @{} does not exist", e))),
            Some(f) if f.params != 0 => return Err(err(format!("entry function {} must not take parameters", f.name))),
            _ => {}
        }
    }
    for t in &m.tables {
        for (tid, f) in &t.entries {
            if *tid >= nt {
                return Err(err(format!("table {} refers to type #{}, which does not exist", t.name, tid)));
            }
            match m.funcs.get(*f as usize) {
                None => return Err(err(format!("table {} refers to function @{}, which does not exist", t.name, f))),
                Some(func) if func.params != t.argc => {
                    return Err(err(format!(
                        "table {} takes {} arguments but function {} takes {}",
                        t.name, t.argc, func.name, func.params
                    )))
                }
                _ => {}
            }
        }
        if t.argc == 0 {
            return Err(err(format!("table {} must take at least the receiver as an argument", t.name)));
        }
    }
    let mut max = Vec::with_capacity(m.funcs.len());
    for f in &m.funcs {
        max.push(verify_func(m, f, nf, nt).map_err(|(at, msg)| VerifyError {
            func: Some(f.name.clone()),
            at,
            msg,
        })?);
    }
    Ok(max)
}

fn verify_func(m: &Module, f: &Function, nf: u32, nt: u32) -> Result<u32, (Option<usize>, String)> {
    if f.locals < f.params {
        return Err((None, format!("has {} locals but {} parameters", f.locals, f.params)));
    }
    if f.code.is_empty() {
        return Err((None, "has no code".into()));
    }
    let n = f.code.len();
    for (i, op) in f.code.iter().enumerate() {
        check_operands(m, f, op, nf, nt).map_err(|e| (Some(i), e))?;
        if let Some(t) = op.jump_target() {
            if t as usize >= n {
                return Err((Some(i), format!("jumps to {}, past the end of the function", t)));
            }
        }
    }
    let mut height: Vec<Option<u32>> = vec![None; n];
    let mut max = 0;
    let mut work = vec![(0usize, 0u32)];
    while let Some((pc, h)) = work.pop() {
        match height[pc] {
            Some(old) if old == h => continue,
            Some(old) => return Err((Some(pc), format!("the stack holds {} values on one path here and {} on another", old, h))),
            None => height[pc] = Some(h),
        }
        let op = &f.code[pc];
        let (need, push) = effect(m, op);
        if h < need {
            return Err((
                Some(pc),
                format!("{} needs {} values on the stack but there are only {}", op.mnemonic(), need, h),
            ));
        }
        let next = h - need + push;
        max = max.max(next).max(h);
        if next > MAX_STACK {
            return Err((Some(pc), "the operand stack grows without bound".into()));
        }
        if let Some(t) = op.jump_target() {
            work.push((t as usize, next));
        }
        if !op.is_terminator() {
            if pc + 1 >= n {
                return Err((Some(pc), "execution can run past the last instruction; end with ret, retv or jmp".into()));
            }
            work.push((pc + 1, next));
        }
    }
    Ok(max)
}

fn effect(m: &Module, op: &Op) -> (u32, u32) {
    match op {
        Op::Const(_) | Op::Str(_) | Op::FuncRef(_) | Op::Load(_) | Op::GLoad(_) => (0, 1),
        Op::Store(_) | Op::GStore(_) | Op::Pop => (1, 0),
        Op::Tee(_) | Op::GTee(_) => (1, 1),
        Op::Dup => (1, 2),
        Op::Swap => (2, 2),
        Op::IAdd
        | Op::ISub
        | Op::IMul
        | Op::IDiv(_)
        | Op::IRem(_)
        | Op::And
        | Op::Or
        | Op::Xor
        | Op::Shl
        | Op::Shr
        | Op::UShr
        | Op::FAdd
        | Op::FSub
        | Op::FMul
        | Op::FDiv
        | Op::FRem
        | Op::ICmp(_)
        | Op::UCmp(_)
        | Op::FCmp(_) => (2, 1),
        Op::INeg | Op::FNeg | Op::Not | Op::I2F | Op::F2I | Op::Len | Op::Unbox | Op::GetField(_) => (1, 1),
        Op::Jmp(_) => (0, 0),
        Op::Jz(_) | Op::Jnz(_) => (1, 0),
        Op::JzKeep(_) | Op::JnzKeep(_) => (1, 1),
        Op::Call(f) => (m.funcs[*f as usize].params, 1),
        Op::CallInd(n) => (n + 1, 1),
        Op::Dispatch(_, n) => (*n, 1),
        Op::Ret => (1, 0),
        Op::RetVoid => (0, 0),
        Op::Rt(r) => (r.argc() as u32, 1),
        Op::Host(i) => (m.imports[*i as usize].argc, 1),
        Op::Spawn(_, n, _) => (*n, 1),
        Op::NewRecord(_, n) | Op::NewArray(_, n) => (*n, 1),
        Op::SetField(_) | Op::Index(_) => (2, 1),
        Op::SetIndex(_) => (3, 1),
        Op::IncLocal(..) | Op::JCmpLL(..) | Op::JCmpLC(..) | Op::LoadField(..) | Op::Load2(..) | Op::LoadK(..) => (0, 0),
    }
}

fn check_operands(m: &Module, f: &Function, op: &Op, nf: u32, nt: u32) -> Result<(), String> {
    let local = |s: u32| {
        if s < f.locals {
            Ok(())
        } else {
            Err(format!("local {} does not exist (the function has {})", s, f.locals))
        }
    };
    let global = |g: u32| {
        if (g as usize) < m.globals.len() {
            Ok(())
        } else {
            Err(format!("global @{} does not exist", g))
        }
    };
    let func = |i: u32| {
        if i < nf {
            Ok(())
        } else {
            Err(format!("function @{} does not exist", i))
        }
    };
    let ty = |t: u32| {
        if t < nt {
            Ok(())
        } else {
            Err(format!("type #{} does not exist", t))
        }
    };
    match *op {
        Op::Str(i) if i as usize >= m.strings.len() => Err(format!("string @{} does not exist", i)),
        Op::FuncRef(i) | Op::Call(i) => func(i),
        Op::Load(s) | Op::Store(s) | Op::Tee(s) => local(s),
        Op::GLoad(g) | Op::GStore(g) | Op::GTee(g) => global(g),
        Op::Dispatch(t, n) => match m.tables.get(t as usize) {
            None => Err(format!("table @{} does not exist", t)),
            Some(tab) if tab.argc != n => Err(format!("table {} takes {} arguments, not {}", tab.name, tab.argc, n)),
            _ => Ok(()),
        },
        Op::Rt(r) if RESERVED_RT.contains(&r) => Err(format!("runtime function {} is reserved for native code", rt_name(r))),
        Op::Host(i) if i as usize >= m.imports.len() => Err(format!("import @{} does not exist", i)),
        Op::Spawn(fi, n, t) => {
            func(fi)?;
            ty(t)?;
            let p = m.funcs[fi as usize].params;
            if p != n {
                return Err(format!("spawn passes {} arguments but {} takes {}", n, m.funcs[fi as usize].name, p));
            }
            Ok(())
        }
        Op::NewRecord(t, n) => {
            ty(t)?;
            match &m.types[t as usize] {
                Desc::Record { fields, .. } if fields.len() as u32 == n => Ok(()),
                Desc::Record { fields, .. } => Err(format!("new passes {} fields but the record has {}", n, fields.len())),
                _ => Err(format!("new needs a record type, not #{}", t)),
            }
        }
        Op::NewArray(t, _) => {
            ty(t)?;
            match &m.types[t as usize] {
                Desc::Array(_) => Ok(()),
                _ => Err(format!("newarr needs an array type, not #{}", t)),
            }
        }
        o if o.is_fused() => Err(format!("{} is an internal instruction and cannot appear in a module", o.mnemonic())),
        _ => Ok(()),
    }
}

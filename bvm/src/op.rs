use burn_runtime::RtFn;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Cmp {
    pub const ALL: [Cmp; 6] = [Cmp::Eq, Cmp::Ne, Cmp::Lt, Cmp::Le, Cmp::Gt, Cmp::Ge];

    pub fn suffix(self) -> &'static str {
        match self {
            Cmp::Eq => "eq",
            Cmp::Ne => "ne",
            Cmp::Lt => "lt",
            Cmp::Le => "le",
            Cmp::Gt => "gt",
            Cmp::Ge => "ge",
        }
    }

    pub fn from_suffix(s: &str) -> Option<Cmp> {
        Cmp::ALL.into_iter().find(|c| c.suffix() == s)
    }

    #[inline(always)]
    pub fn int(self, a: i64, b: i64) -> bool {
        match self {
            Cmp::Eq => a == b,
            Cmp::Ne => a != b,
            Cmp::Lt => a < b,
            Cmp::Le => a <= b,
            Cmp::Gt => a > b,
            Cmp::Ge => a >= b,
        }
    }

    #[inline(always)]
    pub fn uint(self, a: u64, b: u64) -> bool {
        match self {
            Cmp::Eq => a == b,
            Cmp::Ne => a != b,
            Cmp::Lt => a < b,
            Cmp::Le => a <= b,
            Cmp::Gt => a > b,
            Cmp::Ge => a >= b,
        }
    }

    #[inline(always)]
    pub fn float(self, a: f64, b: f64) -> bool {
        match self {
            Cmp::Eq => a == b,
            Cmp::Ne => a != b,
            Cmp::Lt => a < b,
            Cmp::Le => a <= b,
            Cmp::Gt => a > b,
            Cmp::Ge => a >= b,
        }
    }
}

pub const NO_LOC: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Const(u64),
    TypeConst(u32),
    LocConst(u32),
    Str(u32),
    FuncRef(u32),
    Load(u32),
    Store(u32),
    Tee(u32),
    GLoad(u32),
    GStore(u32),
    GTee(u32),
    Pop,
    Dup,
    Swap,
    IAdd,
    ISub,
    IMul,
    IDiv(u32),
    IAddOv(u32),
    ISubOv(u32),
    IMulOv(u32),
    INegOv(u32),
    IRem(u32),
    INeg,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    UShr,
    FAdd,
    FSub,
    FMul,
    FDiv,
    FRem,
    FNeg,
    ICmp(Cmp),
    UCmp(Cmp),
    FCmp(Cmp),
    Not,
    I2F,
    F2I,
    Jmp(u32),
    Jz(u32),
    Jnz(u32),
    JzKeep(u32),
    JnzKeep(u32),
    Call(u32),
    CallInd(u32),
    Dispatch(u32, u32),
    Ret,
    RetVoid,
    Rt(RtFn),
    Host(u32),
    Spawn(u32, u32, u32),
    NewRecord(u32, u32),
    GetField(u32),
    SetField(u32),
    NewArray(u32, u32),
    Index(u32),
    SetIndex(u32),
    Len,
    Unbox,
    IncLocal(u32, i32),
    JCmpLL(Cmp, u32, u32, u32),
    JCmpLC(Cmp, u32, i32, u32),
    LoadField(u32, u32),
    Load2(u32, u32),
    LoadK(u32, i32),
    CallSelf,
    LoopJmp(u32),
    IncLocalOv(u32, i32, u32),
}

impl Op {
    pub fn mnemonic(&self) -> &'static str {
        match self {
            Op::Const(_) => "const",
            Op::TypeConst(_) => "tconst",
            Op::LocConst(_) => "lconst",
            Op::Str(_) => "str",
            Op::FuncRef(_) => "fref",
            Op::Load(_) => "load",
            Op::Store(_) => "store",
            Op::Tee(_) => "tee",
            Op::GLoad(_) => "gload",
            Op::GStore(_) => "gstore",
            Op::GTee(_) => "gtee",
            Op::Pop => "pop",
            Op::Dup => "dup",
            Op::Swap => "swap",
            Op::IAdd => "iadd",
            Op::ISub => "isub",
            Op::IMul => "imul",
            Op::IDiv(_) => "idiv",
            Op::IAddOv(_) => "iadd.ovf",
            Op::ISubOv(_) => "isub.ovf",
            Op::IMulOv(_) => "imul.ovf",
            Op::INegOv(_) => "ineg.ovf",
            Op::IncLocalOv(..) => "inc.local.ovf",
            Op::IRem(_) => "irem",
            Op::INeg => "ineg",
            Op::And => "and",
            Op::Or => "or",
            Op::Xor => "xor",
            Op::Shl => "shl",
            Op::Shr => "shr",
            Op::UShr => "ushr",
            Op::FAdd => "fadd",
            Op::FSub => "fsub",
            Op::FMul => "fmul",
            Op::FDiv => "fdiv",
            Op::FRem => "frem",
            Op::FNeg => "fneg",
            Op::ICmp(c) => match c {
                Cmp::Eq => "ieq",
                Cmp::Ne => "ine",
                Cmp::Lt => "ilt",
                Cmp::Le => "ile",
                Cmp::Gt => "igt",
                Cmp::Ge => "ige",
            },
            Op::UCmp(c) => match c {
                Cmp::Eq => "ueq",
                Cmp::Ne => "une",
                Cmp::Lt => "ult",
                Cmp::Le => "ule",
                Cmp::Gt => "ugt",
                Cmp::Ge => "uge",
            },
            Op::FCmp(c) => match c {
                Cmp::Eq => "feq",
                Cmp::Ne => "fne",
                Cmp::Lt => "flt",
                Cmp::Le => "fle",
                Cmp::Gt => "fgt",
                Cmp::Ge => "fge",
            },
            Op::Not => "not",
            Op::I2F => "i2f",
            Op::F2I => "f2i",
            Op::Jmp(_) => "jmp",
            Op::Jz(_) => "jz",
            Op::Jnz(_) => "jnz",
            Op::JzKeep(_) => "jzk",
            Op::JnzKeep(_) => "jnzk",
            Op::Call(_) => "call",
            Op::CallInd(_) => "calli",
            Op::Dispatch(_, _) => "dispatch",
            Op::Ret => "ret",
            Op::RetVoid => "retv",
            Op::Rt(_) => "rt",
            Op::Host(_) => "host",
            Op::Spawn(_, _, _) => "spawn",
            Op::NewRecord(_, _) => "new",
            Op::GetField(_) => "getf",
            Op::SetField(_) => "setf",
            Op::NewArray(_, _) => "newarr",
            Op::Index(_) => "index",
            Op::SetIndex(_) => "setindex",
            Op::Len => "len",
            Op::Unbox => "unbox",
            Op::IncLocal(_, _) => "inc.local",
            Op::JCmpLL(_, _, _, _) => "jcmp.ll",
            Op::JCmpLC(_, _, _, _) => "jcmp.lc",
            Op::LoadField(_, _) => "load.field",
            Op::Load2(_, _) => "load2",
            Op::LoadK(_, _) => "load.const",
            Op::CallSelf => "call.self",
            Op::LoopJmp(_) => "jmp.loop",
        }
    }

    pub fn is_fused(&self) -> bool {
        matches!(
            self,
            Op::IncLocal(..)
                | Op::JCmpLL(..)
                | Op::JCmpLC(..)
                | Op::LoadField(..)
                | Op::Load2(..)
                | Op::LoadK(..)
                | Op::CallSelf
                | Op::LoopJmp(_)
                | Op::IncLocalOv(..)
        )
    }

    pub fn is_terminator(&self) -> bool {
        matches!(self, Op::Jmp(_) | Op::LoopJmp(_) | Op::Ret | Op::RetVoid)
    }

    pub fn jump_target(&self) -> Option<u32> {
        match self {
            Op::Jmp(t) | Op::LoopJmp(t) | Op::Jz(t) | Op::Jnz(t) | Op::JzKeep(t) | Op::JnzKeep(t) => Some(*t),
            _ => None,
        }
    }

    pub fn with_jump_target(self, t: u32) -> Op {
        match self {
            Op::Jmp(_) => Op::Jmp(t),
            Op::LoopJmp(_) => Op::LoopJmp(t),
            Op::Jz(_) => Op::Jz(t),
            Op::Jnz(_) => Op::Jnz(t),
            Op::JzKeep(_) => Op::JzKeep(t),
            Op::JnzKeep(_) => Op::JnzKeep(t),
            o => o,
        }
    }

    pub fn simple(name: &str) -> Option<Op> {
        Some(match name {
            "pop" => Op::Pop,
            "dup" => Op::Dup,
            "swap" => Op::Swap,
            "iadd" => Op::IAdd,
            "isub" => Op::ISub,
            "imul" => Op::IMul,
            "ineg" => Op::INeg,
            "and" => Op::And,
            "or" => Op::Or,
            "xor" => Op::Xor,
            "shl" => Op::Shl,
            "shr" => Op::Shr,
            "ushr" => Op::UShr,
            "fadd" => Op::FAdd,
            "fsub" => Op::FSub,
            "fmul" => Op::FMul,
            "fdiv" => Op::FDiv,
            "frem" => Op::FRem,
            "fneg" => Op::FNeg,
            "not" => Op::Not,
            "i2f" => Op::I2F,
            "f2i" => Op::F2I,
            "ret" => Op::Ret,
            "retv" => Op::RetVoid,
            "len" => Op::Len,
            "unbox" => Op::Unbox,
            _ => {
                if name.len() == 3 {
                    let c = Cmp::from_suffix(&name[1..])?;
                    return match &name[..1] {
                        "i" => Some(Op::ICmp(c)),
                        "u" => Some(Op::UCmp(c)),
                        "f" => Some(Op::FCmp(c)),
                        _ => None,
                    };
                }
                return None;
            }
        })
    }
}

pub fn rt_name(f: RtFn) -> &'static str {
    let s = f.symbol();
    s.strip_prefix("burn_").unwrap_or(s)
}

pub fn rt_by_name(name: &str) -> Option<RtFn> {
    RtFn::all().iter().copied().find(|f| rt_name(*f) == name)
}

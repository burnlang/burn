use crate::module::Module;
use crate::op::{Op, NO_LOC};
use crate::verify::{analyze, VerifyError};
use burn_runtime::obj::*;
use burn_runtime::{api, gc, io, meta, task};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

const MAX_FRAMES: usize = 1_000_000;

pub type HostFn = Arc<dyn Fn(&[u64]) -> u64 + Send + Sync>;

#[derive(Default, Clone)]
pub struct Host {
    fns: HashMap<String, (u32, HostFn)>,
}

impl Host {
    pub fn new() -> Host {
        Host::default()
    }

    pub fn register(&mut self, name: &str, argc: u32, f: impl Fn(&[u64]) -> u64 + Send + Sync + 'static) -> &mut Host {
        self.fns.insert(name.to_string(), (argc, Arc::new(f)));
        self
    }

    pub fn extend(&mut self, other: &Host) {
        for (k, v) in &other.fns {
            self.fns.insert(k.clone(), v.clone());
        }
    }

    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.fns.keys().cloned().collect();
        v.sort();
        v
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    Verify(VerifyError),
    MissingImport(String),
    Unlinked(String),
    Link(String),
    ImportArity { name: String, module: u32, host: u32 },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadError::Verify(e) => write!(f, "{}", e),
            LoadError::MissingImport(n) => write!(f, "the module imports {}, but the host does not provide it", n),
            LoadError::Unlinked(n) => write!(f, "function {} is external; link the module that defines it", n),
            LoadError::Link(e) => write!(f, "{}", e),
            LoadError::ImportArity { name, module, host } => {
                write!(f, "the module imports {} with {} arguments, but the host function takes {}", name, module, host)
            }
        }
    }
}

impl std::error::Error for LoadError {}

impl From<VerifyError> for LoadError {
    fn from(e: VerifyError) -> Self {
        LoadError::Verify(e)
    }
}

#[derive(Clone, Debug)]
pub struct FuncInfo {
    pub name: String,
    pub entry: u32,
    pub params: u32,
    pub locals: u32,
    pub frame: u32,
}

pub struct Program {
    pub ops: Vec<Op>,
    pub funcs: Vec<FuncInfo>,
    pub entry: Option<u32>,
    pub nglobals: usize,
    strings: Vec<u64>,
    tables: Vec<Vec<u32>>,
    hosts: Vec<(u32, HostFn)>,
}

impl Program {
    pub fn func(&self, name: &str) -> Option<u32> {
        self.funcs.iter().position(|f| f.name == name).map(|i| i as u32)
    }
}

pub fn needs_link(m: &Module) -> bool {
    m.annotations.iter().any(|a| crate::mixin::KINDS.contains(&a.name.as_str()))
        || m.imports
            .iter()
            .any(|i| (0..m.funcs.len() as u32).any(|f| crate::link::export_name(m, f).as_deref() == Some(&i.name)))
}

pub fn load(m: &Module, host: &Host) -> Result<Arc<Program>, LoadError> {
    load_with(m, host, true)
}

pub fn load_with(m: &Module, host: &Host, install_meta: bool) -> Result<Arc<Program>, LoadError> {
    let linked;
    let m = if needs_link(m) {
        analyze(m)?;
        linked = crate::link::link(std::slice::from_ref(m)).map_err(LoadError::Link)?;
        &linked
    } else {
        m
    };
    let max = analyze(m)?;
    if let Some(f) = m.funcs.iter().find(|f| f.external) {
        return Err(LoadError::Unlinked(f.name.clone()));
    }
    let mut hosts = Vec::with_capacity(m.imports.len());
    for imp in &m.imports {
        match host.fns.get(&imp.name) {
            None => return Err(LoadError::MissingImport(imp.name.clone())),
            Some((argc, _)) if *argc != imp.argc => {
                return Err(LoadError::ImportArity {
                    name: imp.name.clone(),
                    module: imp.argc,
                    host: *argc,
                })
            }
            Some((argc, f)) => hosts.push((*argc, f.clone())),
        }
    }
    if install_meta {
        meta::set_meta(m.meta());
    }
    Ok(Arc::new(link(m, &max, hosts)))
}

fn link(m: &Module, max: &[u32], hosts: Vec<(u32, HostFn)>) -> Program {
    let mut ops = Vec::with_capacity(m.code_size());
    let mut funcs = Vec::with_capacity(m.funcs.len());
    for (f, max) in m.funcs.iter().zip(max) {
        let entry = ops.len() as u32;
        ops.extend(f.code.iter().map(|op| match op.jump_target() {
            Some(t) => op.with_jump_target(entry + t),
            None => *op,
        }));
        funcs.push(FuncInfo {
            name: f.name.clone(),
            entry,
            params: f.params,
            locals: f.locals,
            frame: f.locals + max + 1,
        });
    }
    fuse(&mut ops);
    let ntypes = m.types.len();
    let tables = m
        .tables
        .iter()
        .map(|t| {
            let mut v = vec![u32::MAX; ntypes];
            for (tid, f) in &t.entries {
                v[*tid as usize] = *f;
            }
            v
        })
        .collect();
    let strings = m.strings.iter().map(|s| str_static(s.as_bytes())).collect();
    Program {
        ops,
        funcs,
        entry: m.entry,
        nglobals: m.globals.len(),
        strings,
        tables,
        hosts,
    }
}

pub fn fuse(ops: &mut [Op]) {
    let n = ops.len();
    let mut targets = vec![false; n + 1];
    for op in ops.iter() {
        if let Some(t) = op.jump_target() {
            targets[t as usize] = true;
        }
    }
    let free = |i: usize, k: usize| (i + 1..i + k).all(|j| !targets[j]);
    let small = |k: u64| (k as i64) >= i32::MIN as i64 && (k as i64) <= i32::MAX as i64;
    let mut i = 0;
    while i + 3 < n {
        match (ops[i], ops[i + 1], ops[i + 2], ops[i + 3]) {
            (Op::Load(a), Op::Load(b), Op::ICmp(c), Op::Jz(t)) if free(i, 4) => {
                ops[i] = Op::JCmpLL(c, a, b, t);
                i += 4;
                continue;
            }
            (Op::Load(a), Op::Const(k), Op::ICmp(c), Op::Jz(t)) if free(i, 4) && small(k) => {
                ops[i] = Op::JCmpLC(c, a, k as i64 as i32, t);
                i += 4;
                continue;
            }
            (Op::Load(a), Op::Const(k), Op::IAdd, Op::Store(b)) if a == b && free(i, 4) && small(k) => {
                ops[i] = Op::IncLocal(a, k as i64 as i32);
                i += 4;
                continue;
            }
            _ => {}
        }
        if free(i, 2) {
            match (ops[i], ops[i + 1]) {
                (Op::Load(a), Op::GetField(f)) => {
                    ops[i] = Op::LoadField(a, f);
                    i += 2;
                    continue;
                }
                (Op::Load(a), Op::Load(b)) => {
                    ops[i] = Op::Load2(a, b);
                    i += 2;
                    continue;
                }
                (Op::Load(a), Op::Const(k)) if small(k) => {
                    ops[i] = Op::LoadK(a, k as i64 as i32);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        i += 1;
    }
}

#[derive(Clone, Copy)]
struct Frame {
    pc: u32,
    base: u32,
}

pub struct Vm {
    pub stack: Vec<u64>,
    frames: Vec<Frame>,
    prog: Arc<Program>,
    globals: *mut u64,
}

#[derive(Clone, Copy)]
struct SendPtr(*mut u64);
unsafe impl Send for SendPtr {}

#[inline(always)]
fn f(v: u64) -> f64 {
    f64::from_bits(v)
}

pub fn float_to_int(x: f64) -> i64 {
    if x.is_nan() || !(-9.223_372_036_854_776e18..9.223_372_036_854_776e18).contains(&x) {
        i64::MIN
    } else {
        x as i64
    }
}

#[cold]
#[inline(never)]
fn overflow() -> ! {
    io::rt_error("stack overflow (recursion is too deep)", u64::MAX)
}

#[inline(always)]
fn record_ok(o: u64, i: u32) -> bool {
    o != 0 && kind_of(o) == K_STRUCT && (i as usize) < struct_len(o)
}

#[cold]
#[inline(never)]
fn bad_record(o: u64, i: u32) -> ! {
    if o == 0 {
        api::err_null(NO_LOC as u64);
    }
    io::rt_error(
        &format!("field {} does not exist on a value of type {}", i, meta::type_name(tid_of(o))),
        u64::MAX,
    )
}

#[inline(always)]
fn array_ok(a: u64) -> bool {
    a != 0 && kind_of(a) == K_ARRAY
}

#[cold]
#[inline(never)]
fn bad_array(a: u64, loc: u32) -> ! {
    if a == 0 {
        api::err_null(loc as u64);
    }
    io::rt_error(&format!("a value of type {} is not an array", meta::type_name(tid_of(a))), loc as u64)
}

#[cold]
#[inline(never)]
fn not_implemented(tid: u32) -> ! {
    io::rt_error(&format!("value of type {} does not implement this interface", meta::type_name(tid)), u64::MAX)
}

#[cold]
#[inline(never)]
fn bad_call(prog: &Program, func: u64, argc: u32) -> ! {
    match prog.funcs.get(func as usize) {
        None => io::rt_error(&format!("{} is not a function", func as i64), u64::MAX),
        Some(fi) => io::rt_error(&format!("{} takes {} arguments but was called with {}", fi.name, fi.params, argc), u64::MAX),
    }
}

impl Vm {
    pub fn new(prog: Arc<Program>, globals: *mut u64) -> Box<Vm> {
        Box::new(Vm {
            stack: Vec::with_capacity(1 << 14),
            frames: Vec::with_capacity(256),
            prog,
            globals,
        })
    }

    pub fn call(&mut self, func: u32, args: &[u64]) -> u64 {
        let fi = &self.prog.funcs[func as usize];
        assert_eq!(fi.params as usize, args.len(), "{} takes {} arguments", fi.name, fi.params);
        self.stack.extend_from_slice(args);
        self.run(func)
    }

    fn spawn(&self, func: u32, args: Vec<u64>, tid: u32) -> u64 {
        let prog = self.prog.clone();
        let g = SendPtr(self.globals);
        task::spawn(
            tid,
            Box::new(move || {
                let g = g;
                let mut vm = Vm::new(prog, g.0);
                vm.call(func, &args)
            }),
        )
    }

    #[cold]
    #[inline(never)]
    fn grow(&mut self, used: usize, need: usize) -> *mut u64 {
        unsafe { self.stack.set_len(used) };
        self.stack.reserve((need - used).max(self.stack.capacity()));
        self.stack.as_mut_ptr()
    }

    fn run(&mut self, func: u32) -> u64 {
        let prog = self.prog.clone();
        let ops: &[Op] = &prog.ops;
        let funcs = &prog.funcs;
        let stop = self.frames.len();
        let globals = self.globals;
        let mut s0 = self.stack.as_mut_ptr();
        let mut cap = self.stack.capacity();
        let fi = &funcs[func as usize];
        let start = self.stack.len() - fi.params as usize;
        if start + fi.frame as usize > cap {
            s0 = self.grow(start + fi.params as usize, start + fi.frame as usize);
            cap = self.stack.capacity();
        }
        let mut bp = unsafe { s0.add(start) };
        let mut sp = unsafe {
            std::ptr::write_bytes(bp.add(fi.params as usize), 0, (fi.locals - fi.params) as usize);
            bp.add(fi.locals as usize)
        };
        let mut pc = fi.entry as usize;
        macro_rules! push {
            ($v:expr) => {{
                let v = $v;
                unsafe {
                    *sp = v;
                    sp = sp.add(1);
                }
            }};
        }
        macro_rules! pop {
            () => {
                unsafe {
                    sp = sp.sub(1);
                    *sp
                }
            };
        }
        macro_rules! top {
            () => {
                unsafe { &mut *sp.sub(1) }
            };
        }
        macro_rules! local {
            ($s:expr) => {
                unsafe { *bp.add($s as usize) }
            };
        }
        macro_rules! set_local {
            ($s:expr, $v:expr) => {
                unsafe { *bp.add($s as usize) = $v }
            };
        }
        macro_rules! sync {
            () => {
                unsafe { self.stack.set_len(sp.offset_from(s0) as usize) }
            };
        }
        macro_rules! binop {
            (|$a:ident, $b:ident| $e:expr) => {{
                let $b = pop!();
                let t = top!();
                let $a = *t;
                *t = $e;
            }};
        }
        macro_rules! enter {
            ($func:expr) => {{
                if self.frames.len() > MAX_FRAMES {
                    sync!();
                    overflow();
                }
                self.frames.push(Frame {
                    pc: pc as u32,
                    base: unsafe { bp.offset_from(s0) } as u32,
                });
                let fi = unsafe { funcs.get_unchecked($func as usize) };
                let mut at = unsafe { sp.offset_from(s0) } as usize - fi.params as usize;
                if at + fi.frame as usize > cap {
                    s0 = self.grow(at + fi.params as usize, at + fi.frame as usize);
                    cap = self.stack.capacity();
                    at = at.min(cap);
                }
                bp = unsafe { s0.add(at) };
                unsafe { std::ptr::write_bytes(bp.add(fi.params as usize), 0, (fi.locals - fi.params) as usize) };
                sp = unsafe { bp.add(fi.locals as usize) };
                pc = fi.entry as usize;
            }};
        }
        macro_rules! ret {
            ($v:expr) => {{
                let v = $v;
                sp = bp;
                if self.frames.len() == stop {
                    sync!();
                    return v;
                }
                let fr = unsafe { self.frames.pop().unwrap_unchecked() };
                push!(v);
                pc = fr.pc as usize;
                bp = unsafe { s0.add(fr.base as usize) };
            }};
        }
        loop {
            let op = unsafe { *ops.get_unchecked(pc) };
            pc += 1;
            match op {
                Op::Const(v) => push!(v),
                Op::TypeConst(v) | Op::LocConst(v) => push!(v as u64),
                Op::Str(i) => push!(unsafe { *prog.strings.get_unchecked(i as usize) }),
                Op::FuncRef(i) => push!(i as u64),
                Op::Load(s) => push!(local!(s)),
                Op::Store(s) => {
                    let v = pop!();
                    set_local!(s, v)
                }
                Op::Tee(s) => {
                    let v = *top!();
                    set_local!(s, v)
                }
                Op::GLoad(g) => push!(unsafe { *globals.add(g as usize) }),
                Op::GStore(g) => {
                    let v = pop!();
                    unsafe { *globals.add(g as usize) = v }
                }
                Op::GTee(g) => {
                    let v = *top!();
                    unsafe { *globals.add(g as usize) = v }
                }
                Op::Pop => sp = unsafe { sp.sub(1) },
                Op::Dup => {
                    let v = *top!();
                    push!(v)
                }
                Op::Swap => unsafe { std::ptr::swap(sp.sub(1), sp.sub(2)) },
                Op::IAdd => binop!(|a, b| (a as i64).wrapping_add(b as i64) as u64),
                Op::ISub => binop!(|a, b| (a as i64).wrapping_sub(b as i64) as u64),
                Op::IMul => binop!(|a, b| (a as i64).wrapping_mul(b as i64) as u64),
                Op::IDiv(l) => {
                    let b = pop!() as i64;
                    if b == 0 {
                        sync!();
                        api::err_divzero(l as u64);
                    }
                    let t = top!();
                    *t = (*t as i64).wrapping_div(b) as u64
                }
                Op::IRem(l) => {
                    let b = pop!() as i64;
                    if b == 0 {
                        sync!();
                        api::err_divzero(l as u64);
                    }
                    let t = top!();
                    *t = (*t as i64).wrapping_rem(b) as u64
                }
                Op::INeg => {
                    let t = top!();
                    *t = (*t as i64).wrapping_neg() as u64
                }
                Op::And => binop!(|a, b| a & b),
                Op::Or => binop!(|a, b| a | b),
                Op::Xor => binop!(|a, b| a ^ b),
                Op::Shl => binop!(|a, b| a.wrapping_shl(b as u32)),
                Op::Shr => binop!(|a, b| (a as i64).wrapping_shr(b as u32) as u64),
                Op::UShr => binop!(|a, b| a.wrapping_shr(b as u32)),
                Op::FAdd => binop!(|a, b| (f(a) + f(b)).to_bits()),
                Op::FSub => binop!(|a, b| (f(a) - f(b)).to_bits()),
                Op::FMul => binop!(|a, b| (f(a) * f(b)).to_bits()),
                Op::FDiv => binop!(|a, b| (f(a) / f(b)).to_bits()),
                Op::FRem => binop!(|a, b| (f(a) % f(b)).to_bits()),
                Op::FNeg => {
                    let t = top!();
                    *t = (-f(*t)).to_bits()
                }
                Op::ICmp(c) => binop!(|a, b| c.int(a as i64, b as i64) as u64),
                Op::UCmp(c) => binop!(|a, b| c.uint(a, b) as u64),
                Op::FCmp(c) => binop!(|a, b| c.float(f(a), f(b)) as u64),
                Op::Not => {
                    let t = top!();
                    *t = (*t == 0) as u64
                }
                Op::I2F => {
                    let t = top!();
                    *t = ((*t as i64) as f64).to_bits()
                }
                Op::F2I => {
                    let t = top!();
                    *t = float_to_int(f(*t)) as u64
                }
                Op::Jmp(t) => pc = t as usize,
                Op::Jz(t) => {
                    if pop!() == 0 {
                        pc = t as usize
                    }
                }
                Op::Jnz(t) => {
                    if pop!() != 0 {
                        pc = t as usize
                    }
                }
                Op::JzKeep(t) => {
                    if *top!() == 0 {
                        pc = t as usize
                    }
                }
                Op::JnzKeep(t) => {
                    if *top!() != 0 {
                        pc = t as usize
                    }
                }
                Op::JCmpLL(c, a, b, t) => {
                    if c.int(local!(a) as i64, local!(b) as i64) {
                        pc += 3
                    } else {
                        pc = t as usize
                    }
                }
                Op::JCmpLC(c, a, k, t) => {
                    if c.int(local!(a) as i64, k as i64) {
                        pc += 3
                    } else {
                        pc = t as usize
                    }
                }
                Op::IncLocal(a, k) => {
                    let v = local!(a);
                    set_local!(a, (v as i64).wrapping_add(k as i64) as u64);
                    pc += 3
                }
                Op::LoadField(a, i) => {
                    let o = local!(a);
                    if !record_ok(o, i) {
                        sync!();
                        bad_record(o, i);
                    }
                    push!(field(o, i as usize));
                    pc += 1
                }
                Op::Load2(a, b) => {
                    push!(local!(a));
                    push!(local!(b));
                    pc += 1
                }
                Op::LoadK(a, k) => {
                    push!(local!(a));
                    push!(k as i64 as u64);
                    pc += 1
                }
                Op::Call(func) => enter!(func),
                Op::CallInd(argc) => {
                    let func = pop!();
                    match funcs.get(func as usize) {
                        Some(fi) if fi.params == argc => {}
                        _ => {
                            sync!();
                            bad_call(&prog, func, argc)
                        }
                    }
                    enter!(func)
                }
                Op::Dispatch(slot, argc) => {
                    let recv = unsafe { *sp.sub(argc as usize) };
                    if recv == 0 {
                        sync!();
                        api::err_null(u64::MAX);
                    }
                    let tid = tid_of(recv) as usize;
                    let func = unsafe { prog.tables.get_unchecked(slot as usize) }.get(tid).copied().unwrap_or(u32::MAX);
                    if func == u32::MAX {
                        sync!();
                        not_implemented(tid as u32);
                    }
                    enter!(func)
                }
                Op::Ret => {
                    let v = pop!();
                    ret!(v)
                }
                Op::RetVoid => ret!(0),
                Op::Rt(rf) => {
                    let n = rf.argc();
                    sync!();
                    let r = rf.call(unsafe { std::slice::from_raw_parts(sp.sub(n), n) });
                    sp = unsafe { sp.sub(n) };
                    push!(r);
                }
                Op::Host(i) => {
                    let (n, h) = unsafe { prog.hosts.get_unchecked(i as usize) };
                    let n = *n as usize;
                    sync!();
                    let r = h(unsafe { std::slice::from_raw_parts(sp.sub(n), n) });
                    sp = unsafe { sp.sub(n) };
                    push!(r);
                }
                Op::Spawn(func, argc, tid) => {
                    let n = argc as usize;
                    sync!();
                    let args = unsafe { std::slice::from_raw_parts(sp.sub(n), n) }.to_vec();
                    let fut = self.spawn(func, args, tid);
                    sp = unsafe { sp.sub(n) };
                    push!(fut);
                }
                Op::NewRecord(t, n) => {
                    sync!();
                    let p = struct_new(t, n as usize);
                    sp = unsafe { sp.sub(n as usize) };
                    for i in 0..n as usize {
                        set_field(p, i, unsafe { *sp.add(i) });
                    }
                    push!(p);
                }
                Op::GetField(i) => {
                    let t = top!();
                    let o = *t;
                    if !record_ok(o, i) {
                        sync!();
                        bad_record(o, i);
                    }
                    *t = field(o, i as usize)
                }
                Op::SetField(i) => {
                    let v = pop!();
                    let o = pop!();
                    if !record_ok(o, i) {
                        sync!();
                        bad_record(o, i);
                    }
                    set_field(o, i as usize, v);
                    push!(v);
                }
                Op::NewArray(t, n) => {
                    sync!();
                    let a = array_new(t, n as usize);
                    let data = array_data(a);
                    sp = unsafe { sp.sub(n as usize) };
                    unsafe { std::ptr::copy_nonoverlapping(sp, data, n as usize) };
                    push!(a);
                }
                Op::Index(l) => {
                    let i = pop!();
                    let t = top!();
                    let a = *t;
                    if !array_ok(a) {
                        sync!();
                        bad_array(a, l);
                    }
                    let n = array_len(a);
                    if (i as usize) >= n {
                        sync!();
                        api::err_index(l as u64, i, n as u64);
                    }
                    *t = unsafe { *array_data(a).add(i as usize) };
                }
                Op::SetIndex(l) => {
                    let v = pop!();
                    let i = pop!();
                    let a = pop!();
                    if !array_ok(a) {
                        sync!();
                        bad_array(a, l);
                    }
                    let n = array_len(a);
                    if (i as usize) >= n {
                        sync!();
                        api::err_index(l as u64, i, n as u64);
                    }
                    unsafe { *array_data(a).add(i as usize) = v }
                    push!(v);
                }
                Op::Len => {
                    let t = top!();
                    if !array_ok(*t) {
                        sync!();
                        bad_array(*t, NO_LOC);
                    }
                    *t = array_len(*t) as u64
                }
                Op::Unbox => {
                    let t = top!();
                    if *t == 0 {
                        sync!();
                        api::err_null(u64::MAX);
                    }
                    *t = box_val(*t)
                }
            }
        }
    }
}

#[inline(never)]
fn with_stack_base<R>(f: impl FnOnce() -> R) -> R {
    let marker = [0u64; 2];
    gc::set_stack_base(marker.as_ptr() as usize + 16);
    let r = f();
    std::hint::black_box(&marker);
    r
}

pub struct Runner {
    pub globals: Vec<u64>,
    prog: Arc<Program>,
    vm: Box<Vm>,
}

impl Runner {
    pub fn new(prog: Arc<Program>, globals: Vec<u64>) -> Runner {
        let mut globals = globals;
        if globals.len() < prog.nglobals.max(1) {
            globals.resize(prog.nglobals.max(1), 0);
        }
        let ptr = globals.as_mut_ptr();
        gc::clear_root_ranges();
        gc::add_root_range(ptr as usize, globals.len());
        let vm = Vm::new(prog.clone(), ptr);
        gc::add_vm_stack(&vm.stack as *const Vec<u64>);
        Runner { globals, prog, vm }
    }

    pub fn program(&self) -> &Program {
        &self.prog
    }

    pub fn call(&mut self, func: u32) -> u64 {
        self.call_with(func, &[])
    }

    pub fn call_with(&mut self, func: u32, args: &[u64]) -> u64 {
        let vm = &mut self.vm;
        with_stack_base(|| vm.call(func, args))
    }

    pub fn finish(self) -> Vec<u64> {
        task::wait_all();
        io::flush();
        gc::remove_vm_stack(&self.vm.stack as *const Vec<u64>);
        gc::clear_root_ranges();
        gc::clear_stack_base();
        self.globals
    }
}

pub fn run(m: &Module, host: &Host, args: Vec<String>) -> Result<i32, LoadError> {
    io::set_args(args);
    let prog = load(m, host)?;
    let mut r = Runner::new(prog.clone(), Vec::new());
    if let Some(e) = prog.entry {
        r.call(e);
    }
    r.finish();
    Ok(0)
}

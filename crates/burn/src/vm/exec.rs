use super::compile::{compile, Code, Op};
use crate::hir::{Cmp, Program};
use burn_runtime::obj::*;
use burn_runtime::{api, gc, io, meta, task};
use std::sync::Arc;

const MAX_FRAMES: usize = 1_000_000;

#[derive(Clone, Copy)]
struct Frame {
    pc: u32,
    base: u32,
}

pub struct Vm {
    pub stack: Vec<u64>,
    frames: Vec<Frame>,
    code: Arc<Code>,
    globals: *mut u64,
}

#[derive(Clone, Copy)]
struct SendPtr(*mut u64);
unsafe impl Send for SendPtr {}

#[inline(always)]
fn icmp(c: Cmp, a: i64, b: i64) -> bool {
    match c {
        Cmp::Eq => a == b,
        Cmp::Ne => a != b,
        Cmp::Lt => a < b,
        Cmp::Le => a <= b,
        Cmp::Gt => a > b,
        Cmp::Ge => a >= b,
    }
}

#[inline(always)]
fn fcmp(c: Cmp, a: f64, b: f64) -> bool {
    match c {
        Cmp::Eq => a == b,
        Cmp::Ne => a != b,
        Cmp::Lt => a < b,
        Cmp::Le => a <= b,
        Cmp::Gt => a > b,
        Cmp::Ge => a >= b,
    }
}

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

impl Vm {
    pub fn new(code: Arc<Code>, globals: *mut u64) -> Box<Vm> {
        Box::new(Vm {
            stack: Vec::with_capacity(1 << 14),
            frames: Vec::with_capacity(256),
            code,
            globals,
        })
    }

    pub fn call(&mut self, func: u32, args: &[u64]) -> u64 {
        self.stack.extend_from_slice(args);
        self.run(func)
    }

    fn spawn(&self, func: u32, args: Vec<u64>, tid: u32) -> u64 {
        let code = self.code.clone();
        let g = SendPtr(self.globals);
        task::spawn(
            tid,
            Box::new(move || {
                let g = g;
                let mut vm = Vm::new(code, g.0);
                vm.call(func, &args)
            }),
        )
    }

    fn run(&mut self, func: u32) -> u64 {
        let code = self.code.clone();
        let ops: &[Op] = &code.ops;
        let funcs = &code.funcs;
        let stop = self.frames.len();
        let fi = &funcs[func as usize];
        let mut base = self.stack.len() - fi.params as usize;
        self.stack.resize(base + fi.locals as usize, 0);
        let mut pc = fi.entry as usize;
        let globals = self.globals;
        macro_rules! pop {
            () => {{
                let st = &mut self.stack;
                let n = st.len() - 1;
                let v = unsafe { *st.get_unchecked(n) };
                unsafe { st.set_len(n) };
                v
            }};
        }
        macro_rules! top {
            () => {{
                let st = &mut self.stack;
                let n = st.len() - 1;
                unsafe { st.get_unchecked_mut(n) }
            }};
        }
        macro_rules! local {
            ($s:expr) => {
                unsafe { *self.stack.get_unchecked(base + $s as usize) }
            };
        }
        loop {
            let op = unsafe { *ops.get_unchecked(pc) };
            pc += 1;
            match op {
                Op::Const(v) => self.stack.push(v),
                Op::Load(s) => {
                    let v = local!(s);
                    self.stack.push(v)
                }
                Op::Store(s) => {
                    let v = pop!();
                    unsafe { *self.stack.get_unchecked_mut(base + s as usize) = v }
                }
                Op::Tee(s) => {
                    let v = *top!();
                    unsafe { *self.stack.get_unchecked_mut(base + s as usize) = v }
                }
                Op::GLoad(g) => {
                    let v = unsafe { *globals.add(g as usize) };
                    self.stack.push(v)
                }
                Op::GTee(g) => {
                    let v = *top!();
                    unsafe { *globals.add(g as usize) = v }
                }
                Op::Pop => {
                    pop!();
                }
                Op::IAdd => {
                    let b = pop!();
                    let t = top!();
                    *t = (*t as i64).wrapping_add(b as i64) as u64
                }
                Op::ISub => {
                    let b = pop!();
                    let t = top!();
                    *t = (*t as i64).wrapping_sub(b as i64) as u64
                }
                Op::IMul => {
                    let b = pop!();
                    let t = top!();
                    *t = (*t as i64).wrapping_mul(b as i64) as u64
                }
                Op::IDiv(l) => {
                    let b = pop!() as i64;
                    if b == 0 {
                        api::err_divzero(l as u64);
                    }
                    let t = top!();
                    *t = (*t as i64).wrapping_div(b) as u64
                }
                Op::IMod(l) => {
                    let b = pop!() as i64;
                    if b == 0 {
                        api::err_divzero(l as u64);
                    }
                    let t = top!();
                    *t = (*t as i64).wrapping_rem(b) as u64
                }
                Op::INeg => {
                    let t = top!();
                    *t = (*t as i64).wrapping_neg() as u64
                }
                Op::FAdd => {
                    let b = pop!();
                    let t = top!();
                    *t = (f(*t) + f(b)).to_bits()
                }
                Op::FSub => {
                    let b = pop!();
                    let t = top!();
                    *t = (f(*t) - f(b)).to_bits()
                }
                Op::FMul => {
                    let b = pop!();
                    let t = top!();
                    *t = (f(*t) * f(b)).to_bits()
                }
                Op::FDiv => {
                    let b = pop!();
                    let t = top!();
                    *t = (f(*t) / f(b)).to_bits()
                }
                Op::FNeg => {
                    let t = top!();
                    *t = (-f(*t)).to_bits()
                }
                Op::ICmp(c) => {
                    let b = pop!() as i64;
                    let t = top!();
                    *t = icmp(c, *t as i64, b) as u64
                }
                Op::FCmp(c) => {
                    let b = pop!();
                    let t = top!();
                    *t = fcmp(c, f(*t), f(b)) as u64
                }
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
                    if icmp(c, local!(a) as i64, local!(b) as i64) {
                        pc += 3
                    } else {
                        pc = t as usize
                    }
                }
                Op::JCmpLC(c, a, k, t) => {
                    if icmp(c, local!(a) as i64, k as i64) {
                        pc += 3
                    } else {
                        pc = t as usize
                    }
                }
                Op::IncLocal(a, k) => {
                    let v = local!(a);
                    unsafe { *self.stack.get_unchecked_mut(base + a as usize) = (v as i64).wrapping_add(k as i64) as u64 }
                    pc += 3
                }
                Op::LoadField(a, i) => {
                    let o = local!(a);
                    self.stack.push(field(o, i as usize));
                    pc += 1
                }
                Op::Call(func) => {
                    if self.frames.len() > MAX_FRAMES {
                        io::rt_error("stack overflow (recursion is too deep)", u64::MAX);
                    }
                    self.frames.push(Frame {
                        pc: pc as u32,
                        base: base as u32,
                    });
                    let fi = unsafe { funcs.get_unchecked(func as usize) };
                    base = self.stack.len() - fi.params as usize;
                    self.stack.resize(base + fi.locals as usize, 0);
                    pc = fi.entry as usize;
                }
                Op::CallInd(_) => {
                    let func = pop!();
                    if self.frames.len() > MAX_FRAMES {
                        io::rt_error("stack overflow (recursion is too deep)", u64::MAX);
                    }
                    self.frames.push(Frame {
                        pc: pc as u32,
                        base: base as u32,
                    });
                    let fi = &funcs[func as usize];
                    base = self.stack.len() - fi.params as usize;
                    self.stack.resize(base + fi.locals as usize, 0);
                    pc = fi.entry as usize;
                }
                Op::CallIface(slot, argc) => {
                    let recv = self.stack[self.stack.len() - argc as usize];
                    let tid = tid_of(recv) as usize;
                    let func = code.iface[slot as usize].get(tid).copied().unwrap_or(u32::MAX);
                    if func == u32::MAX {
                        io::rt_error(
                            &format!("value of type {} does not implement this interface", meta::type_name(tid as u32)),
                            u64::MAX,
                        );
                    }
                    if self.frames.len() > MAX_FRAMES {
                        io::rt_error("stack overflow (recursion is too deep)", u64::MAX);
                    }
                    self.frames.push(Frame {
                        pc: pc as u32,
                        base: base as u32,
                    });
                    let fi = &funcs[func as usize];
                    base = self.stack.len() - fi.params as usize;
                    self.stack.resize(base + fi.locals as usize, 0);
                    pc = fi.entry as usize;
                }
                Op::Ret | Op::RetVoid => {
                    let v = if op == Op::Ret { pop!() } else { 0 };
                    self.stack.truncate(base);
                    if self.frames.len() == stop {
                        return v;
                    }
                    let fr = self.frames.pop().unwrap();
                    self.stack.push(v);
                    pc = fr.pc as usize;
                    base = fr.base as usize;
                }
                Op::Rt(rf) => {
                    let n = rf.argc();
                    let len = self.stack.len();
                    let r = rf.call(&self.stack[len - n..]);
                    self.stack.truncate(len - n);
                    self.stack.push(r);
                }
                Op::Spawn(func, argc, tid) => {
                    let len = self.stack.len();
                    let args = self.stack[len - argc as usize..].to_vec();
                    let fut = self.spawn(func, args, tid);
                    self.stack.truncate(len - argc as usize);
                    self.stack.push(fut);
                }
                Op::NewStruct(t, n) => {
                    let p = struct_new(t, n as usize);
                    let len = self.stack.len();
                    for i in 0..n as usize {
                        set_field(p, i, self.stack[len - n as usize + i]);
                    }
                    self.stack.truncate(len - n as usize);
                    self.stack.push(p);
                }
                Op::GetField(i) => {
                    let t = top!();
                    *t = field(*t, i as usize)
                }
                Op::SetField(i) => {
                    let v = pop!();
                    let o = pop!();
                    set_field(o, i as usize, v);
                    self.stack.push(v);
                }
                Op::NewArray(t, n) => {
                    let a = array_new(t, n as usize);
                    let len = self.stack.len();
                    let data = array_data(a);
                    for i in 0..n as usize {
                        unsafe { *data.add(i) = self.stack[len - n as usize + i] }
                    }
                    self.stack.truncate(len - n as usize);
                    self.stack.push(a);
                }
                Op::Index(l) => {
                    let i = pop!();
                    let t = top!();
                    let a = *t;
                    let n = array_len(a);
                    if (i as usize) >= n {
                        api::err_index(l as u64, i, n as u64);
                    }
                    *t = unsafe { *array_data(a).add(i as usize) };
                }
                Op::SetIndex(l) => {
                    let v = pop!();
                    let i = pop!();
                    let a = pop!();
                    let n = array_len(a);
                    if (i as usize) >= n {
                        api::err_index(l as u64, i, n as u64);
                    }
                    unsafe { *array_data(a).add(i as usize) = v }
                    self.stack.push(v);
                }
                Op::ArrLen => {
                    let t = top!();
                    *t = array_len(*t) as u64
                }
                Op::BoxVal => {
                    let t = top!();
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

pub fn prepare(p: &Program) -> Arc<Code> {
    meta::set_meta(p.meta());
    let strings: Vec<u64> = p.strings.iter().map(|s| str_static(s.as_bytes())).collect();
    Arc::new(compile(p, strings))
}

pub struct Runner {
    pub globals: Vec<u64>,
    vm: Box<Vm>,
}

impl Runner {
    pub fn new(code: Arc<Code>, globals: Vec<u64>) -> Runner {
        let mut globals = globals;
        if globals.len() < code.nglobals.max(1) {
            globals.resize(code.nglobals.max(1), 0);
        }
        let ptr = globals.as_mut_ptr();
        gc::clear_root_ranges();
        gc::add_root_range(ptr as usize, globals.len());
        let vm = Vm::new(code.clone(), ptr);
        gc::add_vm_stack(&vm.stack as *const Vec<u64>);
        Runner { globals, vm }
    }

    pub fn call(&mut self, func: u32) -> u64 {
        let vm = &mut self.vm;
        with_stack_base(|| vm.call(func, &[]))
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

pub fn run_program(p: &Program, args: Vec<String>) -> i32 {
    io::set_args(args);
    let code = prepare(p);
    let mut r = Runner::new(code.clone(), Vec::new());
    r.call(code.entry);
    r.finish();
    0
}

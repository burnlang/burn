use crate::hir::{BinOp, Cmp, Conv, Expr, ExprKind, Program, Stmt, UnOp};
use burn_runtime::RtFn;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Const(u64),
    Load(u32),
    Store(u32),
    Tee(u32),
    GLoad(u32),
    GTee(u32),
    Pop,
    IAdd,
    ISub,
    IMul,
    IDiv(u32),
    IMod(u32),
    INeg,
    FAdd,
    FSub,
    FMul,
    FDiv,
    FNeg,
    ICmp(Cmp),
    FCmp(Cmp),
    Not,
    I2F,
    F2I,
    Jmp(u32),
    Jz(u32),
    JzKeep(u32),
    JnzKeep(u32),
    Call(u32),
    CallInd(u32),
    CallIface(u32, u32),
    Ret,
    RetVoid,
    Rt(RtFn),
    Spawn(u32, u32, u32),
    NewStruct(u32, u32),
    GetField(u32),
    SetField(u32),
    NewArray(u32, u32),
    Index(u32),
    SetIndex(u32),
    ArrLen,
    BoxVal,
    IncLocal(u32, i32),
    JCmpLL(Cmp, u32, u32, u32),
    JCmpLC(Cmp, u32, i32, u32),
    LoadField(u32, u32),
}

#[derive(Clone, Debug)]
pub struct FuncCode {
    pub name: String,
    pub entry: u32,
    pub params: u32,
    pub locals: u32,
}

pub struct Code {
    pub ops: Vec<Op>,
    pub funcs: Vec<FuncCode>,
    pub iface: Vec<Vec<u32>>,
    pub entry: u32,
    pub nglobals: usize,
}

struct Loop {
    breaks: Vec<usize>,
    continues: Vec<usize>,
}

pub struct Compiler {
    ops: Vec<Op>,
    loops: Vec<Loop>,
    strings: Vec<u64>,
}

pub fn compile(p: &Program, strings: Vec<u64>) -> Code {
    let mut c = Compiler { ops: Vec::new(), loops: Vec::new(), strings };
    let mut funcs = Vec::with_capacity(p.funcs.len());
    for f in &p.funcs {
        let entry = c.ops.len() as u32;
        for s in &f.body {
            c.stmt(s);
        }
        c.ops.push(Op::RetVoid);
        funcs.push(FuncCode { name: f.name.clone(), entry, params: f.params, locals: f.locals.len().max(f.params as usize) as u32 });
    }
    let ntypes = p.types.len();
    let mut iface = Vec::with_capacity(p.slots.len());
    for s in &p.slots {
        let mut t = vec![u32::MAX; ntypes];
        for (tid, f) in &s.impls {
            t[*tid as usize] = *f;
        }
        iface.push(t);
    }
    let mut ops = c.ops;
    peephole(&mut ops);
    Code { ops, funcs, iface, entry: p.entry, nglobals: p.globals.len() }
}

fn peephole(ops: &mut [Op]) {
    let n = ops.len();
    let mut targets = vec![false; n + 1];
    for op in ops.iter() {
        match op {
            Op::Jmp(t) | Op::Jz(t) | Op::JzKeep(t) | Op::JnzKeep(t) => targets[*t as usize] = true,
            _ => {}
        }
    }
    let mut i = 0;
    while i + 3 < n {
        match (ops[i], ops[i + 1], ops[i + 2], ops[i + 3]) {
            (Op::Load(a), Op::Load(b), Op::ICmp(c), Op::Jz(t)) if !targets[i + 1] && !targets[i + 2] && !targets[i + 3] => {
                ops[i] = Op::JCmpLL(c, a, b, t);
                i += 4;
                continue;
            }
            (Op::Load(a), Op::Const(k), Op::ICmp(c), Op::Jz(t)) if !targets[i + 1] && !targets[i + 2] && !targets[i + 3] && (k as i64) >= i32::MIN as i64 && (k as i64) <= i32::MAX as i64 => {
                ops[i] = Op::JCmpLC(c, a, k as i64 as i32, t);
                i += 4;
                continue;
            }
            (Op::Load(a), Op::Const(k), Op::IAdd, Op::Store(b)) if a == b && !targets[i + 1] && !targets[i + 2] && !targets[i + 3] && (k as i64).abs() < i32::MAX as i64 => {
                ops[i] = Op::IncLocal(a, k as i64 as i32);
                i += 4;
                continue;
            }
            _ => {}
        }
        if let (Op::Load(a), Op::GetField(f)) = (ops[i], ops[i + 1]) {
            if !targets[i + 1] {
                ops[i] = Op::LoadField(a, f);
                i += 2;
                continue;
            }
        }
        i += 1;
    }
}

impl Compiler {
    fn emit(&mut self, op: Op) -> usize {
        self.ops.push(op);
        self.ops.len() - 1
    }

    fn here(&self) -> u32 {
        self.ops.len() as u32
    }

    fn patch(&mut self, at: usize, target: u32) {
        self.ops[at] = match self.ops[at] {
            Op::Jmp(_) => Op::Jmp(target),
            Op::Jz(_) => Op::Jz(target),
            Op::JzKeep(_) => Op::JzKeep(target),
            Op::JnzKeep(_) => Op::JnzKeep(target),
            o => o,
        };
    }

    fn stmts(&mut self, ss: &[Stmt]) {
        for s in ss {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Expr(e) => self.expr_discard(e),
            Stmt::If(c, a, b) => {
                self.expr(c);
                let jz = self.emit(Op::Jz(0));
                self.stmts(a);
                if b.is_empty() {
                    let h = self.here();
                    self.patch(jz, h);
                } else {
                    let j = self.emit(Op::Jmp(0));
                    let h = self.here();
                    self.patch(jz, h);
                    self.stmts(b);
                    let h = self.here();
                    self.patch(j, h);
                }
            }
            Stmt::Loop { cond, body, step } => {
                let top = self.here();
                let exit = cond.as_ref().map(|c| {
                    self.expr(c);
                    self.emit(Op::Jz(0))
                });
                self.loops.push(Loop { breaks: Vec::new(), continues: Vec::new() });
                self.stmts(body);
                let cont = self.here();
                self.stmts(step);
                self.emit(Op::Jmp(top));
                let end = self.here();
                let l = self.loops.pop().unwrap();
                for b in l.breaks {
                    self.patch(b, end);
                }
                for c in l.continues {
                    self.patch(c, cont);
                }
                if let Some(x) = exit {
                    self.patch(x, end);
                }
            }
            Stmt::Return(Some(e)) => {
                self.expr(e);
                self.emit(Op::Ret);
            }
            Stmt::Return(None) => {
                self.emit(Op::RetVoid);
            }
            Stmt::Break => {
                let j = self.emit(Op::Jmp(0));
                self.loops.last_mut().unwrap().breaks.push(j);
            }
            Stmt::Continue => {
                let j = self.emit(Op::Jmp(0));
                self.loops.last_mut().unwrap().continues.push(j);
            }
        }
    }

    fn expr_discard(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::SetLocal(s, v) => {
                self.expr(v);
                self.emit(Op::Store(*s));
            }
            _ => {
                self.expr(e);
                self.emit(Op::Pop);
            }
        }
    }

    fn args(&mut self, xs: &[Expr]) {
        for x in xs {
            self.expr(x);
        }
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Int(v) => {
                self.emit(Op::Const(*v as u64));
            }
            ExprKind::Float(f) => {
                self.emit(Op::Const(f.to_bits()));
            }
            ExprKind::Bool(b) => {
                self.emit(Op::Const(*b as u64));
            }
            ExprKind::Str(i) => {
                let v = self.strings[*i as usize];
                self.emit(Op::Const(v));
            }
            ExprKind::Null => {
                self.emit(Op::Const(0));
            }
            ExprKind::Local(s) => {
                self.emit(Op::Load(*s));
            }
            ExprKind::Global(g) => {
                self.emit(Op::GLoad(*g));
            }
            ExprKind::SetLocal(s, v) => {
                self.expr(v);
                self.emit(Op::Tee(*s));
            }
            ExprKind::SetGlobal(g, v) => {
                self.expr(v);
                self.emit(Op::GTee(*g));
            }
            ExprKind::Unary(op, x) => {
                self.expr(x);
                self.emit(match op {
                    UnOp::INeg => Op::INeg,
                    UnOp::FNeg => Op::FNeg,
                    UnOp::Not => Op::Not,
                });
            }
            ExprKind::Binary(op, a, b) => {
                self.expr(a);
                self.expr(b);
                self.emit(match op {
                    BinOp::IAdd => Op::IAdd,
                    BinOp::ISub => Op::ISub,
                    BinOp::IMul => Op::IMul,
                    BinOp::IDiv(l) => Op::IDiv(*l),
                    BinOp::IMod(l) => Op::IMod(*l),
                    BinOp::FAdd => Op::FAdd,
                    BinOp::FSub => Op::FSub,
                    BinOp::FMul => Op::FMul,
                    BinOp::FDiv => Op::FDiv,
                    BinOp::ICmp(c) => Op::ICmp(*c),
                    BinOp::FCmp(c) => Op::FCmp(*c),
                });
            }
            ExprKind::And(a, b) => {
                self.expr(a);
                let j = self.emit(Op::JzKeep(0));
                self.emit(Op::Pop);
                self.expr(b);
                let h = self.here();
                self.patch(j, h);
            }
            ExprKind::Or(a, b) => {
                self.expr(a);
                let j = self.emit(Op::JnzKeep(0));
                self.emit(Op::Pop);
                self.expr(b);
                let h = self.here();
                self.patch(j, h);
            }
            ExprKind::Conv(c, x) => {
                self.expr(x);
                self.emit(match c {
                    Conv::IntToFloat => Op::I2F,
                    Conv::FloatToInt => Op::F2I,
                });
            }
            ExprKind::Call(f, xs) => {
                self.args(xs);
                self.emit(Op::Call(*f));
            }
            ExprKind::CallIndirect(c, xs) => {
                self.args(xs);
                self.expr(c);
                self.emit(Op::CallInd(xs.len() as u32));
            }
            ExprKind::CallIface(slot, xs) => {
                self.args(xs);
                self.emit(Op::CallIface(*slot, xs.len() as u32));
            }
            ExprKind::Rt(f, xs) => {
                self.args(xs);
                self.emit(Op::Rt(*f));
            }
            ExprKind::Spawn(f, xs) => {
                self.args(xs);
                self.emit(Op::Spawn(*f, xs.len() as u32, e.ty));
            }
            ExprKind::FuncRef(f) => {
                self.emit(Op::Const(*f as u64));
            }
            ExprKind::NewStruct(t, xs) => {
                self.args(xs);
                self.emit(Op::NewStruct(*t, xs.len() as u32));
            }
            ExprKind::GetField(o, i) => {
                self.expr(o);
                self.emit(Op::GetField(*i));
            }
            ExprKind::SetField(o, i, v) => {
                self.expr(o);
                self.expr(v);
                self.emit(Op::SetField(*i));
            }
            ExprKind::NewArray(t, xs) => {
                self.args(xs);
                self.emit(Op::NewArray(*t, xs.len() as u32));
            }
            ExprKind::Index(a, i, l) => {
                self.expr(a);
                self.expr(i);
                self.emit(Op::Index(*l));
            }
            ExprKind::SetIndex(a, i, v, l) => {
                self.expr(a);
                self.expr(i);
                self.expr(v);
                self.emit(Op::SetIndex(*l));
            }
            ExprKind::ArrLen(a) => {
                self.expr(a);
                self.emit(Op::ArrLen);
            }
            ExprKind::BoxVal(x) => {
                self.expr(x);
                self.emit(Op::BoxVal);
            }
            ExprKind::Seq(ss, x) => {
                self.stmts(ss);
                self.expr(x);
            }
        }
    }
}

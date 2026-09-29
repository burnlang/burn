use crate::hir::{BinOp, Const, Conv, Expr, ExprKind, External, Program, Stmt, UnOp};
use bvm::{Annotation, FuncBuilder, Function, Import, Label, Module, Op, Sig, Table, Target, Value};

struct Loop {
    brk: Label,
    cont: Label,
}

struct Compiler {
    f: FuncBuilder,
    loops: Vec<Loop>,
}

pub fn compile(p: &Program) -> Module {
    let mut m = Module::new();
    let meta = p.meta();
    m.types = meta.types;
    m.locs = meta.locs;
    m.strings = p.strings.clone();
    m.globals = p
        .globals
        .iter()
        .map(|g| {
            if g.module.is_empty() {
                g.name.clone()
            } else {
                format!("{}.{}", g.module, g.name)
            }
        })
        .collect();
    m.name = p.name.clone();
    for (i, f) in p.funcs.iter().enumerate() {
        let sig = Some(Sig {
            params: f.locals.iter().take(f.params as usize).copied().collect(),
            ret: f.ret,
        });
        let compiled = match &f.external {
            Some(External::Lib { name, .. }) => Function::external(name, f.params, sig),
            Some(External::Native { name }) => {
                let imp = match m.imports.iter().position(|x| x.name == *name) {
                    Some(k) => k as u32,
                    None => {
                        m.imports.push(Import {
                            name: name.clone(),
                            argc: f.params,
                        });
                        m.imports.len() as u32 - 1
                    }
                };
                let mut b = FuncBuilder::new(f.params);
                for k in 0..f.params {
                    b.load(k);
                }
                b.emit(Op::Host(imp)).ret();
                let mut out = b.finish(&f.name);
                out.sig = sig;
                out
            }
            None => {
                let mut out = func(f);
                out.sig = sig;
                out
            }
        };
        m.funcs.push(compiled);
        for a in &f.annotations {
            m.annotations.push(annotation(Target::Func(i as u32), a));
        }
    }
    for (t, anns) in &p.type_annotations {
        for a in anns {
            m.annotations.push(annotation(Target::Type(*t), a));
        }
    }
    m.tables = p
        .slots
        .iter()
        .map(|s| Table {
            name: s.name.clone(),
            argc: s.argc,
            entries: s.impls.clone(),
        })
        .collect();
    m.entry = Some(p.entry);
    m
}

fn annotation(target: Target, a: &crate::hir::Annotation) -> Annotation {
    Annotation {
        target,
        name: a.name.clone(),
        args: a
            .args
            .iter()
            .filter_map(|(k, v)| {
                let v = match v {
                    Const::Int(x) => Value::Int(*x),
                    Const::Float(x) => Value::Float(*x),
                    Const::Bool(b) => Value::Bool(*b),
                    Const::Str(s) => Value::Str(s.clone()),
                    Const::Null => return None,
                };
                Some((k.clone(), v))
            })
            .collect(),
    }
}

fn func(f: &crate::hir::Func) -> Function {
    let mut c = Compiler {
        f: FuncBuilder::new(f.params),
        loops: Vec::new(),
    };
    c.f.reserve_locals(f.locals.len() as u32);
    c.stmts(&f.body);
    if c.f.reachable_end() {
        c.f.ret_void();
    }
    c.f.finish(&f.name)
}

impl Compiler {
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
                let other = self.f.label();
                self.f.jz(other);
                self.stmts(a);
                if b.is_empty() {
                    self.f.bind(other);
                } else {
                    let end = self.f.label();
                    self.f.jmp(end);
                    self.f.bind(other);
                    self.stmts(b);
                    self.f.bind(end);
                }
            }
            Stmt::Loop { cond, body, step } => {
                let top = self.f.here();
                let brk = self.f.label();
                let cont = self.f.label();
                if let Some(c) = cond {
                    self.expr(c);
                    self.f.jz(brk);
                }
                self.loops.push(Loop { brk, cont });
                self.stmts(body);
                self.f.bind(cont);
                self.stmts(step);
                self.f.jmp(top);
                self.f.bind(brk);
                self.loops.pop();
            }
            Stmt::Return(Some(e)) => {
                self.expr(e);
                self.f.ret();
            }
            Stmt::Return(None) => {
                self.f.ret_void();
            }
            Stmt::Break => {
                let l = self.loops.last().unwrap().brk;
                self.f.jmp(l);
            }
            Stmt::Continue => {
                let l = self.loops.last().unwrap().cont;
                self.f.jmp(l);
            }
        }
    }

    fn expr_discard(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::SetLocal(s, v) => {
                self.expr(v);
                self.f.emit(Op::Store(*s));
            }
            ExprKind::SetGlobal(g, v) => {
                self.expr(v);
                self.f.emit(Op::GStore(*g));
            }
            _ => {
                self.expr(e);
                self.f.emit(Op::Pop);
            }
        }
    }

    fn args(&mut self, xs: &[Expr]) {
        for x in xs {
            self.expr(x);
        }
    }

    fn emit(&mut self, op: Op) {
        self.f.emit(op);
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Int(v) => self.emit(Op::Const(*v as u64)),
            ExprKind::TypeId(t) => self.emit(Op::TypeConst(*t)),
            ExprKind::LocId(l) => self.emit(Op::LocConst(*l)),
            ExprKind::Float(f) => self.emit(Op::Const(f.to_bits())),
            ExprKind::Bool(b) => self.emit(Op::Const(*b as u64)),
            ExprKind::Str(i) => self.emit(Op::Str(*i)),
            ExprKind::Null => self.emit(Op::Const(0)),
            ExprKind::Local(s) => self.emit(Op::Load(*s)),
            ExprKind::Global(g) => self.emit(Op::GLoad(*g)),
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
                    BinOp::IMod(l) => Op::IRem(*l),
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
                let end = self.f.label();
                self.f.jz_keep(end);
                self.emit(Op::Pop);
                self.expr(b);
                self.f.bind(end);
            }
            ExprKind::Or(a, b) => {
                self.expr(a);
                let end = self.f.label();
                self.f.jnz_keep(end);
                self.emit(Op::Pop);
                self.expr(b);
                self.f.bind(end);
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
                self.emit(Op::Dispatch(*slot, xs.len() as u32));
            }
            ExprKind::Rt(f, xs) => {
                self.args(xs);
                self.emit(Op::Rt(*f));
            }
            ExprKind::Spawn(f, xs) => {
                self.args(xs);
                self.emit(Op::Spawn(*f, xs.len() as u32, e.ty));
            }
            ExprKind::FuncRef(f) => self.emit(Op::FuncRef(*f)),
            ExprKind::NewStruct(t, xs) => {
                self.args(xs);
                self.emit(Op::NewRecord(*t, xs.len() as u32));
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
                self.emit(Op::Len);
            }
            ExprKind::BoxVal(x) => {
                self.expr(x);
                self.emit(Op::Unbox);
            }
            ExprKind::Seq(ss, x) => {
                self.stmts(ss);
                self.expr(x);
            }
        }
    }
}

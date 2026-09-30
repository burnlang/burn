use crate::exec::{fuse, Code, Program};
use crate::op::Op;
use std::collections::{HashMap, HashSet};

const INLINE_MAX: usize = 24;
const LOCALS_MAX: u32 = 1024;

#[derive(Default)]
pub struct Gains {
    pub inlined: u32,
    pub folded: u32,
    pub removed: u32,
}

#[derive(Clone, Copy)]
struct Ins {
    op: Op,
    id: u32,
}

fn effect(p: &Program, op: &Op) -> Option<(u32, u32)> {
    Some(match op {
        Op::Const(_) | Op::TypeConst(_) | Op::LocConst(_) | Op::Str(_) | Op::FuncRef(_) | Op::Load(_) | Op::GLoad(_) => (0, 1),
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
        Op::Call(f) => (p.funcs.get(*f as usize)?.params, 1),
        Op::CallInd(n) => (n + 1, 1),
        Op::Dispatch(_, n) => (*n, 1),
        Op::Ret => (1, 0),
        Op::RetVoid => (0, 0),
        Op::Rt(r) => (r.argc() as u32, 1),
        Op::Host(i) => (p.host_argc(*i)?, 1),
        Op::Spawn(_, n, _) => (*n, 1),
        Op::NewRecord(_, n) | Op::NewArray(_, n) => (*n, 1),
        Op::SetField(_) | Op::Index(_) => (2, 1),
        Op::SetIndex(_) => (3, 1),
        _ => return None,
    })
}

fn heights(p: &Program, code: &[Op]) -> Option<(Vec<Option<u32>>, u32)> {
    let n = code.len();
    let mut h: Vec<Option<u32>> = vec![None; n];
    let mut max = 0;
    let mut work = vec![(0usize, 0u32)];
    while let Some((pc, s)) = work.pop() {
        if pc >= n {
            return None;
        }
        match h[pc] {
            Some(old) if old == s => continue,
            Some(_) => return None,
            None => h[pc] = Some(s),
        }
        let op = &code[pc];
        let (need, push) = effect(p, op)?;
        if s < need {
            return None;
        }
        let next = s - need + push;
        max = max.max(next).max(s);
        if let Some(t) = op.jump_target() {
            work.push((t as usize, next));
        }
        if !op.is_terminator() {
            work.push((pc + 1, next));
        }
    }
    Some((h, max))
}

fn inlinable(p: &Program, caller: u32, g: u32) -> bool {
    if g == caller {
        return false;
    }
    let Some(code) = p.raw.get(g as usize) else { return false };
    if code.is_empty() || code.len() > INLINE_MAX {
        return false;
    }
    for (i, op) in code.iter().enumerate() {
        match op {
            Op::Call(_) | Op::CallInd(_) | Op::Dispatch(..) | Op::Spawn(..) => return false,
            _ => {}
        }
        if let Some(t) = op.jump_target() {
            if t as usize <= i {
                return false;
            }
        }
    }
    let Some((h, _)) = heights(p, code) else { return false };
    code.iter().zip(&h).all(|(op, s)| match op {
        Op::Ret => *s == Some(1) || s.is_none(),
        Op::RetVoid => *s == Some(0) || s.is_none(),
        _ => true,
    })
}

fn shift_local(op: Op, base: u32) -> Op {
    match op {
        Op::Load(s) => Op::Load(s + base),
        Op::Store(s) => Op::Store(s + base),
        Op::Tee(s) => Op::Tee(s + base),
        o => o,
    }
}

fn fold_bin(op: &Op, a: u64, b: u64) -> Option<u64> {
    let f = f64::from_bits;
    Some(match op {
        Op::IAdd => (a as i64).wrapping_add(b as i64) as u64,
        Op::ISub => (a as i64).wrapping_sub(b as i64) as u64,
        Op::IMul => (a as i64).wrapping_mul(b as i64) as u64,
        Op::IDiv(_) if b != 0 => (a as i64).wrapping_div(b as i64) as u64,
        Op::IRem(_) if b != 0 => (a as i64).wrapping_rem(b as i64) as u64,
        Op::And => a & b,
        Op::Or => a | b,
        Op::Xor => a ^ b,
        Op::Shl => a.wrapping_shl(b as u32),
        Op::Shr => (a as i64).wrapping_shr(b as u32) as u64,
        Op::UShr => a.wrapping_shr(b as u32),
        Op::FAdd => (f(a) + f(b)).to_bits(),
        Op::FSub => (f(a) - f(b)).to_bits(),
        Op::FMul => (f(a) * f(b)).to_bits(),
        Op::FDiv => (f(a) / f(b)).to_bits(),
        Op::ICmp(c) => c.int(a as i64, b as i64) as u64,
        Op::UCmp(c) => c.uint(a, b) as u64,
        Op::FCmp(c) => c.float(f(a), f(b)) as u64,
        _ => return None,
    })
}

fn fold_un(op: &Op, a: u64) -> Option<u64> {
    Some(match op {
        Op::INeg => (a as i64).wrapping_neg() as u64,
        Op::FNeg => (-f64::from_bits(a)).to_bits(),
        Op::Not => (a == 0) as u64,
        Op::I2F => ((a as i64) as f64).to_bits(),
        _ => return None,
    })
}

fn pure_push(op: &Op) -> bool {
    matches!(
        op,
        Op::Const(_) | Op::TypeConst(_) | Op::LocConst(_) | Op::Str(_) | Op::FuncRef(_) | Op::Load(_) | Op::GLoad(_)
    )
}

struct Ir {
    ins: Vec<Ins>,
    forward: HashMap<u32, u32>,
    next_id: u32,
}

impl Ir {
    fn resolve(&self, mut id: u32) -> u32 {
        let mut guard = 0;
        while let Some(n) = self.forward.get(&id) {
            id = *n;
            guard += 1;
            if guard > 1_000_000 {
                break;
            }
        }
        id
    }

    fn targets(&self) -> HashSet<u32> {
        self.ins.iter().filter_map(|i| i.op.jump_target()).map(|t| self.resolve(t)).collect()
    }

    fn remove(&mut self, idx: usize) {
        let id = self.ins[idx].id;
        let next = self.ins.get(idx + 1).map(|n| n.id);
        self.ins.remove(idx);
        if let Some(n) = next {
            self.forward.insert(id, n);
        }
    }
}

fn peephole(ir: &mut Ir, orig_locals: u32, g: &mut Gains) -> bool {
    let mut changed = false;
    let loaded: HashSet<u32> = ir
        .ins
        .iter()
        .filter_map(|i| match i.op {
            Op::Load(s) => Some(s),
            _ => None,
        })
        .collect();
    for i in 0..ir.ins.len() {
        match ir.ins[i].op {
            Op::Store(s) if s >= orig_locals && !loaded.contains(&s) => {
                ir.ins[i].op = Op::Pop;
                changed = true;
            }
            _ => {}
        }
    }
    let mut i = 0;
    while i < ir.ins.len() {
        if let Op::Tee(s) = ir.ins[i].op {
            if s >= orig_locals && !loaded.contains(&s) {
                ir.remove(i);
                g.removed += 1;
                changed = true;
                continue;
            }
        }
        i += 1;
    }
    let mut i = 0;
    while i < ir.ins.len() {
        let targets = ir.targets();
        let free = |k: usize, ir: &Ir| (1..k).all(|d| ir.ins.get(i + d).map(|x| !targets.contains(&x.id)).unwrap_or(false));
        let a = ir.ins[i].op;
        let b = ir.ins.get(i + 1).map(|x| x.op);
        let c = ir.ins.get(i + 2).map(|x| x.op);
        if let (Op::Const(x), Some(Op::Const(y)), Some(op)) = (a, b, c) {
            if free(3, ir) {
                if let Some(v) = fold_bin(&op, x, y) {
                    ir.ins[i].op = Op::Const(v);
                    ir.remove(i + 1);
                    ir.remove(i + 1);
                    g.folded += 1;
                    changed = true;
                    continue;
                }
            }
        }
        if let (Op::Const(x), Some(op)) = (a, b) {
            if free(2, ir) {
                if let Some(v) = fold_un(&op, x) {
                    ir.ins[i].op = Op::Const(v);
                    ir.remove(i + 1);
                    g.folded += 1;
                    changed = true;
                    continue;
                }
                let branch = match op {
                    Op::Jz(t) => Some((x == 0, t)),
                    Op::Jnz(t) => Some((x != 0, t)),
                    _ => None,
                };
                if let Some((taken, t)) = branch {
                    g.folded += 1;
                    changed = true;
                    if taken {
                        ir.ins[i].op = Op::Jmp(t);
                        ir.remove(i + 1);
                    } else {
                        ir.remove(i);
                        ir.remove(i);
                        g.removed += 1;
                    }
                    continue;
                }
            }
        }
        if let Some(bop) = b {
            if free(2, ir) {
                let pair = match (a, bop) {
                    (Op::Not, Op::Jz(t)) => Some(Some(Op::Jnz(t))),
                    (Op::Not, Op::Jnz(t)) => Some(Some(Op::Jz(t))),
                    (Op::Store(x), Op::Load(y)) if x == y => Some(Some(Op::Tee(x))),
                    (Op::Tee(x), Op::Pop) => Some(Some(Op::Store(x))),
                    (Op::Dup, Op::Pop) => Some(None),
                    (p, Op::Pop) if pure_push(&p) => Some(None),
                    _ => None,
                };
                if let Some(rep) = pair {
                    match rep {
                        Some(op) => {
                            ir.ins[i].op = op;
                            ir.remove(i + 1);
                        }
                        None => {
                            ir.remove(i);
                            ir.remove(i);
                        }
                    }
                    g.removed += 1;
                    changed = true;
                    continue;
                }
            }
        }
        if let Some(t) = a.jump_target() {
            let rt = ir.resolve(t);
            if let Some(pos) = ir.ins.iter().position(|x| x.id == rt) {
                if let Op::Jmp(u) = ir.ins[pos].op {
                    if ir.resolve(u) != rt {
                        ir.ins[i].op = a.with_jump_target(u);
                        changed = true;
                        continue;
                    }
                }
                if matches!(a, Op::Jmp(_)) && pos == i + 1 {
                    ir.remove(i);
                    g.removed += 1;
                    changed = true;
                    continue;
                }
            }
        }
        i += 1;
    }
    changed
}

fn dead_code(ir: &mut Ir, g: &mut Gains) -> bool {
    let n = ir.ins.len();
    if n == 0 {
        return false;
    }
    let pos: HashMap<u32, usize> = ir.ins.iter().enumerate().map(|(i, x)| (x.id, i)).collect();
    let mut live = vec![false; n];
    let mut work = vec![0usize];
    while let Some(i) = work.pop() {
        if i >= n || live[i] {
            continue;
        }
        live[i] = true;
        let op = ir.ins[i].op;
        if let Some(t) = op.jump_target() {
            if let Some(p) = pos.get(&ir.resolve(t)) {
                work.push(*p);
            }
        }
        if !op.is_terminator() {
            work.push(i + 1);
        }
    }
    let before = ir.ins.len();
    let mut k = 0;
    ir.ins.retain(|_| {
        let keep = live[k];
        k += 1;
        keep
    });
    let removed = (before - ir.ins.len()) as u32;
    g.removed += removed;
    removed > 0
}

pub fn optimize(p: &Program, f: u32) -> Option<(Code, Gains)> {
    let raw = p.raw.get(f as usize)?;
    let info = p.funcs.get(f as usize)?;
    if raw.is_empty() {
        return None;
    }
    let mut g = Gains::default();
    let orig_locals = info.locals;
    let mut locals = info.locals;
    let mut ir = Ir {
        ins: Vec::with_capacity(raw.len() * 2),
        forward: HashMap::new(),
        next_id: raw.len() as u32,
    };
    for (i, op) in raw.iter().enumerate() {
        let op = *op;
        let callee = match op {
            Op::Call(c) if locals + p.funcs[c as usize].locals <= LOCALS_MAX && inlinable(p, f, c) => Some(c),
            _ => None,
        };
        let Some(c) = callee else {
            ir.ins.push(Ins { op, id: i as u32 });
            continue;
        };
        let ci = &p.funcs[c as usize];
        let base = locals;
        locals += ci.locals;
        let after = (i + 1) as u32;
        let body = &p.raw[c as usize];
        let map: Vec<u32> = (0..body.len())
            .map(|_| {
                ir.next_id += 1;
                ir.next_id - 1
            })
            .collect();
        let mut out: Vec<(Op, Option<u32>)> = Vec::new();
        for k in (0..ci.params).rev() {
            out.push((Op::Store(base + k), None));
        }
        for k in ci.params..ci.locals {
            out.push((Op::Const(0), None));
            out.push((Op::Store(base + k), None));
        }
        for (j, bop) in body.iter().enumerate() {
            let id = Some(map[j]);
            match *bop {
                Op::Ret => out.push((Op::Jmp(after), id)),
                Op::RetVoid => {
                    out.push((Op::Const(0), id));
                    out.push((Op::Jmp(after), None));
                }
                o => {
                    let o = shift_local(o, base);
                    let o = match o.jump_target() {
                        Some(t) => o.with_jump_target(map[t as usize]),
                        None => o,
                    };
                    out.push((o, id));
                }
            }
        }
        for (k, (op, id)) in out.into_iter().enumerate() {
            let id = if k == 0 {
                if let Some(orig) = id {
                    ir.forward.insert(orig, i as u32);
                }
                i as u32
            } else {
                id.unwrap_or_else(|| {
                    ir.next_id += 1;
                    ir.next_id - 1
                })
            };
            ir.ins.push(Ins { op, id });
        }
        g.inlined += 1;
    }
    let mut rounds = 0;
    loop {
        let a = peephole(&mut ir, orig_locals, &mut g);
        let b = dead_code(&mut ir, &mut g);
        rounds += 1;
        if !(a || b) || rounds > 8 {
            break;
        }
    }
    let pos: HashMap<u32, u32> = ir.ins.iter().enumerate().map(|(i, x)| (x.id, i as u32)).collect();
    let mut ops = Vec::with_capacity(ir.ins.len());
    for x in &ir.ins {
        let op = match x.op.jump_target() {
            Some(t) => x.op.with_jump_target(*pos.get(&ir.resolve(t))?),
            None => x.op,
        };
        ops.push(op);
    }
    let (h, max) = heights(p, &ops)?;
    let (raw_h, _) = heights(p, raw)?;
    let mut osr = Vec::new();
    for (i, op) in raw.iter().enumerate() {
        if let Some(t) = op.jump_target() {
            if (t as usize) <= i && raw_h.get(t as usize) == Some(&Some(0)) {
                if let Some(np) = pos.get(&ir.resolve(t)) {
                    if h.get(*np as usize) == Some(&Some(0)) && !osr.iter().any(|(r, _)| *r == t) {
                        osr.push((t, *np));
                    }
                }
            }
        }
    }
    for op in ops.iter_mut() {
        if *op == Op::Call(f) {
            *op = Op::CallSelf;
        }
    }
    fuse(&mut ops);
    Some((
        Code {
            ops,
            params: info.params,
            locals,
            frame: locals + max + 1,
            func: f,
            heat: std::sync::atomic::AtomicU32::new(0),
            tier: 1,
            osr,
        },
        g,
    ))
}

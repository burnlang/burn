use super::*;
use crate::diag::Diagnostic;
use std::collections::BTreeSet;

#[derive(Default, Clone)]
struct Uses {
    reads: Vec<u32>,
    calls: Vec<FuncId>,
    indirect: Vec<u32>,
}

fn walk_stmts(stmts: &[Stmt], u: &mut Uses, slots: &[IfaceSlot], taken: &mut BTreeSet<FuncId>) {
    for s in stmts {
        match s {
            Stmt::Expr(e) | Stmt::Return(Some(e)) => walk(e, u, slots, taken),
            Stmt::If(c, a, b) => {
                walk(c, u, slots, taken);
                walk_stmts(a, u, slots, taken);
                walk_stmts(b, u, slots, taken);
            }
            Stmt::Loop { cond, body, step } => {
                if let Some(c) = cond {
                    walk(c, u, slots, taken);
                }
                walk_stmts(body, u, slots, taken);
                walk_stmts(step, u, slots, taken);
            }
            _ => {}
        }
    }
}

fn walk(e: &Expr, u: &mut Uses, slots: &[IfaceSlot], taken: &mut BTreeSet<FuncId>) {
    match &e.kind {
        ExprKind::Global(g) => u.reads.push(*g),
        ExprKind::FuncRef(f) => {
            taken.insert(*f);
        }
        ExprKind::Call(f, xs) | ExprKind::Spawn(f, xs) => {
            u.calls.push(*f);
            xs.iter().for_each(|x| walk(x, u, slots, taken));
        }
        ExprKind::CallIface(slot, xs) => {
            if let Some(s) = slots.get(*slot as usize) {
                u.calls.extend(s.impls.iter().map(|(_, f)| *f));
            }
            xs.iter().for_each(|x| walk(x, u, slots, taken));
        }
        ExprKind::CallIndirect(f, xs) => {
            u.indirect.push(xs.len() as u32);
            walk(f, u, slots, taken);
            xs.iter().for_each(|x| walk(x, u, slots, taken));
        }
        ExprKind::SetLocal(_, x)
        | ExprKind::SetGlobal(_, x)
        | ExprKind::Unary(_, x)
        | ExprKind::Conv(_, x)
        | ExprKind::GetField(x, _)
        | ExprKind::ArrLen(x)
        | ExprKind::BoxVal(x) => walk(x, u, slots, taken),
        ExprKind::Binary(_, a, b) | ExprKind::And(a, b) | ExprKind::Or(a, b) | ExprKind::SetField(a, _, b) | ExprKind::Index(a, b, _) => {
            walk(a, u, slots, taken);
            walk(b, u, slots, taken);
        }
        ExprKind::SetIndex(a, b, c, _) => {
            walk(a, u, slots, taken);
            walk(b, u, slots, taken);
            walk(c, u, slots, taken);
        }
        ExprKind::Rt(_, xs) | ExprKind::NewStruct(_, xs) | ExprKind::NewArray(_, xs) => xs.iter().for_each(|x| walk(x, u, slots, taken)),
        ExprKind::Seq(ss, x) => {
            walk_stmts(ss, u, slots, taken);
            walk(x, u, slots, taken);
        }
        _ => {}
    }
}

impl<'a> Checker<'a> {
    pub fn check_init_order(&mut self, loaded: &Loaded) {
        let mut taken = BTreeSet::new();
        let mut uses: Vec<Uses> = Vec::with_capacity(self.funcs.len());
        for f in &self.funcs {
            let mut u = Uses::default();
            if let Some(h) = &f.hir {
                walk_stmts(&h.body, &mut u, &self.slots, &mut taken);
            }
            uses.push(u);
        }
        let arity: Vec<u32> = self.funcs.iter().map(|f| f.params.len() as u32).collect();
        let spans = std::mem::take(&mut self.init_spans);
        for &mi in &loaded.order {
            let init = self.mods[mi].init as usize;
            let Some(body) = self.funcs[init].hir.as_ref().map(|h| h.body.clone()) else {
                continue;
            };
            let mut init_at: HashMap<u32, usize> = HashMap::new();
            for (g, gi) in self.globals.iter().enumerate() {
                if gi.module != mi {
                    continue;
                }
                if let Some((_, a, _, _)) = spans
                    .iter()
                    .find(|(m, _, _, sp)| *m == mi && sp.file == gi.span.file && sp.start <= gi.span.start && gi.span.end <= sp.end)
                {
                    init_at.insert(g as u32, *a);
                }
            }
            if init_at.is_empty() {
                continue;
            }
            let mut reported: HashSet<u32> = HashSet::new();
            for (i, s) in body.iter().enumerate() {
                let mut u = Uses::default();
                let mut scratch = BTreeSet::new();
                walk_stmts(std::slice::from_ref(s), &mut u, &self.slots, &mut scratch);
                let late = |g: &u32| init_at.get(g).map(|at| *at >= i).unwrap_or(false);
                let mut seen: HashSet<FuncId> = HashSet::new();
                let mut work: Vec<(FuncId, Vec<FuncId>)> = u.calls.iter().map(|f| (*f, vec![*f])).collect();
                for n in &u.indirect {
                    for f in taken.iter().filter(|f| arity[**f as usize] == *n) {
                        work.push((*f, vec![*f]));
                    }
                }
                let mut hit: Option<(u32, Vec<FuncId>)> = None;
                while let Some((f, path)) = work.pop() {
                    if !seen.insert(f) {
                        continue;
                    }
                    let fu = &uses[f as usize];
                    if let Some(g) = fu.reads.iter().find(|g| late(g) && !reported.contains(g)) {
                        hit = Some((*g, path));
                        break;
                    }
                    for c in &fu.calls {
                        let mut p = path.clone();
                        p.push(*c);
                        work.push((*c, p));
                    }
                    for n in &fu.indirect {
                        for t in taken.iter().filter(|t| arity[**t as usize] == *n) {
                            let mut p = path.clone();
                            p.push(*t);
                            work.push((*t, p));
                        }
                    }
                }
                if let Some(g) = u.reads.iter().find(|g| late(g) && !reported.contains(g)) {
                    hit = Some((*g, Vec::new()));
                }
                let Some((g, path)) = hit else { continue };
                reported.insert(g);
                let span = spans
                    .iter()
                    .find(|(m, a, b, _)| *m == mi && *a <= i && i < *b)
                    .map(|x| x.3)
                    .unwrap_or(self.globals[g as usize].span);
                let gi = &self.globals[g as usize];
                let (gname, gspan) = (gi.name.clone(), gi.span);
                let (line, _) = self.sm.file(gspan.file).line_col(gspan.start as usize);
                let chain: Vec<String> = path.iter().map(|f| format!("`{}`", self.funcs[*f as usize].name)).collect();
                let mut d = Diagnostic::error(span, format!("`{}` is used before it is initialized", gname));
                if !chain.is_empty() {
                    d = d.note(format!("this runs {}, which reads `{}`", chain.join(" → "), gname));
                }
                let d = d.help(format!(
                    "`{}` is initialized on line {}; move this statement below it, or move `{}` above this statement",
                    gname, line, gname
                ));
                self.emit(d);
            }
        }
    }
}

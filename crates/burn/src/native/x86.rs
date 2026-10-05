use crate::hir::{BinOp, Cmp, Const, Conv, Expr, ExprKind, External, Func, Program, Stmt, UnOp};
use crate::types::{Num, Ty};
use burn_runtime::RtFn;
use std::fmt::Write;

pub struct Target {
    pub prefix: &'static str,
    pub macos: bool,
}

impl Target {
    pub fn host() -> Target {
        if cfg!(target_os = "macos") {
            Target { prefix: "_", macos: true }
        } else {
            Target { prefix: "", macos: false }
        }
    }
}

const ENTRY: &str = include_str!("entry_x86_64.s");

pub const MIXINS: [&str; 3] = ["Inject", "Overwrite", "Redirect"];

fn quote(s: &str) -> String {
    format!("\"{}\"", bvm::asm::escape(s))
}

pub fn mixin_module(p: &Program) -> String {
    let mut s = String::new();
    for (i, f) in p.funcs.iter().enumerate() {
        let anns: Vec<_> = f.annotations.iter().filter(|a| MIXINS.contains(&a.name.as_str())).collect();
        if anns.is_empty() {
            continue;
        }
        let hook = format!("__mixin_hook_{}", i);
        writeln!(s, "import {} {}", quote(&hook), f.params).unwrap();
        for a in &anns {
            let _ = write!(s, "@{}", a.name);
            for (k, v) in &a.args {
                let v = match v {
                    Const::Int(x) => x.to_string(),
                    Const::Float(x) => format!("{:?}", x),
                    Const::Bool(b) => b.to_string(),
                    Const::Str(t) => quote(t),
                    Const::Null => continue,
                };
                let _ = write!(s, " {}={}", k, v);
            }
            s.push('\n');
        }
        writeln!(s, "func {}({})", quote(&format!("__mixin_{}", i)), f.params).unwrap();
        for k in 0..f.params {
            writeln!(s, "    load {}", k).unwrap();
        }
        writeln!(s, "    host {}", quote(&hook)).unwrap();
        s.push_str("    ret\nend\n");
    }
    s
}

const ARG_REGS: [&str; 6] = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];

struct Gen<'p> {
    p: &'p Program,
    t: &'p Target,
    out: String,
    cold: String,
    label: usize,
    fid: usize,
    loops: Vec<(String, String)>,
}

pub fn generate(p: &Program, meta: &[u8], t: &Target) -> String {
    let mut g = Gen {
        p,
        t,
        out: String::with_capacity(1 << 20),
        cold: String::new(),
        label: 0,
        fid: 0,
        loops: Vec::new(),
    };
    g.emit_all(meta);
    g.out
}

fn cc(c: Cmp) -> &'static str {
    match c {
        Cmp::Eq => "e",
        Cmp::Ne => "ne",
        Cmp::Lt => "l",
        Cmp::Le => "le",
        Cmp::Gt => "g",
        Cmp::Ge => "ge",
    }
}

fn ucc(c: Cmp) -> &'static str {
    match c {
        Cmp::Eq => "e",
        Cmp::Ne => "ne",
        Cmp::Lt => "b",
        Cmp::Le => "be",
        Cmp::Gt => "a",
        Cmp::Ge => "ae",
    }
}

fn inv(c: Cmp) -> Cmp {
    match c {
        Cmp::Eq => Cmp::Ne,
        Cmp::Ne => Cmp::Eq,
        Cmp::Lt => Cmp::Ge,
        Cmp::Le => Cmp::Gt,
        Cmp::Gt => Cmp::Le,
        Cmp::Ge => Cmp::Lt,
    }
}

fn is_const(e: &Expr) -> bool {
    matches!(
        e.kind,
        ExprKind::Int(_)
            | ExprKind::TypeId(_)
            | ExprKind::LocId(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Str(_)
            | ExprKind::Null
            | ExprKind::FuncRef(_)
    )
}

fn writes_local(e: &Expr, slot: u32) -> bool {
    let mut found = false;
    visit(e, &mut |x| {
        if let ExprKind::SetLocal(s, _) = x.kind {
            if s == slot {
                found = true;
            }
        }
    });
    found
}

fn writes_global(e: &Expr) -> bool {
    let mut found = false;
    visit(e, &mut |x| {
        if matches!(
            x.kind,
            ExprKind::SetGlobal(..) | ExprKind::Call(..) | ExprKind::CallIndirect(..) | ExprKind::CallIface(..) | ExprKind::Seq(..)
        ) {
            found = true;
        }
    });
    found
}

fn visit(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::SetLocal(_, x)
        | ExprKind::SetGlobal(_, x)
        | ExprKind::Unary(_, x)
        | ExprKind::Conv(_, x)
        | ExprKind::GetField(x, _)
        | ExprKind::ArrLen(x)
        | ExprKind::BoxVal(x)
        | ExprKind::Retain(x)
        | ExprKind::Release(x) => visit(x, f),
        ExprKind::Binary(_, a, b) | ExprKind::And(a, b) | ExprKind::Or(a, b) | ExprKind::Index(a, b, _) | ExprKind::SetField(a, _, b) => {
            visit(a, f);
            visit(b, f);
        }
        ExprKind::SetIndex(a, b, c, _) => {
            visit(a, f);
            visit(b, f);
            visit(c, f);
        }
        ExprKind::Call(_, xs)
        | ExprKind::CallIface(_, xs)
        | ExprKind::Rt(_, xs)
        | ExprKind::Spawn(_, xs)
        | ExprKind::NewStruct(_, xs)
        | ExprKind::NewArray(_, xs) => {
            for x in xs {
                visit(x, f);
            }
        }
        ExprKind::CallIndirect(c, xs) => {
            visit(c, f);
            for x in xs {
                visit(x, f);
            }
        }
        ExprKind::Seq(ss, x) => {
            for s in ss {
                visit_stmt(s, f);
            }
            visit(x, f);
        }
        _ => {}
    }
}

fn visit_stmt(s: &Stmt, f: &mut dyn FnMut(&Expr)) {
    match s {
        Stmt::Expr(e) | Stmt::Return(Some(e)) => visit(e, f),
        Stmt::If(c, a, b) => {
            visit(c, f);
            a.iter().for_each(|x| visit_stmt(x, f));
            b.iter().for_each(|x| visit_stmt(x, f));
        }
        Stmt::Loop { cond, body, step } => {
            if let Some(c) = cond {
                visit(c, f);
            }
            body.iter().for_each(|x| visit_stmt(x, f));
            step.iter().for_each(|x| visit_stmt(x, f));
        }
        _ => {}
    }
}

impl<'p> Gen<'p> {
    fn sym(&self, name: &str) -> String {
        format!("{}{}", self.t.prefix, name)
    }

    fn l(&mut self) -> String {
        self.label += 1;
        format!(".L{}", self.label)
    }

    fn e(&mut self, s: &str) {
        self.out.push_str("    ");
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn lbl(&mut self, l: &str) {
        self.out.push_str(l);
        self.out.push_str(":\n");
    }

    fn local(slot: u32) -> String {
        format!("qword ptr [rbp - {}]", 8 * (slot as usize + 1))
    }

    fn global(&self, g: u32) -> String {
        format!("qword ptr [rip + {} + {}]", self.sym("burn_globals"), 8 * g as usize)
    }

    fn call_rt_raw(&mut self, name: &str) {
        let s = self.sym(name);
        self.e("mov r12, rsp");
        self.e("and rsp, -16");
        if self.t.macos {
            self.e(&format!("call {}", s));
        } else {
            self.e(&format!("call {}@PLT", s));
        }
        self.e("mov rsp, r12");
    }

    fn emit_all(&mut self, meta: &[u8]) {
        let p = self.p;
        let plt = if self.t.macos { "" } else { "@PLT" };
        let exports: Vec<(String, usize, u32)> = if p.libs.is_empty() {
            Vec::new()
        } else {
            let mut v = Vec::new();
            for (i, f) in p.funcs.iter().enumerate() {
                if f.external.is_some() {
                    continue;
                }
                if let Some(a) = f.annotations.iter().find(|a| a.name == "Export") {
                    v.push((a.str_arg("name").unwrap_or(&f.name).to_string(), i, f.params));
                }
                if f.annotations.iter().any(|a| MIXINS.contains(&a.name.as_str())) {
                    v.push((format!("__mixin_hook_{}", i), i, f.params));
                }
            }
            v
        };
        let pre = if p.libs.is_empty() {
            String::new()
        } else {
            let pl = self.t.prefix;
            let mut s = String::new();
            s.push_str("    lea rdi, [rip + burn_exports]\n");
            s.push_str(&format!("    mov rsi, {}\n", exports.len()));
            s.push_str("    mov r12, rsp\n    and rsp, -16\n");
            s.push_str(&format!("    call {}burn_bvm_register_exports{}\n", pl, plt));
            s.push_str("    mov rsp, r12\n");
            s.push_str("    lea rdi, [rip + burn_mixins]\n");
            s.push_str("    mov r12, rsp\n    and rsp, -16\n");
            s.push_str(&format!("    call {}burn_bvm_register_mixins{}\n", pl, plt));
            s.push_str("    mov rsp, r12\n");
            for i in 0..p.libs.len() {
                s.push_str(&format!("    lea rdi, [rip + burn_lib_{}]\n", i));
                s.push_str(&format!("    mov rsi, qword ptr [rip + burn_lib_{}_len]\n", i));
                s.push_str("    mov r12, rsp\n    and rsp, -16\n");
                s.push_str(&format!("    call {}burn_bvm_init_lib{}\n", pl, plt));
                s.push_str("    mov rsp, r12\n");
            }
            s
        };
        self.out
            .push_str(&ENTRY.replace("{P}", self.t.prefix).replace("{PLT}", plt).replace("{PRE_ENTRY}", &pre));
        writeln!(self.out, ".set burn_entry, bf_{}", p.entry).unwrap();
        let meta_sym = "burn_meta";
        let glob_sym = self.sym("burn_globals");
        for (i, f) in p.funcs.iter().enumerate() {
            self.fid = i;
            self.func(f);
        }
        let rodata = if self.t.macos { ".section __TEXT,__const" } else { ".section .rodata" };
        self.out.push_str(rodata);
        self.out.push('\n');
        self.out.push_str(".p2align 4\n");
        writeln!(self.out, "burn_meta_len:\n    .quad {}", meta.len()).unwrap();
        writeln!(self.out, "burn_nglobals:\n    .quad {}", p.globals.len()).unwrap();
        writeln!(self.out, "{}:", meta_sym).unwrap();
        self.bytes(meta);
        for (i, l) in p.libs.iter().enumerate() {
            self.out.push_str(".p2align 4\n");
            writeln!(self.out, "burn_lib_{}_len:\n    .quad {}", i, l.bytes.len()).unwrap();
            writeln!(self.out, "burn_lib_{}:", i).unwrap();
            self.bytes(&l.bytes);
        }
        for (i, f) in p.funcs.iter().enumerate() {
            if let Some(External::Lib { name, .. }) = &f.external {
                writeln!(self.out, "burn_libfn_{}:", i).unwrap();
                let mut b = name.as_bytes().to_vec();
                b.push(0);
                self.bytes(&b);
            }
        }
        if !p.libs.is_empty() {
            for (k, (name, _, _)) in exports.iter().enumerate() {
                writeln!(self.out, "burn_export_name_{}:", k).unwrap();
                let mut b = name.as_bytes().to_vec();
                b.push(0);
                self.bytes(&b);
            }
            self.out.push_str("burn_mixins:\n");
            let mut b = mixin_module(p).into_bytes();
            b.push(0);
            self.bytes(&b);
        }
        for (i, s) in p.strings.iter().enumerate() {
            let b = s.as_bytes();
            let flags = 1 | if s.is_ascii() { 2 } else { 0 };
            self.out.push_str(".p2align 4\n");
            writeln!(self.out, "bs_{}:", i).unwrap();
            writeln!(self.out, "    .byte 1, 0, {}, 0", flags).unwrap();
            writeln!(self.out, "    .long {}", burn_runtime::meta::TID_STR).unwrap();
            writeln!(self.out, "    .long {}, 0", 24 + b.len() + 1).unwrap();
            writeln!(self.out, "    .quad {}", b.len()).unwrap();
            let mut bb = b.to_vec();
            bb.push(0);
            self.bytes(&bb);
        }
        let relro = if self.t.macos { ".section __DATA,__const" } else { ".section .data.rel.ro" };
        self.out.push_str(relro);
        self.out.push('\n');
        if !p.libs.is_empty() {
            self.out.push_str(".p2align 3\nburn_exports:\n");
            for (k, (_, f, argc)) in exports.iter().enumerate() {
                writeln!(self.out, "    .quad burn_export_name_{}, bf_{}, {}", k, f, argc).unwrap();
            }
            if exports.is_empty() {
                self.out.push_str("    .quad 0\n");
            }
        }
        let ntypes = p.types.len();
        for (i, s) in p.slots.iter().enumerate() {
            let mut table = vec![String::from("0"); ntypes];
            for (tid, f) in &s.impls {
                table[*tid as usize] = format!("bf_{}", f);
            }
            self.out.push_str(".p2align 3\n");
            writeln!(self.out, "bi_{}:", i).unwrap();
            for chunk in table.chunks(8) {
                writeln!(self.out, "    .quad {}", chunk.join(", ")).unwrap();
            }
        }
        let bss = if self.t.macos { ".section __DATA,__bss" } else { ".bss" };
        self.out.push_str(bss);
        self.out.push('\n');
        self.out.push_str(".p2align 4\n");
        writeln!(self.out, "{}:", glob_sym).unwrap();
        writeln!(self.out, "    .zero {}", 8 * p.globals.len().max(1)).unwrap();
        if !self.t.macos {
            self.out.push_str(".section .note.GNU-stack,\"\",@progbits\n");
        }
    }

    fn bytes(&mut self, b: &[u8]) {
        for chunk in b.chunks(32) {
            let parts: Vec<String> = chunk.iter().map(|x| x.to_string()).collect();
            writeln!(self.out, "    .byte {}", parts.join(",")).unwrap();
        }
    }

    fn func(&mut self, f: &Func) {
        self.out.push_str(".p2align 4\n");
        writeln!(self.out, "bf_{}:", self.fid).unwrap();
        if let Some(External::Lib { lib, .. }) = &f.external {
            self.e("push rbp");
            self.e("mov rbp, rsp");
            self.e(&format!("lea rdi, [rip + burn_lib_{}]", lib));
            self.e(&format!("mov rsi, qword ptr [rip + burn_lib_{}_len]", lib));
            self.e(&format!("lea rdx, [rip + burn_libfn_{}]", self.fid));
            self.e(&format!("mov ecx, {}", f.params));
            self.e("lea r8, [rbp + 16]");
            self.e("and rsp, -16");
            let s = self.sym("burn_bvm_call");
            let plt = if self.t.macos { "" } else { "@PLT" };
            self.e(&format!("call {}{}", s, plt));
            self.e("leave");
            self.e("ret");
            return;
        }
        self.e("push rbp");
        self.e("mov rbp, rsp");
        let n = f.locals.len().max(f.params as usize);
        let frame = (n * 8 + 15) & !15;
        if frame > 0 {
            self.e(&format!("sub rsp, {}", frame));
        }
        let np = f.params as usize;
        for i in 0..np {
            self.e(&format!("mov rax, qword ptr [rbp + {}]", 16 + 8 * (np - 1 - i)));
            self.e(&format!("mov {}, rax", Self::local(i as u32)));
        }
        for s in &f.body {
            self.stmt(s);
        }
        self.e("xor eax, eax");
        self.e("leave");
        self.e("ret");
        let cold = std::mem::take(&mut self.cold);
        self.out.push_str(&cold);
    }

    fn stmts(&mut self, ss: &[Stmt]) {
        for s in ss {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Expr(e) => self.expr(e),
            Stmt::If(c, a, b) => {
                let else_l = self.l();
                self.jump_false(c, &else_l);
                self.stmts(a);
                if b.is_empty() {
                    self.lbl(&else_l);
                } else {
                    let end = self.l();
                    self.e(&format!("jmp {}", end));
                    self.lbl(&else_l);
                    self.stmts(b);
                    self.lbl(&end);
                }
            }
            Stmt::Loop { cond, body, step } => {
                let top = self.l();
                let cont = self.l();
                let end = self.l();
                self.lbl(&top);
                if let Some(c) = cond {
                    self.jump_false(c, &end);
                }
                self.loops.push((cont.clone(), end.clone()));
                self.stmts(body);
                self.loops.pop();
                self.lbl(&cont);
                self.stmts(step);
                self.e(&format!("jmp {}", top));
                self.lbl(&end);
            }
            Stmt::Return(v) => {
                match v {
                    Some(e) => self.expr(e),
                    None => self.e("xor eax, eax"),
                }
                self.e("leave");
                self.e("ret");
            }
            Stmt::Break => {
                let end = self.loops.last().unwrap().1.clone();
                self.e(&format!("jmp {}", end));
            }
            Stmt::Continue => {
                let c = self.loops.last().unwrap().0.clone();
                self.e(&format!("jmp {}", c));
            }
        }
    }

    fn jump_false(&mut self, c: &Expr, target: &str) {
        match &c.kind {
            ExprKind::Bool(true) => {}
            ExprKind::Bool(false) => self.e(&format!("jmp {}", target)),
            ExprKind::Binary(BinOp::ICmp(k), a, b) => {
                self.operands(a, b);
                self.e("cmp rax, rcx");
                self.e(&format!("j{} {}", cc(inv(*k)), target));
            }
            ExprKind::Unary(UnOp::Not, x) => self.jump_true(x, target),
            ExprKind::And(a, b) => {
                self.jump_false(a, target);
                self.jump_false(b, target);
            }
            ExprKind::Or(a, b) => {
                let t = self.l();
                self.jump_true(a, &t);
                self.jump_false(b, target);
                self.lbl(&t);
            }
            _ => {
                self.expr(c);
                self.e("test rax, rax");
                self.e(&format!("jz {}", target));
            }
        }
    }

    fn jump_true(&mut self, c: &Expr, target: &str) {
        match &c.kind {
            ExprKind::Bool(false) => {}
            ExprKind::Bool(true) => self.e(&format!("jmp {}", target)),
            ExprKind::Binary(BinOp::ICmp(k), a, b) => {
                self.operands(a, b);
                self.e("cmp rax, rcx");
                self.e(&format!("j{} {}", cc(*k), target));
            }
            ExprKind::Unary(UnOp::Not, x) => self.jump_false(x, target),
            ExprKind::Or(a, b) => {
                self.jump_true(a, target);
                self.jump_true(b, target);
            }
            ExprKind::And(a, b) => {
                let f = self.l();
                self.jump_false(a, &f);
                self.jump_true(b, target);
                self.lbl(&f);
            }
            _ => {
                self.expr(c);
                self.e("test rax, rax");
                self.e(&format!("jnz {}", target));
            }
        }
    }

    fn simple_load(&mut self, e: &Expr, reg: &str) -> bool {
        match &e.kind {
            ExprKind::Int(v) => {
                if *v == 0 {
                    self.e(&format!("xor {}, {}", reg, reg));
                } else if *v >= i32::MIN as i64 && *v <= i32::MAX as i64 {
                    self.e(&format!("mov {}, {}", reg, v));
                } else {
                    self.e(&format!("movabs {}, {}", reg, v));
                }
            }
            ExprKind::TypeId(v) | ExprKind::LocId(v) => {
                if *v <= i32::MAX as u32 {
                    self.e(&format!("mov {}, {}", reg, v));
                } else {
                    self.e(&format!("movabs {}, {}", reg, v));
                }
            }
            ExprKind::Float(f) => {
                let b = f.to_bits();
                if b == 0 {
                    self.e(&format!("xor {}, {}", reg, reg));
                } else {
                    self.e(&format!("movabs {}, {}", reg, b as i64));
                }
            }
            ExprKind::Bool(b) => self.e(&format!("mov {}, {}", reg, *b as u8)),
            ExprKind::Null => self.e(&format!("xor {}, {}", reg, reg)),
            ExprKind::Str(i) => self.e(&format!("lea {}, [rip + bs_{}]", reg, i)),
            ExprKind::FuncRef(f) => self.e(&format!("lea {}, [rip + bf_{}]", reg, f)),
            ExprKind::Local(s) => self.e(&format!("mov {}, {}", reg, Self::local(*s))),
            ExprKind::Global(g) => {
                let m = self.global(*g);
                self.e(&format!("mov {}, {}", reg, m))
            }
            _ => return false,
        }
        true
    }

    fn is_simple(e: &Expr) -> bool {
        matches!(
            e.kind,
            ExprKind::Int(_)
                | ExprKind::TypeId(_)
                | ExprKind::LocId(_)
                | ExprKind::Float(_)
                | ExprKind::Bool(_)
                | ExprKind::Str(_)
                | ExprKind::Null
                | ExprKind::FuncRef(_)
                | ExprKind::Local(_)
                | ExprKind::Global(_)
        )
    }

    fn operands(&mut self, a: &Expr, b: &Expr) {
        if Self::is_simple(b) {
            self.expr(a);
            self.simple_load(b, "rcx");
        } else if is_const(a) {
            self.expr(b);
            self.e("mov rcx, rax");
            self.simple_load(a, "rax");
        } else {
            self.expr(a);
            self.e("push rax");
            self.expr(b);
            self.e("mov rcx, rax");
            self.e("pop rax");
        }
    }

    fn late_ok(arg: &Expr, all: &[Expr]) -> bool {
        match arg.kind {
            ExprKind::Local(s) => !all.iter().any(|x| writes_local(x, s)),
            ExprKind::Global(_) => !all.iter().any(writes_global),
            _ => is_const(arg),
        }
    }

    fn rt_call(&mut self, f: RtFn, args: &[Expr]) {
        let n = args.len();
        let late: Vec<bool> = args.iter().map(|a| Self::late_ok(a, args)).collect();
        for (i, a) in args.iter().enumerate() {
            if !late[i] {
                self.expr(a);
                self.e("push rax");
            }
        }
        for i in (0..n).rev() {
            if late[i] {
                self.simple_load(&args[i], ARG_REGS[i]);
            } else {
                self.e(&format!("pop {}", ARG_REGS[i]));
            }
        }
        self.call_rt_raw(f.symbol());
    }

    fn push_args(&mut self, args: &[Expr]) {
        for a in args {
            match &a.kind {
                ExprKind::Int(v) if *v >= i32::MIN as i64 && *v <= i32::MAX as i64 => self.e(&format!("push {}", v)),
                ExprKind::TypeId(v) | ExprKind::LocId(v) if *v <= i32::MAX as u32 => self.e(&format!("push {}", v)),
                ExprKind::Local(s) => self.e(&format!("push {}", Self::local(*s))),
                _ => {
                    self.expr(a);
                    self.e("push rax");
                }
            }
        }
    }

    fn pop_args(&mut self, n: usize) {
        if n > 0 {
            self.e(&format!("add rsp, {}", 8 * n));
        }
    }

    fn overflow(&mut self, loc: u32) {
        if loc == u32::MAX {
            return;
        }
        let ov = self.l();
        self.e(&format!("jo {}", ov));
        let sym = self.sym(RtFn::ErrOverflow.symbol());
        let call = if self.t.macos { format!("call {}", sym) } else { format!("call {}@PLT", sym) };
        writeln!(self.cold, "{}:\n    mov edi, {}\n    and rsp, -16\n    {}\n    ud2", ov, loc, call).unwrap();
    }

    fn div(&mut self, is_mod: bool, loc: u32) {
        let dz = self.l();
        let normal = self.l();
        let done = self.l();
        self.e("test rcx, rcx");
        self.e(&format!("jz {}", dz));
        self.e("cmp rcx, -1");
        self.e(&format!("jne {}", normal));
        if is_mod {
            self.e("xor eax, eax");
        } else {
            self.e("neg rax");
            self.overflow(loc);
        }
        self.e(&format!("jmp {}", done));
        self.lbl(&normal);
        self.e("cqo");
        self.e("idiv rcx");
        if is_mod {
            self.e("mov rax, rdx");
        }
        self.lbl(&done);
        let sym = self.sym(RtFn::ErrDivZero.symbol());
        let call = if self.t.macos { format!("call {}", sym) } else { format!("call {}@PLT", sym) };
        writeln!(self.cold, "{}:\n    mov edi, {}\n    and rsp, -16\n    {}\n    ud2", dz, loc, call).unwrap();
    }

    fn expr(&mut self, e: &Expr) {
        if self.simple_load(e, "rax") {
            return;
        }
        match &e.kind {
            ExprKind::SetLocal(s, v) => {
                self.expr(v);
                self.e(&format!("mov {}, rax", Self::local(*s)));
            }
            ExprKind::SetGlobal(g, v) => {
                self.expr(v);
                let m = self.global(*g);
                self.e(&format!("mov {}, rax", m));
            }
            ExprKind::Unary(op, x) => {
                self.expr(x);
                match op {
                    UnOp::INeg(l) => {
                        self.e("neg rax");
                        self.overflow(*l);
                    }
                    UnOp::FNeg => self.e("btc rax, 63"),
                    UnOp::BitNot => self.e("not rax"),
                    UnOp::Not => {
                        self.e("test rax, rax");
                        self.e("sete al");
                        self.e("movzx eax, al");
                    }
                }
            }
            ExprKind::Binary(op, a, b) => {
                self.operands(a, b);
                match op {
                    BinOp::IAdd(l) => {
                        self.e("add rax, rcx");
                        self.overflow(*l);
                    }
                    BinOp::ISub(l) => {
                        self.e("sub rax, rcx");
                        self.overflow(*l);
                    }
                    BinOp::IMul(l) => {
                        self.e("imul rax, rcx");
                        self.overflow(*l);
                    }
                    BinOp::BitAnd => self.e("and rax, rcx"),
                    BinOp::BitOr => self.e("or rax, rcx"),
                    BinOp::BitXor => self.e("xor rax, rcx"),
                    BinOp::Shl(l) | BinOp::Shr(l) | BinOp::UShr(l) => {
                        if *l != u32::MAX {
                            let bad = self.l();
                            self.e("cmp rcx, 63");
                            self.e(&format!("ja {}", bad));
                            let sym = self.sym(RtFn::ErrShift.symbol());
                            let call = if self.t.macos { format!("call {}", sym) } else { format!("call {}@PLT", sym) };
                            writeln!(
                                self.cold,
                                "{}:\n    mov rsi, rcx\n    mov edi, {}\n    and rsp, -16\n    {}\n    ud2",
                                bad, l, call
                            )
                            .unwrap();
                        }
                        self.e(match op {
                            BinOp::Shl(_) => "shl rax, cl",
                            BinOp::Shr(_) => "sar rax, cl",
                            _ => "shr rax, cl",
                        });
                    }
                    BinOp::IDiv(l) => self.div(false, *l),
                    BinOp::IMod(l) => self.div(true, *l),
                    BinOp::FAdd | BinOp::FSub | BinOp::FMul | BinOp::FDiv => {
                        self.e("movq xmm0, rax");
                        self.e("movq xmm1, rcx");
                        let ins = match op {
                            BinOp::FAdd => "addsd",
                            BinOp::FSub => "subsd",
                            BinOp::FMul => "mulsd",
                            _ => "divsd",
                        };
                        self.e(&format!("{} xmm0, xmm1", ins));
                        self.e("movq rax, xmm0");
                    }
                    BinOp::ICmp(c) => {
                        self.e("cmp rax, rcx");
                        self.e(&format!("set{} al", cc(*c)));
                        self.e("movzx eax, al");
                    }
                    BinOp::UCmp(c) => {
                        self.e("cmp rax, rcx");
                        self.e(&format!("set{} al", ucc(*c)));
                        self.e("movzx eax, al");
                    }
                    BinOp::WAdd => self.e("add rax, rcx"),
                    BinOp::WSub => self.e("sub rax, rcx"),
                    BinOp::WMul => self.e("imul rax, rcx"),
                    BinOp::FCmp(c) => {
                        self.e("movq xmm0, rax");
                        self.e("movq xmm1, rcx");
                        match c {
                            Cmp::Eq => {
                                self.e("ucomisd xmm0, xmm1");
                                self.e("sete al");
                                self.e("setnp cl");
                                self.e("and al, cl");
                            }
                            Cmp::Ne => {
                                self.e("ucomisd xmm0, xmm1");
                                self.e("setne al");
                                self.e("setp cl");
                                self.e("or al, cl");
                            }
                            Cmp::Lt => {
                                self.e("ucomisd xmm1, xmm0");
                                self.e("seta al");
                            }
                            Cmp::Le => {
                                self.e("ucomisd xmm1, xmm0");
                                self.e("setae al");
                            }
                            Cmp::Gt => {
                                self.e("ucomisd xmm0, xmm1");
                                self.e("seta al");
                            }
                            Cmp::Ge => {
                                self.e("ucomisd xmm0, xmm1");
                                self.e("setae al");
                            }
                        }
                        self.e("movzx eax, al");
                    }
                }
            }
            ExprKind::And(a, b) => {
                let end = self.l();
                self.expr(a);
                self.e("test rax, rax");
                self.e(&format!("jz {}", end));
                self.expr(b);
                self.lbl(&end);
            }
            ExprKind::Or(a, b) => {
                let end = self.l();
                self.expr(a);
                self.e("test rax, rax");
                self.e(&format!("jnz {}", end));
                self.expr(b);
                self.lbl(&end);
            }
            ExprKind::Conv(c, x) => {
                self.expr(x);
                match c {
                    Conv::IntToFloat => {
                        self.e("cvtsi2sd xmm0, rax");
                        self.e("movq rax, xmm0");
                    }
                    Conv::FloatToInt => {
                        self.e("movq xmm0, rax");
                        self.e("cvttsd2si rax, xmm0");
                    }
                }
            }
            ExprKind::Call(f, args) => {
                self.push_args(args);
                self.e(&format!("call bf_{}", f));
                self.pop_args(args.len());
            }
            ExprKind::CallIndirect(c, args) => {
                self.push_args(args);
                self.expr(c);
                self.e("call rax");
                self.pop_args(args.len());
            }
            ExprKind::CallIface(slot, args) => {
                self.push_args(args);
                let n = args.len();
                self.e(&format!("mov rax, qword ptr [rsp + {}]", 8 * (n - 1)));
                self.e("mov eax, dword ptr [rax + 4]");
                self.e(&format!("lea rcx, [rip + bi_{}]", slot));
                self.e("mov rax, qword ptr [rcx + rax*8]");
                self.e("call rax");
                self.pop_args(n);
            }
            ExprKind::Rt(f, args) => {
                if !self.inline_num(*f, args) {
                    self.rt_call(*f, args)
                }
            }
            ExprKind::Spawn(f, args) => {
                self.push_args(args);
                self.e(&format!("lea rdi, [rip + bf_{}]", f));
                self.e(&format!("mov rsi, {}", args.len()));
                self.e("mov rdx, rsp");
                self.e(&format!("mov rcx, {}", e.ty));
                self.call_rt_raw(RtFn::Spawn.symbol());
                self.pop_args(args.len());
            }
            ExprKind::NewStruct(t, fields) => {
                let n = fields.len();
                self.push_args(fields);
                self.e(&format!("mov edi, {}", t));
                self.e(&format!("mov esi, {}", n));
                self.call_rt_raw(RtFn::StructNew.symbol());
                for i in (0..n).rev() {
                    self.e("pop rcx");
                    self.e(&format!("mov qword ptr [rax + {}], rcx", 16 + 8 * i));
                }
            }
            ExprKind::GetField(o, i) => {
                self.expr(o);
                self.e(&format!("mov rax, qword ptr [rax + {}]", 16 + 8 * *i as usize));
            }
            ExprKind::SetField(o, i, v) => {
                if Self::is_simple(v) {
                    self.expr(o);
                    self.simple_load(v, "rcx");
                    self.e(&format!("mov qword ptr [rax + {}], rcx", 16 + 8 * *i as usize));
                    self.e("mov rax, rcx");
                } else {
                    self.expr(o);
                    self.e("push rax");
                    self.expr(v);
                    self.e("pop rcx");
                    self.e(&format!("mov qword ptr [rcx + {}], rax", 16 + 8 * *i as usize));
                }
            }
            ExprKind::NewArray(t, items) => {
                let n = items.len();
                self.push_args(items);
                self.e(&format!("mov edi, {}", t));
                self.e(&format!("mov esi, {}", n));
                self.call_rt_raw(RtFn::ArrNew.symbol());
                if n > 0 {
                    let elem = self.packed_elem(*t);
                    self.e("mov rdx, qword ptr [rax + 32]");
                    for i in (0..n).rev() {
                        self.e("pop rcx");
                        match elem {
                            None => self.e(&format!("mov qword ptr [rdx + {}], rcx", 8 * i)),
                            Some(Num::F32) => {
                                self.e("movq xmm0, rcx");
                                self.e("cvtsd2ss xmm0, xmm0");
                                self.e(&format!("movss dword ptr [rdx + {}], xmm0", 4 * i));
                            }
                            Some(k) => {
                                let (reg, w) = match k.width() {
                                    1 => ("cl", "byte"),
                                    2 => ("cx", "word"),
                                    _ => ("ecx", "dword"),
                                };
                                self.e(&format!("mov {} ptr [rdx + {}], {}", w, k.width() * i, reg));
                            }
                        }
                    }
                }
            }
            ExprKind::Index(a, i, loc) => {
                self.operands(a, i);
                let bad = self.l();
                self.e("cmp rcx, qword ptr [rax + 16]");
                self.e(&format!("jae {}", bad));
                self.e("mov rdx, qword ptr [rax + 32]");
                let load = match self.packed_elem(a.ty) {
                    None => "mov rax, qword ptr [rdx + rcx*8]",
                    Some(Num::I8) => "movsx rax, byte ptr [rdx + rcx]",
                    Some(Num::U8) => "movzx eax, byte ptr [rdx + rcx]",
                    Some(Num::I16) => "movsx rax, word ptr [rdx + rcx*2]",
                    Some(Num::U16) => "movzx eax, word ptr [rdx + rcx*2]",
                    Some(Num::I32) => "movsxd rax, dword ptr [rdx + rcx*4]",
                    Some(Num::U32) => "mov eax, dword ptr [rdx + rcx*4]",
                    Some(_) => "cvtss2sd xmm0, dword ptr [rdx + rcx*4]\n    movq rax, xmm0",
                };
                self.e(load);
                self.index_cold(&bad, "rax", *loc);
            }
            ExprKind::SetIndex(a, i, v, loc) => {
                self.expr(a);
                self.e("push rax");
                self.expr(i);
                self.e("push rax");
                self.expr(v);
                self.e("pop rcx");
                self.e("pop rdx");
                let bad = self.l();
                self.e("cmp rcx, qword ptr [rdx + 16]");
                self.e(&format!("jae {}", bad));
                self.e("mov r8, qword ptr [rdx + 32]");
                let store = match self.packed_elem(a.ty) {
                    None => "mov qword ptr [r8 + rcx*8], rax",
                    Some(Num::I8 | Num::U8) => "mov byte ptr [r8 + rcx], al",
                    Some(Num::I16 | Num::U16) => "mov word ptr [r8 + rcx*2], ax",
                    Some(Num::I32 | Num::U32) => "mov dword ptr [r8 + rcx*4], eax",
                    Some(_) => "movq xmm0, rax\n    cvtsd2ss xmm0, xmm0\n    movss dword ptr [r8 + rcx*4], xmm0",
                };
                self.e(store);
                self.index_cold(&bad, "rdx", *loc);
            }
            ExprKind::ArrLen(a) => {
                self.expr(a);
                self.e("mov rax, qword ptr [rax + 16]");
            }
            ExprKind::BoxVal(x) => {
                self.expr(x);
                self.e("mov rax, qword ptr [rax + 16]");
            }
            ExprKind::Seq(ss, x) => {
                self.stmts(ss);
                self.expr(x);
            }
            ExprKind::Retain(x) => {
                self.expr(x);
                let done = self.l();
                let slow = self.l();
                let mt = self.sym("burn_rc_mt");
                self.e("test rax, rax");
                self.e(&format!("jz {}", done));
                self.e("test byte ptr [rax + 2], 1");
                self.e(&format!("jnz {}", done));
                self.e(&format!("cmp byte ptr [rip + {}], 0", mt));
                self.e(&format!("jne {}", slow));
                self.e("inc dword ptr [rax + 12]");
                self.lbl(&done);
                let call = self.cold_call(RtFn::Retain.symbol());
                writeln!(self.cold, "{}:\n    mov rdi, rax\n{}    jmp {}", slow, call, done).unwrap();
            }
            ExprKind::Release(x) => {
                self.expr(x);
                let done = self.l();
                let mt_l = self.l();
                let zero = self.l();
                let root = self.l();
                let mt = self.sym("burn_rc_mt");
                self.e("test rax, rax");
                self.e(&format!("jz {}", done));
                self.e("test byte ptr [rax + 2], 1");
                self.e(&format!("jnz {}", done));
                self.e(&format!("cmp byte ptr [rip + {}], 0", mt));
                self.e(&format!("jne {}", mt_l));
                self.e("dec dword ptr [rax + 12]");
                self.e(&format!("jz {}", zero));
                self.e("test byte ptr [rax + 2], 4");
                self.e(&format!("jnz {}", root));
                self.lbl(&done);
                for (l, f) in [(mt_l, RtFn::Release), (zero, RtFn::ReleaseZero), (root, RtFn::PossibleRoot)] {
                    let call = self.cold_call(f.symbol());
                    writeln!(self.cold, "{}:\n    mov rdi, rax\n{}    jmp {}", l, call, done).unwrap();
                }
            }
            _ => unreachable!(),
        }
    }

    fn packed_elem(&self, arr: u32) -> Option<Num> {
        match self.p.types.get(arr) {
            Ty::Array(e) => self.p.types.num_of(*e).filter(|n| n.packed()),
            _ => None,
        }
    }

    fn rt_cold(&mut self, label: &str, setup: &str, f: RtFn) {
        let sym = self.sym(f.symbol());
        let call = if self.t.macos { format!("call {}", sym) } else { format!("call {}@PLT", sym) };
        writeln!(self.cold, "{}:\n{}    and rsp, -16\n    {}\n    ud2", label, setup, call).unwrap();
    }

    fn inline_num(&mut self, f: RtFn, args: &[Expr]) -> bool {
        let code = |e: &Expr| match e.kind {
            ExprKind::Int(v) => Num::from_code(v as u8),
            _ => None,
        };
        let loc = |e: &Expr| match e.kind {
            ExprKind::LocId(l) => l,
            _ => u32::MAX,
        };
        match f {
            RtFn::NumWrap => {
                let Some(n) = code(&args[1]) else { return false };
                self.expr(&args[0]);
                self.e(match n {
                    Num::I8 => "movsx rax, al",
                    Num::U8 => "movzx eax, al",
                    Num::I16 => "movsx rax, ax",
                    Num::U16 => "movzx eax, ax",
                    Num::I32 => "movsxd rax, eax",
                    Num::U32 => "mov eax, eax",
                    _ => return true,
                });
                true
            }
            RtFn::NumFit => {
                let Some(n) = code(&args[1]) else { return false };
                if !n.packed() || n.is_float() {
                    return false;
                }
                self.expr(&args[0]);
                let bad = self.l();
                match n {
                    Num::U8 | Num::U16 => {
                        self.e(&format!("cmp rax, {}", n.max_value()));
                        self.e(&format!("ja {}", bad));
                    }
                    _ => {
                        self.e(match n {
                            Num::I8 => "movsx rcx, al",
                            Num::I16 => "movsx rcx, ax",
                            Num::I32 => "movsxd rcx, eax",
                            _ => "mov ecx, eax",
                        });
                        self.e("cmp rcx, rax");
                        self.e(&format!("jne {}", bad));
                    }
                }
                let setup = format!("    mov rdi, rax\n    mov esi, {}\n    mov edx, {}\n", n as u8, loc(&args[2]));
                self.rt_cold(&bad, &setup, RtFn::NumFit);
                true
            }
            RtFn::F32Round => {
                self.expr(&args[0]);
                self.e("movq xmm0, rax");
                self.e("cvtsd2ss xmm0, xmm0");
                self.e("cvtss2sd xmm0, xmm0");
                self.e("movq rax, xmm0");
                true
            }
            RtFn::UAdd | RtFn::USub | RtFn::UMul => {
                self.operands(&args[0], &args[1]);
                let bad = self.l();
                match f {
                    RtFn::UAdd => {
                        self.e("mov rdx, rax");
                        self.e("add rdx, rcx");
                        self.e(&format!("jc {}", bad));
                        self.e("mov rax, rdx");
                    }
                    RtFn::USub => {
                        self.e("mov rdx, rax");
                        self.e("sub rdx, rcx");
                        self.e(&format!("jc {}", bad));
                        self.e("mov rax, rdx");
                    }
                    _ => {
                        self.e("mov r8, rax");
                        self.e("mul rcx");
                        self.e(&format!("jo {}", bad));
                    }
                }
                let first = if f == RtFn::UMul { "r8" } else { "rax" };
                let setup = format!("    mov rdi, {}\n    mov rsi, rcx\n    mov edx, {}\n", first, loc(&args[2]));
                self.rt_cold(&bad, &setup, f);
                true
            }
            _ => false,
        }
    }

    fn cold_call(&self, name: &str) -> String {
        let s = self.sym(name);
        let target = if self.t.macos { s } else { format!("{}@PLT", s) };
        format!("    mov r12, rsp\n    and rsp, -16\n    call {}\n    mov rsp, r12\n", target)
    }

    fn index_cold(&mut self, label: &str, arr: &str, loc: u32) {
        let sym = self.sym(RtFn::ErrIndex.symbol());
        let call = if self.t.macos { format!("call {}", sym) } else { format!("call {}@PLT", sym) };
        writeln!(
            self.cold,
            "{}:\n    mov rsi, rcx\n    mov rdx, qword ptr [{} + 16]\n    mov edi, {}\n    and rsp, -16\n    {}\n    ud2",
            label, arr, loc, call
        )
        .unwrap();
    }
}

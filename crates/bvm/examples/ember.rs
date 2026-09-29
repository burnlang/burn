use bvm::{Cmp, FuncBuilder, FuncId, Host, ModuleBuilder, Op, RtFn};
use std::collections::HashMap;
use std::process::ExitCode;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Int(i64),
    Str(String),
    Ident(String),
    Sym(&'static str),
    Eof,
}

const SYMS: [&str; 23] = [
    "==", "!=", "<=", ">=", "&&", "||", "+", "-", "*", "/", "%", "<", ">", "=", "!", "(", ")", "{", "}", "[", "]", ",", ":",
];

fn lex(src: &str) -> Result<Vec<(Tok, usize)>, String> {
    let cs: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    while i < cs.len() {
        let c = cs[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() || c == ';' {
            i += 1;
        } else if c == '#' {
            while i < cs.len() && cs[i] != '\n' {
                i += 1;
            }
        } else if c.is_ascii_digit() {
            let start = i;
            while i < cs.len() && cs[i].is_ascii_digit() {
                i += 1;
            }
            let text: String = cs[start..i].iter().collect();
            out.push((Tok::Int(text.parse().map_err(|_| format!("line {}: number too large", line))?), line));
        } else if c == '"' {
            let mut s = String::new();
            i += 1;
            while i < cs.len() && cs[i] != '"' {
                if cs[i] == '\\' && i + 1 < cs.len() {
                    i += 1;
                    s.push(match cs[i] {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                } else {
                    s.push(cs[i]);
                }
                i += 1;
            }
            if i == cs.len() {
                return Err(format!("line {}: unterminated string", line));
            }
            i += 1;
            out.push((Tok::Str(s), line));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            out.push((Tok::Ident(cs[start..i].iter().collect()), line));
        } else {
            let rest: String = cs[i..(i + 2).min(cs.len())].iter().collect();
            let sym = SYMS
                .iter()
                .find(|s| rest.starts_with(**s))
                .ok_or_else(|| format!("line {}: unexpected character {:?}", line, c))?;
            out.push((Tok::Sym(sym), line));
            i += sym.len();
        }
    }
    out.push((Tok::Eof, line));
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ty {
    Int,
    Str,
    Arr,
    Void,
}

impl Ty {
    fn tid(self) -> i64 {
        match self {
            Ty::Int => 3,
            Ty::Str => 6,
            Ty::Arr => 11,
            Ty::Void => 1,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Ty::Int => "int",
            Ty::Str => "str",
            Ty::Arr => "arr",
            Ty::Void => "nothing",
        }
    }
}

#[derive(Debug)]
enum Expr {
    Int(i64),
    Str(String),
    Var(String),
    Array(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
}

#[derive(Debug)]
enum Stmt {
    Let(String, Expr),
    Assign(String, Expr),
    SetIndex(Expr, Expr, Expr),
    If(Expr, Vec<Line>, Vec<Line>),
    While(Expr, Vec<Line>),
    Return(Option<Expr>),
    Expr(Expr),
}

type Line = (usize, Stmt);

struct Func {
    name: String,
    params: Vec<(String, Ty)>,
    ret: Ty,
    body: Vec<Line>,
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].0
    }
    fn line(&self) -> usize {
        self.toks[self.i].1
    }
    fn next(&mut self) -> Tok {
        let t = self.toks[self.i].0.clone();
        if self.i + 1 < self.toks.len() {
            self.i += 1;
        }
        t
    }
    fn err<T>(&self, msg: &str) -> Result<T, String> {
        Err(format!("line {}: {}", self.line(), msg))
    }
    fn eat(&mut self, s: &str) -> bool {
        if matches!(self.peek(), Tok::Sym(x) if *x == s) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) -> Result<(), String> {
        if self.eat(s) {
            Ok(())
        } else {
            self.err(&format!("expected {} but found {:?}", s, self.peek()))
        }
    }
    fn keyword(&mut self, k: &str) -> bool {
        if matches!(self.peek(), Tok::Ident(x) if x == k) {
            self.next();
            true
        } else {
            false
        }
    }
    fn ident(&mut self) -> Result<String, String> {
        match self.next() {
            Tok::Ident(s) => Ok(s),
            t => self.err(&format!("expected a name but found {:?}", t)),
        }
    }
    fn ty(&mut self) -> Result<Ty, String> {
        match self.ident()?.as_str() {
            "int" => Ok(Ty::Int),
            "str" => Ok(Ty::Str),
            "arr" => Ok(Ty::Arr),
            other => self.err(&format!("unknown type {}", other)),
        }
    }

    fn program(&mut self) -> Result<(Vec<Func>, Vec<Line>), String> {
        let mut funcs = Vec::new();
        let mut top = Vec::new();
        while *self.peek() != Tok::Eof {
            if self.keyword("fn") {
                let name = self.ident()?;
                self.expect("(")?;
                let mut params = Vec::new();
                while !self.eat(")") {
                    let p = self.ident()?;
                    let t = if self.eat_colon() { self.ty()? } else { Ty::Int };
                    params.push((p, t));
                    if !self.eat(",") {
                        self.expect(")")?;
                        break;
                    }
                }
                let ret = if self.eat_colon() { self.ty()? } else { Ty::Int };
                let body = self.block()?;
                funcs.push(Func { name, params, ret, body });
            } else {
                top.push(self.stmt()?);
            }
        }
        Ok((funcs, top))
    }

    fn eat_colon(&mut self) -> bool {
        self.eat(":")
    }

    fn block(&mut self) -> Result<Vec<Line>, String> {
        self.expect("{")?;
        let mut out = Vec::new();
        while !self.eat("}") {
            if *self.peek() == Tok::Eof {
                return self.err("unclosed {");
            }
            out.push(self.stmt()?);
        }
        Ok(out)
    }

    fn stmt(&mut self) -> Result<Line, String> {
        let line = self.line();
        let s = if self.keyword("let") {
            let name = self.ident()?;
            self.expect("=")?;
            Stmt::Let(name, self.expr()?)
        } else if self.keyword("if") {
            let c = self.expr()?;
            let a = self.block()?;
            let b = if self.keyword("else") {
                if matches!(self.peek(), Tok::Ident(k) if k == "if") {
                    vec![self.stmt()?]
                } else {
                    self.block()?
                }
            } else {
                Vec::new()
            };
            Stmt::If(c, a, b)
        } else if self.keyword("while") {
            let c = self.expr()?;
            Stmt::While(c, self.block()?)
        } else if self.keyword("return") {
            if matches!(self.peek(), Tok::Sym("}")) {
                Stmt::Return(None)
            } else {
                Stmt::Return(Some(self.expr()?))
            }
        } else {
            let e = self.expr()?;
            if self.eat("=") {
                let v = self.expr()?;
                match e {
                    Expr::Var(n) => Stmt::Assign(n, v),
                    Expr::Index(a, i) => Stmt::SetIndex(*a, *i, v),
                    _ => return self.err("only variables and array elements can be assigned"),
                }
            } else {
                Stmt::Expr(e)
            }
        };
        Ok((line, s))
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.binary(0)
    }

    fn binary(&mut self, level: usize) -> Result<Expr, String> {
        const LEVELS: [&[&str]; 5] = [&["||"], &["&&"], &["==", "!=", "<", "<=", ">", ">="], &["+", "-"], &["*", "/", "%"]];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut l = self.binary(level + 1)?;
        loop {
            let op = match self.peek() {
                Tok::Sym(s) if LEVELS[level].contains(s) => *s,
                _ => return Ok(l),
            };
            self.next();
            let r = self.binary(level + 1)?;
            l = Expr::Binary(op, Box::new(l), Box::new(r));
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat("-") {
            return Ok(Expr::Unary("-", Box::new(self.unary()?)));
        }
        if self.eat("!") {
            return Ok(Expr::Unary("!", Box::new(self.unary()?)));
        }
        let mut e = self.primary()?;
        while self.eat("[") {
            let i = self.expr()?;
            self.expect("]")?;
            e = Expr::Index(Box::new(e), Box::new(i));
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Tok::Int(v) => Ok(Expr::Int(v)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Ident(n) => {
                if self.eat("(") {
                    let mut args = Vec::new();
                    while !self.eat(")") {
                        args.push(self.expr()?);
                        if !self.eat(",") {
                            self.expect(")")?;
                            break;
                        }
                    }
                    Ok(Expr::Call(n, args))
                } else {
                    Ok(Expr::Var(n))
                }
            }
            Tok::Sym("(") => {
                let e = self.expr()?;
                self.expect(")")?;
                Ok(e)
            }
            Tok::Sym("[") => {
                let mut xs = Vec::new();
                while !self.eat("]") {
                    xs.push(self.expr()?);
                    if !self.eat(",") {
                        self.expect("]")?;
                        break;
                    }
                }
                Ok(Expr::Array(xs))
            }
            t => self.err(&format!("expected an expression but found {:?}", t)),
        }
    }
}

struct Sig {
    id: FuncId,
    params: Vec<Ty>,
    ret: Ty,
}

struct Gen<'a> {
    mb: &'a mut ModuleBuilder,
    file: &'a str,
    sigs: &'a HashMap<String, Sig>,
    globals: &'a mut HashMap<String, (u32, Ty)>,
    top: bool,
    f: FuncBuilder,
    locals: HashMap<String, (u32, Ty)>,
    ret: Ty,
    line: usize,
}

impl Gen<'_> {
    fn err<T>(&self, msg: String) -> Result<T, String> {
        Err(format!("{}:{}: {}", self.file, self.line, msg))
    }

    fn loc(&mut self) -> u32 {
        let l = format!("{}:{}", self.file, self.line);
        self.mb.loc(&l)
    }

    fn var(&self, name: &str) -> Option<(bool, u32, Ty)> {
        if let Some((slot, ty)) = self.locals.get(name) {
            return Some((true, *slot, *ty));
        }
        self.globals.get(name).map(|(g, ty)| (false, *g, *ty))
    }

    fn block(&mut self, body: &[Line]) -> Result<(), String> {
        for (line, s) in body {
            self.line = *line;
            self.stmt(s)?;
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> Result<(), String> {
        match s {
            Stmt::Let(name, e) => {
                let ty = self.expr(e)?;
                if ty == Ty::Void {
                    return self.err(format!("{} cannot hold nothing", name));
                }
                if self.top {
                    let g = match self.globals.get(name) {
                        Some((g, _)) => *g,
                        None => self.mb.global(name),
                    };
                    self.globals.insert(name.clone(), (g, ty));
                    self.f.emit(Op::GStore(g));
                } else {
                    let slot = self.f.local(name);
                    self.locals.insert(name.clone(), (slot, ty));
                    self.f.store(slot);
                }
            }
            Stmt::Assign(name, e) => {
                let ty = self.expr(e)?;
                let Some((local, slot, want)) = self.var(name) else {
                    return self.err(format!("{} is not defined; declare it with let", name));
                };
                if ty != want {
                    return self.err(format!("{} holds {}, not {}", name, want.name(), ty.name()));
                }
                self.f.emit(if local { Op::Store(slot) } else { Op::GStore(slot) });
            }
            Stmt::SetIndex(a, i, v) => {
                self.expect(a, Ty::Arr)?;
                self.expect(i, Ty::Int)?;
                self.expect(v, Ty::Int)?;
                let l = self.loc();
                self.f.emit(Op::SetIndex(l)).emit(Op::Pop);
            }
            Stmt::If(c, a, b) => {
                self.expect(c, Ty::Int)?;
                let other = self.f.label();
                let end = self.f.label();
                self.f.jz(other);
                self.block(a)?;
                self.f.jmp(end);
                self.f.bind(other);
                self.block(b)?;
                self.f.bind(end);
            }
            Stmt::While(c, body) => {
                let top = self.f.here();
                let end = self.f.label();
                self.expect(c, Ty::Int)?;
                self.f.jz(end);
                self.block(body)?;
                self.f.jmp(top);
                self.f.bind(end);
            }
            Stmt::Return(None) => {
                if self.ret != Ty::Void {
                    return self.err(format!("this function must return {}", self.ret.name()));
                }
                self.f.ret_void();
            }
            Stmt::Return(Some(e)) => {
                let ret = self.ret;
                self.expect(e, ret)?;
                self.f.ret();
            }
            Stmt::Expr(e) => {
                self.expr(e)?;
                self.f.emit(Op::Pop);
            }
        }
        Ok(())
    }

    fn expect(&mut self, e: &Expr, want: Ty) -> Result<(), String> {
        let got = self.expr(e)?;
        if got != want {
            return self.err(format!("expected {} but found {}", want.name(), got.name()));
        }
        Ok(())
    }

    fn stringify(&mut self, ty: Ty) {
        if ty != Ty::Str {
            self.f.int(ty.tid()).rt(RtFn::ToStr);
        }
    }

    fn expr(&mut self, e: &Expr) -> Result<Ty, String> {
        Ok(match e {
            Expr::Int(v) => {
                self.f.int(*v);
                Ty::Int
            }
            Expr::Str(s) => {
                let i = self.mb.string(s);
                self.f.emit(Op::Str(i));
                Ty::Str
            }
            Expr::Var(n) => match self.var(n) {
                Some((true, slot, ty)) => {
                    self.f.load(slot);
                    ty
                }
                Some((false, g, ty)) => {
                    self.f.emit(Op::GLoad(g));
                    ty
                }
                None => return self.err(format!("{} is not defined", n)),
            },
            Expr::Array(xs) => {
                for x in xs {
                    self.expect(x, Ty::Int)?;
                }
                self.f.emit(Op::NewArray(Ty::Arr.tid() as u32, xs.len() as u32));
                Ty::Arr
            }
            Expr::Index(a, i) => {
                self.expect(a, Ty::Arr)?;
                self.expect(i, Ty::Int)?;
                let l = self.loc();
                self.f.emit(Op::Index(l));
                Ty::Int
            }
            Expr::Unary(op, x) => {
                self.expect(x, Ty::Int)?;
                self.f.emit(if *op == "-" { Op::INeg } else { Op::Not });
                Ty::Int
            }
            Expr::Binary(op, a, b) => return self.binary(op, a, b),
            Expr::Call(name, args) => return self.call(name, args),
        })
    }

    fn binary(&mut self, op: &str, a: &Expr, b: &Expr) -> Result<Ty, String> {
        if op == "&&" || op == "||" {
            self.expect(a, Ty::Int)?;
            let end = self.f.label();
            if op == "&&" {
                self.f.jz_keep(end);
            } else {
                self.f.jnz_keep(end);
            }
            self.f.emit(Op::Pop);
            self.expect(b, Ty::Int)?;
            self.f.bind(end);
            return Ok(Ty::Int);
        }
        let ta = self.expr(a)?;
        if op == "+" && ta == Ty::Str {
            let tb = self.expr(b)?;
            self.stringify(tb);
            self.f.rt(RtFn::StrConcat);
            return Ok(Ty::Str);
        }
        let tb = self.expr(b)?;
        if op == "+" && tb == Ty::Str {
            self.f.emit(Op::Swap);
            self.stringify(ta);
            self.f.emit(Op::Swap).rt(RtFn::StrConcat);
            return Ok(Ty::Str);
        }
        if ta == Ty::Str && tb == Ty::Str && (op == "==" || op == "!=") {
            self.f.rt(RtFn::StrEq);
            if op == "!=" {
                self.f.emit(Op::Not);
            }
            return Ok(Ty::Int);
        }
        if ta != Ty::Int || tb != Ty::Int {
            return self.err(format!("{} works on ints, not {} and {}", op, ta.name(), tb.name()));
        }
        let op = match op {
            "+" => Op::IAdd,
            "-" => Op::ISub,
            "*" => Op::IMul,
            "/" => Op::IDiv(self.loc()),
            "%" => Op::IRem(self.loc()),
            "==" => Op::ICmp(Cmp::Eq),
            "!=" => Op::ICmp(Cmp::Ne),
            "<" => Op::ICmp(Cmp::Lt),
            "<=" => Op::ICmp(Cmp::Le),
            ">" => Op::ICmp(Cmp::Gt),
            _ => Op::ICmp(Cmp::Ge),
        };
        self.f.emit(op);
        Ok(Ty::Int)
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> Result<Ty, String> {
        match name {
            "print" => {
                if args.is_empty() {
                    let i = self.mb.string("");
                    self.f.emit(Op::Str(i));
                }
                for (k, a) in args.iter().enumerate() {
                    let t = self.expr(a)?;
                    if t == Ty::Void {
                        return self.err("print needs values".into());
                    }
                    self.stringify(t);
                    if k > 0 {
                        let sp = self.mb.string(" ");
                        self.f.emit(Op::Str(sp)).emit(Op::Swap).rt(RtFn::StrConcat).rt(RtFn::StrConcat);
                    }
                }
                self.f.rt(RtFn::Print);
                return Ok(Ty::Void);
            }
            "len" if args.len() == 1 => {
                return match self.expr(&args[0])? {
                    Ty::Arr => {
                        self.f.emit(Op::Len);
                        Ok(Ty::Int)
                    }
                    Ty::Str => {
                        self.f.rt(RtFn::StrLen);
                        Ok(Ty::Int)
                    }
                    t => self.err(format!("len needs arr or str, not {}", t.name())),
                };
            }
            "push" if args.len() == 2 => {
                self.expect(&args[0], Ty::Arr)?;
                self.expect(&args[1], Ty::Int)?;
                self.f.rt(RtFn::ArrPush);
                return Ok(Ty::Void);
            }
            "clock" if args.is_empty() => {
                self.f.rt(RtFn::NowMs);
                return Ok(Ty::Int);
            }
            "square" if args.len() == 1 => {
                self.expect(&args[0], Ty::Int)?;
                let h = self.mb.import("square", 1);
                self.f.emit(Op::Host(h));
                return Ok(Ty::Int);
            }
            _ => {}
        }
        let Some(sig) = self.sigs.get(name) else {
            return self.err(format!("unknown function {}", name));
        };
        if sig.params.len() != args.len() {
            return self.err(format!("{} takes {} arguments, not {}", name, sig.params.len(), args.len()));
        }
        for (a, t) in args.iter().zip(sig.params.clone()) {
            self.expect(a, t)?;
        }
        self.f.call(sig.id);
        Ok(sig.ret)
    }
}

fn compile(file: &str, src: &str) -> Result<bvm::Module, String> {
    let toks = lex(src).map_err(|e| format!("{}:{}", file, e))?;
    let (funcs, top) = Parser { toks, i: 0 }.program().map_err(|e| format!("{}:{}", file, e))?;
    let mut mb = ModuleBuilder::new();
    let mut sigs = HashMap::new();
    for f in &funcs {
        let id = mb.declare(&f.name, f.params.len() as u32);
        sigs.insert(
            f.name.clone(),
            Sig {
                id,
                params: f.params.iter().map(|p| p.1).collect(),
                ret: f.ret,
            },
        );
    }
    let mut globals = HashMap::new();
    let main = mb.declare("main", 0);
    let mut g = Gen {
        mb: &mut mb,
        file,
        sigs: &sigs,
        globals: &mut globals,
        top: true,
        f: FuncBuilder::new(0),
        locals: HashMap::new(),
        ret: Ty::Void,
        line: 1,
    };
    g.block(&top)?;
    if g.f.reachable_end() {
        g.f.ret_void();
    }
    let main_body = std::mem::replace(&mut g.f, FuncBuilder::new(0));
    drop(g);
    mb.define(main, main_body);
    for f in &funcs {
        let names: Vec<&str> = f.params.iter().map(|p| p.0.as_str()).collect();
        let mut g = Gen {
            mb: &mut mb,
            file,
            sigs: &sigs,
            globals: &mut globals,
            top: false,
            f: FuncBuilder::with_params(&names),
            locals: f.params.iter().enumerate().map(|(i, (n, t))| (n.clone(), (i as u32, *t))).collect(),
            ret: f.ret,
            line: 1,
        };
        g.block(&f.body)?;
        if g.f.reachable_end() {
            if f.ret == Ty::Void {
                g.f.ret_void();
            } else {
                g.f.int(0).ret();
            }
        }
        let body = std::mem::replace(&mut g.f, FuncBuilder::new(0));
        drop(g);
        mb.define(sigs[&f.name].id, body);
    }
    mb.entry(main);
    Ok(mb.finish())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(file) = args.first() else {
        eprintln!("usage: ember <file.em> [--emit-asm <out.bvm>] [-o <out.bvmc>]");
        return ExitCode::from(2);
    };
    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", file, e);
            return ExitCode::from(1);
        }
    };
    let module = match compile(file, &src) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::from(1);
        }
    };
    let mut wrote = false;
    let mut i = 1;
    while i + 1 < args.len() {
        let out = &args[i + 1];
        let data = match args[i].as_str() {
            "--emit-asm" => bvm::asm::disassemble(&module).into_bytes(),
            "-o" => bvm::binary::encode(&module),
            other => {
                eprintln!("error: unknown option {}", other);
                return ExitCode::from(2);
            }
        };
        if let Err(e) = std::fs::write(out, data) {
            eprintln!("error: cannot write {}: {}", out, e);
            return ExitCode::from(1);
        }
        wrote = true;
        i += 2;
    }
    if wrote {
        return ExitCode::SUCCESS;
    }
    let mut host = Host::new();
    host.register("square", 1, |a| (a[0] as i64).wrapping_mul(a[0] as i64) as u64);
    match bvm::run(&module, &host, Vec::new()) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::from(1)
        }
    }
}

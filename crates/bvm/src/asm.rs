use crate::module::{Annotation, Function, Import, Module, Sig, Table, Target, Value, FIRST_USER_TYPE};
use crate::op::{rt_by_name, rt_name, Cmp, Op, NO_LOC};
use burn_runtime::meta::Desc;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub struct AsmError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for AsmError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for AsmError {}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Int(i128),
    Float(f64),
    Str(String),
    Hash(u32),
    At(u32),
    Annot(String),
    P(char),
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Tok::Ident(s) => write!(f, "{}", s),
            Tok::Int(v) => write!(f, "{}", v),
            Tok::Float(v) => write!(f, "{}", v),
            Tok::Str(s) => write!(f, "\"{}\"", escape(s)),
            Tok::Hash(n) => write!(f, "#{}", n),
            Tok::At(n) => write!(f, "@{}", n),
            Tok::Annot(n) => write!(f, "@{}", n),
            Tok::P(c) => write!(f, "{}", c),
        }
    }
}

type R<T> = Result<T, String>;

const BUILTIN_TYPES: [&str; 8] = ["error", "void", "null", "int", "float", "bool", "string", "any"];
const TYPE_WORDS: [&str; 9] = ["fun", "map", "future", "record", "struct", "class", "interface", "enum", "implements"];

fn ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

fn ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '.'
}

fn is_ident(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some(c) if ident_start(c)) && cs.all(ident_char) && !s.ends_with('.')
}

fn is_type_ident(s: &str) -> bool {
    is_ident(s) && !BUILTIN_TYPES.contains(&s) && !TYPE_WORDS.contains(&s)
}

pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(o, "\\u{{{:x}}}", c as u32);
            }
            c => o.push(c),
        }
    }
    o
}

fn lex(line: &str) -> R<Vec<Tok>> {
    let cs: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == ';' {
            break;
        } else if c == '"' {
            i += 1;
            let mut s = String::new();
            loop {
                match cs.get(i) {
                    None => return Err("unterminated string".into()),
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some('\\') => {
                        i += 1;
                        let e = *cs.get(i).ok_or("unterminated string")?;
                        i += 1;
                        match e {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            '0' => s.push('\0'),
                            '\\' => s.push('\\'),
                            '"' => s.push('"'),
                            'u' => {
                                if cs.get(i) != Some(&'{') {
                                    return Err("expected { after \\u".into());
                                }
                                let start = i + 1;
                                let end = (start..cs.len()).find(|j| cs[*j] == '}').ok_or("unterminated \\u{...} escape")?;
                                let hex: String = cs[start..end].iter().collect();
                                let v = u32::from_str_radix(&hex, 16).map_err(|_| format!("bad escape \\u{{{}}}", hex))?;
                                s.push(char::from_u32(v).ok_or_else(|| format!("\\u{{{}}} is not a character", hex))?);
                                i = end + 1;
                            }
                            e => return Err(format!("unknown escape \\{}", e)),
                        }
                    }
                    Some(ch) => {
                        s.push(*ch);
                        i += 1;
                    }
                }
            }
            out.push(Tok::Str(s));
        } else if c == '@' && cs.get(i + 1).is_some_and(|d| ident_start(*d)) {
            let start = i + 1;
            i = start;
            while i < cs.len() && ident_char(cs[i]) {
                i += 1;
            }
            out.push(Tok::Annot(cs[start..i].iter().collect()));
        } else if c == '#' || c == '@' {
            let start = i + 1;
            let mut j = start;
            while j < cs.len() && cs[j].is_ascii_digit() {
                j += 1;
            }
            if j == start {
                return Err(format!("expected a number after {}", c));
            }
            let n: String = cs[start..j].iter().collect();
            let n: u32 = n.parse().map_err(|_| format!("{}{} is too large", c, n))?;
            out.push(if c == '#' { Tok::Hash(n) } else { Tok::At(n) });
            i = j;
        } else if c.is_ascii_digit() || (c == '-' && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            i += 1;
            if c == '0' && matches!(cs.get(i), Some('x') | Some('X')) || c == '-' && cs.get(i) == Some(&'0') && matches!(cs.get(i + 1), Some('x') | Some('X')) {
                let neg = c == '-';
                i = if neg { i + 2 } else { i + 1 };
                let hs = i;
                while i < cs.len() && (cs[i].is_ascii_hexdigit() || cs[i] == '_') {
                    i += 1;
                }
                let hex: String = cs[hs..i].iter().filter(|c| **c != '_').collect();
                let v = i128::from_str_radix(&hex, 16).map_err(|_| format!("bad hex number 0x{}", hex))?;
                out.push(Tok::Int(if neg { -v } else { v }));
                continue;
            }
            let mut float = false;
            while i < cs.len() {
                let d = cs[i];
                if d.is_ascii_digit() || d == '_' {
                    i += 1;
                } else if d == '.' && cs.get(i + 1).is_some_and(|x| x.is_ascii_digit()) {
                    float = true;
                    i += 1;
                } else if (d == 'e' || d == 'E')
                    && (cs.get(i + 1).is_some_and(|x| x.is_ascii_digit())
                        || matches!(cs.get(i + 1), Some('+') | Some('-')) && cs.get(i + 2).is_some_and(|x| x.is_ascii_digit()))
                {
                    float = true;
                    i += 2;
                } else {
                    break;
                }
            }
            let text: String = cs[start..i].iter().filter(|c| **c != '_').collect();
            if float {
                out.push(Tok::Float(text.parse().map_err(|_| format!("bad number {}", text))?));
            } else {
                out.push(Tok::Int(text.parse().map_err(|_| format!("bad number {}", text))?));
            }
        } else if ident_start(c) {
            let start = i;
            while i < cs.len() && ident_char(cs[i]) {
                i += 1;
            }
            while i > start + 1 && cs[i - 1] == '.' {
                i -= 1;
            }
            out.push(Tok::Ident(cs[start..i].iter().collect()));
        } else if "(){}[]<>,:=?-".contains(c) {
            out.push(Tok::P(c));
            i += 1;
        } else {
            return Err(format!("unexpected character {:?}", c));
        }
    }
    Ok(out)
}

struct Line {
    no: usize,
    toks: Vec<Tok>,
}

fn logical_lines(src: &str) -> Result<Vec<Line>, AsmError> {
    let mut out: Vec<Line> = Vec::new();
    let mut pending: Option<Line> = None;
    let mut depth = 0i32;
    for (i, raw) in src.lines().enumerate() {
        let toks = lex(raw).map_err(|msg| AsmError { line: i + 1, msg })?;
        if toks.is_empty() {
            continue;
        }
        for t in &toks {
            match t {
                Tok::P('{') => depth += 1,
                Tok::P('}') => depth -= 1,
                _ => {}
            }
        }
        match pending.as_mut() {
            Some(p) => p.toks.extend(toks),
            None => pending = Some(Line { no: i + 1, toks }),
        }
        if depth <= 0 {
            depth = 0;
            out.push(pending.take().unwrap());
        }
    }
    if let Some(p) = pending {
        return Err(AsmError {
            line: p.no,
            msg: "unclosed {".into(),
        });
    }
    Ok(out)
}

struct Cur<'a> {
    t: &'a [Tok],
    i: usize,
}

impl<'a> Cur<'a> {
    fn new(t: &'a [Tok]) -> Cur<'a> {
        Cur { t, i: 0 }
    }
    fn peek(&self) -> Option<&'a Tok> {
        self.t.get(self.i)
    }
    fn next(&mut self) -> R<&'a Tok> {
        let t = self.t.get(self.i).ok_or("unexpected end of line")?;
        self.i += 1;
        Ok(t)
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(&Tok::P(c)) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: char) -> R<()> {
        match self.next()? {
            Tok::P(x) if *x == c => Ok(()),
            t => Err(format!("expected {} but found {}", c, t)),
        }
    }
    fn eat_word(&mut self, w: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(s)) if s == w) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn name(&mut self) -> R<String> {
        match self.next()? {
            Tok::Ident(s) | Tok::Str(s) => Ok(s.clone()),
            t => Err(format!("expected a name but found {}", t)),
        }
    }
    fn uint(&mut self) -> R<u32> {
        match self.next()? {
            Tok::Int(v) if *v >= 0 && *v <= u32::MAX as i128 => Ok(*v as u32),
            t => Err(format!("expected a count but found {}", t)),
        }
    }
    fn done(&self) -> R<()> {
        match self.peek() {
            None => Ok(()),
            Some(t) => Err(format!("unexpected {} at the end of the line", t)),
        }
    }
}

enum TyRef {
    Tid(u32),
    D(Desc),
}

struct Asm {
    m: Module,
    type_names: HashMap<String, u32>,
    func_names: HashMap<String, Vec<u32>>,
    global_names: HashMap<String, Vec<u32>>,
    table_names: HashMap<String, Vec<u32>>,
}

fn pick(map: &HashMap<String, Vec<u32>>, what: &str, name: &str) -> R<u32> {
    match map.get(name).map(|v| v.as_slice()) {
        Some([one]) => Ok(*one),
        Some([_, _, ..]) => Err(format!("{} {} is ambiguous; refer to it by @index", what, name)),
        _ => Err(format!("unknown {} {}", what, name)),
    }
}

impl Asm {
    fn ty_ref(&mut self, c: &mut Cur) -> R<TyRef> {
        let mut r = match c.next()? {
            Tok::P('[') => {
                let e = self.ty(c)?;
                c.expect(']')?;
                TyRef::D(Desc::Array(e))
            }
            Tok::Hash(n) => TyRef::Tid(*n),
            Tok::Str(s) => TyRef::Tid(*self.type_names.get(s).ok_or_else(|| format!("unknown type {:?}", s))?),
            Tok::Ident(s) => match s.as_str() {
                "error" => TyRef::D(Desc::Error),
                "void" => TyRef::D(Desc::Void),
                "null" => TyRef::D(Desc::Null),
                "int" => TyRef::D(Desc::Int),
                "float" => TyRef::D(Desc::Float),
                "bool" => TyRef::D(Desc::Bool),
                "string" => TyRef::D(Desc::Str),
                "any" => TyRef::D(Desc::Any),
                "fun" => TyRef::D(Desc::Func),
                n if burn_runtime::meta::Num::ALL.iter().any(|k| k.name() == n) => {
                    TyRef::D(Desc::Num(*burn_runtime::meta::Num::ALL.iter().find(|k| k.name() == n).unwrap()))
                }
                "map" => {
                    c.expect('<')?;
                    let k = self.ty(c)?;
                    c.expect(',')?;
                    let v = self.ty(c)?;
                    c.expect('>')?;
                    TyRef::D(Desc::Map(k, v))
                }
                "future" => {
                    c.expect('<')?;
                    let t = self.ty(c)?;
                    c.expect('>')?;
                    TyRef::D(Desc::Future(t))
                }
                "record" | "struct" | "class" | "interface" | "enum" => {
                    c.i -= 1;
                    TyRef::D(self.def(c, "")?)
                }
                name => TyRef::Tid(*self.type_names.get(name).ok_or_else(|| format!("unknown type {}", name))?),
            },
            t => return Err(format!("expected a type but found {}", t)),
        };
        while c.eat('?') {
            let inner = self.intern(r);
            r = TyRef::D(Desc::Optional(inner));
        }
        Ok(r)
    }

    fn intern(&mut self, r: TyRef) -> u32 {
        match r {
            TyRef::Tid(t) => t,
            TyRef::D(d) => self.m.intern_type(d),
        }
    }

    fn ty(&mut self, c: &mut Cur) -> R<u32> {
        let r = self.ty_ref(c)?;
        Ok(self.intern(r))
    }

    fn def(&mut self, c: &mut Cur, decl: &str) -> R<Desc> {
        let kind = match c.peek() {
            Some(Tok::Ident(s)) if matches!(s.as_str(), "record" | "struct" | "class" | "interface" | "enum") => s.clone(),
            _ => {
                return match self.ty_ref(c)? {
                    TyRef::D(d) => Ok(d),
                    TyRef::Tid(t) => self.m.types.get(t as usize).cloned().ok_or_else(|| format!("type #{} does not exist", t)),
                }
            }
        };
        c.i += 1;
        let name = match c.peek() {
            Some(Tok::Str(s)) => {
                c.i += 1;
                s.clone()
            }
            _ => decl.to_string(),
        };
        match kind.as_str() {
            "interface" => Ok(Desc::Interface { name }),
            "enum" => {
                c.expect('{')?;
                let mut variants = Vec::new();
                while !c.eat('}') {
                    variants.push(c.name()?);
                    if !c.eat(',') {
                        c.expect('}')?;
                        break;
                    }
                }
                Ok(Desc::Enum { name, variants })
            }
            _ => {
                c.expect('{')?;
                let mut fields = Vec::new();
                while !c.eat('}') {
                    let f = c.name()?;
                    c.expect(':')?;
                    let t = self.ty(c)?;
                    fields.push((f, t));
                    if !c.eat(',') {
                        c.expect('}')?;
                        break;
                    }
                }
                let mut implements = Vec::new();
                if c.eat_word("implements") {
                    loop {
                        implements.push(self.ty(c)?);
                        if !c.eat(',') {
                            break;
                        }
                    }
                }
                Ok(Desc::Record {
                    name,
                    fields,
                    class: kind == "class" || kind == "struct",
                    implements,
                })
            }
        }
    }

    fn func_ref(&self, c: &mut Cur) -> R<u32> {
        match c.next()? {
            Tok::At(n) => Ok(*n),
            Tok::Ident(s) | Tok::Str(s) => pick(&self.func_names, "function", s),
            t => Err(format!("expected a function but found {}", t)),
        }
    }

    fn global_ref(&self, c: &mut Cur) -> R<u32> {
        match c.next()? {
            Tok::At(n) => Ok(*n),
            Tok::Int(n) if *n >= 0 && *n <= u32::MAX as i128 => Ok(*n as u32),
            Tok::Ident(s) | Tok::Str(s) => pick(&self.global_names, "global", s),
            t => Err(format!("expected a global but found {}", t)),
        }
    }

    fn table_ref(&self, c: &mut Cur) -> R<u32> {
        match c.next()? {
            Tok::At(n) => Ok(*n),
            Tok::Ident(s) | Tok::Str(s) => pick(&self.table_names, "table", s),
            t => Err(format!("expected a table but found {}", t)),
        }
    }

    fn import_ref(&self, c: &mut Cur) -> R<u32> {
        match c.next()? {
            Tok::At(n) => Ok(*n),
            Tok::Ident(s) | Tok::Str(s) => self
                .m
                .imports
                .iter()
                .position(|i| i.name == *s)
                .map(|i| i as u32)
                .ok_or_else(|| format!("unknown import {}; declare it with: import {} <argc>", s, s)),
            t => Err(format!("expected an import but found {}", t)),
        }
    }
}

fn loc_operand(c: &mut Cur) -> R<u32> {
    match c.peek() {
        None => Ok(NO_LOC),
        Some(_) => c.uint(),
    }
}

fn const_operand(c: &mut Cur) -> R<u64> {
    let neg = c.eat('-');
    let v = match c.next()? {
        Tok::Int(v) => {
            let v = if neg { -*v } else { *v };
            if v < i64::MIN as i128 || v > u64::MAX as i128 {
                return Err(format!("{} does not fit in 64 bits", v));
            }
            if v < 0 {
                v as i64 as u64
            } else {
                v as u64
            }
        }
        Tok::Float(f) => (if neg { -*f } else { *f }).to_bits(),
        Tok::Ident(s) => match s.as_str() {
            "true" => 1,
            "false" | "null" => 0,
            "inf" => (if neg { f64::NEG_INFINITY } else { f64::INFINITY }).to_bits(),
            "nan" => f64::NAN.to_bits(),
            _ => return Err(format!("expected a number but found {}", s)),
        },
        t => return Err(format!("expected a number but found {}", t)),
    };
    Ok(v)
}

pub fn assemble(src: &str) -> Result<Module, AsmError> {
    let lines = logical_lines(src)?;
    let mut a = Asm {
        m: Module::new(),
        type_names: HashMap::new(),
        func_names: HashMap::new(),
        global_names: HashMap::new(),
        table_names: HashMap::new(),
    };
    let e = |line: usize| move |msg: String| AsmError { line, msg };
    let mut bodies: Vec<(usize, usize, usize)> = Vec::new();
    let mut headers: Vec<(usize, u32)> = Vec::new();
    let mut pending: Vec<(String, Vec<(String, Value)>)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = &lines[i];
        let mut c = Cur::new(&l.toks);
        let word = match c.next().map_err(e(l.no))? {
            Tok::Ident(w) => w.clone(),
            Tok::Annot(name) => {
                let args = annotation_args(&mut c).map_err(e(l.no))?;
                pending.push((name.clone(), args));
                i += 1;
                continue;
            }
            t => return Err(e(l.no)(format!("expected a directive but found {}", t))),
        };
        if !pending.is_empty() && !matches!(word.as_str(), "func" | "extern" | "type" | "global" | "module") {
            return Err(e(l.no)("an annotation must come right before func, extern, type, global or module".into()));
        }
        match word.as_str() {
            "module" => {
                a.m.name = c.name().map_err(e(l.no))?;
                c.done().map_err(e(l.no))?;
                attach(&mut a.m, &mut pending, Target::Module);
            }
            "type" => {
                let tid = a.m.types.len() as u32;
                match c.next().map_err(e(l.no))? {
                    Tok::Hash(n) if *n == tid => {}
                    Tok::Hash(n) => return Err(e(l.no)(format!("type #{} is declared where #{} comes next", n, tid))),
                    Tok::Ident(n) if is_type_ident(n) => {
                        if a.type_names.insert(n.clone(), tid).is_some() {
                            return Err(e(l.no)(format!("type {} is declared twice", n)));
                        }
                    }
                    t => return Err(e(l.no)(format!("expected a type name but found {}", t))),
                }
                a.m.types.push(Desc::Error);
                attach(&mut a.m, &mut pending, Target::Type(tid));
            }
            "func" | "extern" => {
                if word == "extern" && !c.eat_word("func") {
                    return Err(e(l.no)("write extern func name(params)".into()));
                }
                let (name, params, names, _) = header(&mut a, &mut c, false).map_err(e(l.no))?;
                let id = a.m.funcs.len() as u32;
                a.func_names.entry(name.clone()).or_default().push(id);
                a.m.funcs.push(Function {
                    name,
                    params,
                    locals: params,
                    names,
                    external: word == "extern",
                    ..Function::default()
                });
                headers.push((i, id));
                attach(&mut a.m, &mut pending, Target::Func(id));
                if word == "extern" {
                    i += 1;
                    continue;
                }
                let start = i + 1;
                let mut j = start;
                while j < lines.len() && !matches!(lines[j].toks.as_slice(), [Tok::Ident(w)] if w == "end") {
                    if matches!(lines[j].toks.first(), Some(Tok::Ident(w)) if w == "func") {
                        return Err(e(lines[j].no)("func inside a function; close the previous one with end".into()));
                    }
                    j += 1;
                }
                if j == lines.len() {
                    return Err(e(l.no)(format!("function {} has no end", a.m.funcs[id as usize].name)));
                }
                bodies.push((id as usize, start, j));
                i = j;
            }
            "global" => {
                let name = c.name().map_err(e(l.no))?;
                c.done().map_err(e(l.no))?;
                let g = a.m.globals.len() as u32;
                a.global_names.entry(name.clone()).or_default().push(g);
                a.m.globals.push(name);
                attach(&mut a.m, &mut pending, Target::Global(g));
            }
            "import" => {
                let name = c.name().map_err(e(l.no))?;
                let argc = c.uint().map_err(e(l.no))?;
                c.done().map_err(e(l.no))?;
                if a.m.imports.iter().any(|x| x.name == name) {
                    return Err(e(l.no)(format!("import {} is declared twice", name)));
                }
                a.m.imports.push(Import { name, argc });
            }
            "table" => {
                let name = c.name().map_err(e(l.no))?;
                let argc = c.uint().map_err(e(l.no))?;
                a.table_names.entry(name.clone()).or_default().push(a.m.tables.len() as u32);
                a.m.tables.push(Table {
                    name,
                    argc,
                    entries: Vec::new(),
                });
            }
            "string" => {
                match c.next().map_err(e(l.no))? {
                    Tok::Str(s) => a.m.strings.push(s.clone()),
                    t => return Err(e(l.no)(format!("expected a string but found {}", t))),
                }
                c.done().map_err(e(l.no))?;
            }
            "loc" => {
                match c.next().map_err(e(l.no))? {
                    Tok::Str(s) => a.m.locs.push(s.clone()),
                    t => return Err(e(l.no)(format!("expected a string but found {}", t))),
                }
                c.done().map_err(e(l.no))?;
            }
            "entry" => {}
            "end" => return Err(e(l.no)("end without a matching func".into())),
            w => return Err(e(l.no)(format!("unknown directive {}", w))),
        }
        i += 1;
    }
    let mut next_type = FIRST_USER_TYPE;
    let mut next_table = 0;
    for l in &lines {
        let mut c = Cur::new(&l.toks);
        match &l.toks[0] {
            Tok::Ident(w) if w == "type" => {
                let decl = match c.t.get(1) {
                    Some(Tok::Ident(n)) => n.clone(),
                    _ => String::new(),
                };
                c.i = 2;
                c.expect('=').map_err(e(l.no))?;
                let d = a.def(&mut c, &decl).map_err(e(l.no))?;
                c.done().map_err(e(l.no))?;
                a.m.types[next_type as usize] = d;
                next_type += 1;
            }
            Tok::Ident(w) if w == "table" => {
                c.i = 3;
                c.expect('{').map_err(e(l.no))?;
                let mut entries = Vec::new();
                while !c.eat('}') {
                    let t = a.ty(&mut c).map_err(e(l.no))?;
                    c.expect(':').map_err(e(l.no))?;
                    let f = a.func_ref(&mut c).map_err(e(l.no))?;
                    entries.push((t, f));
                    if !c.eat(',') {
                        c.expect('}').map_err(e(l.no))?;
                        break;
                    }
                }
                c.done().map_err(e(l.no))?;
                a.m.tables[next_table].entries = entries;
                next_table += 1;
            }
            Tok::Ident(w) if w == "entry" => {
                c.i = 1;
                let f = a.func_ref(&mut c).map_err(e(l.no))?;
                c.done().map_err(e(l.no))?;
                a.m.entry = Some(f);
            }
            _ => {}
        }
    }
    if !pending.is_empty() {
        return Err(e(lines.last().map(|l| l.no).unwrap_or(0))(
            "an annotation at the end of the file has nothing to attach to".into(),
        ));
    }
    for (li, id) in headers {
        let l = &lines[li];
        let mut c = Cur::new(&l.toks);
        c.i = if matches!(l.toks.first(), Some(Tok::Ident(w)) if w == "extern") {
            2
        } else {
            1
        };
        let (_, _, _, sig) = header(&mut a, &mut c, true).map_err(e(l.no))?;
        a.m.funcs[id as usize].sig = sig;
    }
    for (id, start, end) in bodies {
        body(&mut a, id, &lines[start..end])?;
    }
    if a.m.entry.is_none() {
        a.m.entry = a.func_names.get("main").and_then(|v| v.first().copied());
    }
    Ok(a.m)
}

fn attach(m: &mut Module, pending: &mut Vec<(String, Vec<(String, Value)>)>, target: Target) {
    for (name, args) in pending.drain(..) {
        m.annotations.push(Annotation { target, name, args });
    }
}

fn annotation_value(c: &mut Cur) -> R<Value> {
    let neg = c.eat('-');
    Ok(match c.next()? {
        Tok::Str(s) if !neg => Value::Str(s.clone()),
        Tok::Int(v) => {
            let v = if neg { -*v } else { *v };
            Value::Int(i64::try_from(v).map_err(|_| format!("{} does not fit in 64 bits", v))?)
        }
        Tok::Float(f) => Value::Float(if neg { -*f } else { *f }),
        Tok::Ident(w) if w == "true" && !neg => Value::Bool(true),
        Tok::Ident(w) if w == "false" && !neg => Value::Bool(false),
        t => return Err(format!("expected a string, number or bool but found {}", t)),
    })
}

fn annotation_args(c: &mut Cur) -> R<Vec<(String, Value)>> {
    let mut out = Vec::new();
    while c.peek().is_some() {
        if matches!(c.t.get(c.i + 1), Some(Tok::P('='))) {
            let key = c.name()?;
            c.expect('=')?;
            out.push((key, annotation_value(c)?));
        } else {
            if !out.is_empty() {
                return Err("write the remaining annotation arguments as key=value".into());
            }
            out.push(("value".to_string(), annotation_value(c)?));
        }
    }
    Ok(out)
}

fn skip_type(c: &mut Cur, stop_at_close: bool) -> R<()> {
    let mut depth = 0i32;
    let start = c.i;
    while let Some(t) = c.peek() {
        match t {
            Tok::P('(') | Tok::P('[') | Tok::P('{') | Tok::P('<') => depth += 1,
            Tok::P(')') if depth == 0 && stop_at_close => break,
            Tok::P(',') if depth == 0 && stop_at_close => break,
            Tok::P(')') | Tok::P(']') | Tok::P('}') | Tok::P('>') => depth -= 1,
            _ => {}
        }
        c.i += 1;
    }
    if c.i == start {
        return Err("expected a type".into());
    }
    Ok(())
}

type Header = (String, u32, Vec<String>, Option<Sig>);

fn header(a: &mut Asm, c: &mut Cur, resolve: bool) -> R<Header> {
    let name = c.name()?;
    c.expect('(')?;
    let mut names = Vec::new();
    let mut types: Vec<Option<u32>> = Vec::new();
    if let Some(Tok::Int(n)) = c.peek() {
        let n = u32::try_from(*n).map_err(|_| "bad parameter count".to_string())?;
        c.i += 1;
        c.expect(')')?;
        types = vec![None; n as usize];
    } else {
        while !c.eat(')') {
            match c.next()? {
                Tok::Ident(n) => names.push(n.clone()),
                t => return Err(format!("expected a parameter name but found {}", t)),
            }
            if c.eat(':') {
                if resolve {
                    types.push(Some(a.ty(c)?));
                } else {
                    skip_type(c, true)?;
                    types.push(Some(0));
                }
            } else {
                types.push(None);
            }
            if !c.eat(',') {
                c.expect(')')?;
                break;
            }
        }
    }
    let ret = if c.eat(':') {
        if resolve {
            Some(a.ty(c)?)
        } else {
            skip_type(c, false)?;
            Some(0)
        }
    } else {
        None
    };
    c.done()?;
    let params = types.len() as u32;
    if names.iter().all(|n| n == "_") {
        names.clear();
    } else {
        for (i, n) in names.iter_mut().enumerate() {
            if n == "_" {
                *n = format!("v{}", i);
            }
        }
    }
    let typed = types.iter().any(|t| t.is_some()) || ret.is_some();
    let sig = if typed {
        if types.iter().any(|t| t.is_none()) {
            return Err("give every parameter a type, or none of them".into());
        }
        Some(Sig {
            params: types.into_iter().map(|t| t.unwrap()).collect(),
            ret: ret.unwrap_or(1),
        })
    } else {
        None
    };
    Ok((name, params, names, sig))
}

fn body(a: &mut Asm, id: usize, lines: &[Line]) -> Result<(), AsmError> {
    let e = |line: usize| move |msg: String| AsmError { line, msg };
    let mut names = a.m.funcs[id].names.clone();
    let mut locals = a.m.funcs[id].params;
    let mut labels: HashMap<String, u32> = HashMap::new();
    let mut count = 0u32;
    for l in lines {
        let mut t = l.toks.as_slice();
        while let [Tok::Ident(n), Tok::P(':'), rest @ ..] = t {
            if labels.insert(n.clone(), count).is_some() {
                return Err(e(l.no)(format!("label {} is defined twice", n)));
            }
            t = rest;
        }
        match t.first() {
            None => {}
            Some(Tok::Ident(w)) if w == "local" || w == "locals" => {}
            Some(_) => count += 1,
        }
    }
    let mut code = Vec::with_capacity(count as usize);
    for l in lines {
        let mut t = l.toks.as_slice();
        while let [Tok::Ident(_), Tok::P(':'), rest @ ..] = t {
            t = rest;
        }
        if t.is_empty() {
            continue;
        }
        let mut c = Cur::new(t);
        let m = match c.next().map_err(e(l.no))? {
            Tok::Ident(m) => m.clone(),
            t => return Err(e(l.no)(format!("expected an instruction but found {}", t))),
        };
        if m == "local" {
            let n = c.name().map_err(e(l.no))?;
            if names.len() < locals as usize {
                names.extend((names.len()..locals as usize).map(|i| format!("v{}", i)));
            }
            names.push(n);
            locals += 1;
            c.done().map_err(e(l.no))?;
            continue;
        }
        if m == "locals" {
            let n = c.uint().map_err(e(l.no))?;
            locals += n;
            if !names.is_empty() {
                names.extend((names.len()..locals as usize).map(|i| format!("v{}", i)));
            }
            c.done().map_err(e(l.no))?;
            continue;
        }
        let op = instr(a, &m, &mut c, &names, &labels).map_err(e(l.no))?;
        c.done().map_err(e(l.no))?;
        code.push(op);
    }
    let f = &mut a.m.funcs[id];
    f.locals = locals;
    f.names = names;
    f.code = code;
    Ok(())
}

fn instr(a: &mut Asm, m: &str, c: &mut Cur, names: &[String], labels: &HashMap<String, u32>) -> R<Op> {
    let local = |c: &mut Cur| -> R<u32> {
        match c.next()? {
            Tok::Int(n) if *n >= 0 && *n <= u32::MAX as i128 => Ok(*n as u32),
            Tok::Ident(s) => names
                .iter()
                .rposition(|x| x == s)
                .map(|i| i as u32)
                .ok_or_else(|| format!("unknown local {}", s)),
            t => Err(format!("expected a local but found {}", t)),
        }
    };
    let label = |c: &mut Cur| -> R<u32> {
        match c.next()? {
            Tok::Ident(s) => labels.get(s).copied().ok_or_else(|| format!("unknown label {}", s)),
            Tok::Int(n) if *n >= 0 && *n <= u32::MAX as i128 => Ok(*n as u32),
            t => Err(format!("expected a label but found {}", t)),
        }
    };
    if let Some(op) = Op::simple(m) {
        return Ok(op);
    }
    Ok(match m {
        "const" => Op::Const(const_operand(c)?),
        "tconst" => Op::TypeConst(a.ty(c)?),
        "lconst" => Op::LocConst(c.uint()?),
        "str" => match c.next()? {
            Tok::Str(s) => Op::Str(a.m.intern_string(s)),
            Tok::At(n) => Op::Str(*n),
            t => return Err(format!("expected a string but found {}", t)),
        },
        "fref" => Op::FuncRef(a.func_ref(c)?),
        "load" => Op::Load(local(c)?),
        "store" => Op::Store(local(c)?),
        "tee" => Op::Tee(local(c)?),
        "gload" => Op::GLoad(a.global_ref(c)?),
        "gstore" => Op::GStore(a.global_ref(c)?),
        "gtee" => Op::GTee(a.global_ref(c)?),
        "idiv" => Op::IDiv(loc_operand(c)?),
        "iadd.ovf" => Op::IAddOv(loc_operand(c)?),
        "isub.ovf" => Op::ISubOv(loc_operand(c)?),
        "imul.ovf" => Op::IMulOv(loc_operand(c)?),
        "ineg.ovf" => Op::INegOv(loc_operand(c)?),
        "irem" => Op::IRem(loc_operand(c)?),
        "index" => Op::Index(loc_operand(c)?),
        "setindex" => Op::SetIndex(loc_operand(c)?),
        "jmp" => Op::Jmp(label(c)?),
        "jz" => Op::Jz(label(c)?),
        "jnz" => Op::Jnz(label(c)?),
        "jzk" => Op::JzKeep(label(c)?),
        "jnzk" => Op::JnzKeep(label(c)?),
        "call" => Op::Call(a.func_ref(c)?),
        "calli" => Op::CallInd(c.uint()?),
        "dispatch" => {
            let t = a.table_ref(c)?;
            let argc =
                a.m.tables
                    .get(t as usize)
                    .map(|t| t.argc)
                    .ok_or_else(|| format!("table @{} does not exist", t))?;
            Op::Dispatch(t, argc)
        }
        "rt" => {
            let n = c.name()?;
            Op::Rt(rt_by_name(&n).ok_or_else(|| format!("unknown runtime function {}; run `bvm runtime` for the list", n))?)
        }
        "host" => Op::Host(a.import_ref(c)?),
        "spawn" => {
            let f = a.func_ref(c)?;
            let t = a.ty(c)?;
            let argc =
                a.m.funcs
                    .get(f as usize)
                    .map(|f| f.params)
                    .ok_or_else(|| format!("function @{} does not exist", f))?;
            Op::Spawn(f, argc, t)
        }
        "new" => {
            let t = a.ty(c)?;
            let n = match a.m.types.get(t as usize) {
                Some(Desc::Record { fields, .. }) => fields.len() as u32,
                _ => return Err(format!("new needs a record type, not #{}", t)),
            };
            Op::NewRecord(t, n)
        }
        "newarr" => {
            let t = a.ty(c)?;
            Op::NewArray(t, c.uint()?)
        }
        "getf" | "setf" => {
            let i = match c.next()? {
                Tok::Int(n) if *n >= 0 && *n <= u32::MAX as i128 => *n as u32,
                Tok::Ident(s) => field_index(a, s)?,
                t => return Err(format!("expected a field but found {}", t)),
            };
            if m == "getf" {
                Op::GetField(i)
            } else {
                Op::SetField(i)
            }
        }
        _ => return Err(format!("unknown instruction {}", m)),
    })
}

fn field_index(a: &Asm, s: &str) -> R<u32> {
    let (ty, field) = s.rsplit_once('.').ok_or_else(|| format!("write the field as Type.field, not {}", s))?;
    let tid = *a.type_names.get(ty).ok_or_else(|| format!("unknown type {}", ty))?;
    match &a.m.types[tid as usize] {
        Desc::Record { fields, .. } => fields
            .iter()
            .position(|f| f.0 == field)
            .map(|i| i as u32)
            .ok_or_else(|| format!("{} has no field {}", ty, field)),
        _ => Err(format!("{} is not a record type", ty)),
    }
}

struct Names {
    types: HashMap<u32, String>,
    funcs: Vec<String>,
    globals: Vec<String>,
    tables: Vec<String>,
    imports: Vec<String>,
}

fn quoted(s: &str) -> String {
    if is_ident(s) {
        s.to_string()
    } else {
        format!("\"{}\"", escape(s))
    }
}

fn unique_refs<'a>(items: impl Iterator<Item = &'a str>, count: usize) -> Vec<String> {
    let items: Vec<&str> = items.collect();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for s in &items {
        *seen.entry(s).or_default() += 1;
    }
    (0..count)
        .map(|i| {
            let s = items[i];
            if seen[s] == 1 && !s.is_empty() {
                quoted(s)
            } else {
                format!("@{}", i)
            }
        })
        .collect()
}

fn type_decl_name(d: &Desc) -> Option<&str> {
    match d {
        Desc::Record { name, .. } | Desc::Interface { name } | Desc::Enum { name, .. } if !name.is_empty() => Some(name),
        _ => None,
    }
}

impl Names {
    fn new(m: &Module) -> Names {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for d in m.types.iter().skip(FIRST_USER_TYPE as usize) {
            if let Some(n) = type_decl_name(d) {
                *counts.entry(n).or_default() += 1;
            }
        }
        let mut types = HashMap::new();
        for (i, d) in m.types.iter().enumerate().skip(FIRST_USER_TYPE as usize) {
            if let Some(n) = type_decl_name(d) {
                if counts[n] == 1 && is_type_ident(n) {
                    types.insert(i as u32, n.to_string());
                }
            }
        }
        Names {
            types,
            funcs: unique_refs(m.funcs.iter().map(|f| f.name.as_str()), m.funcs.len()),
            globals: unique_refs(m.globals.iter().map(|s| s.as_str()), m.globals.len()),
            tables: unique_refs(m.tables.iter().map(|t| t.name.as_str()), m.tables.len()),
            imports: m.imports.iter().map(|i| quoted(&i.name)).collect(),
        }
    }

    fn ty(&self, m: &Module, t: u32) -> String {
        if t < FIRST_USER_TYPE {
            return match m.types.get(t as usize) {
                Some(d) if t as usize >= BUILTIN_TYPES.len() => self.desc(m, d, ""),
                _ => BUILTIN_TYPES.get(t as usize).map(|s| s.to_string()).unwrap_or_else(|| format!("#{}", t)),
            };
        }
        match self.types.get(&t) {
            Some(n) => n.clone(),
            None => format!("#{}", t),
        }
    }

    fn desc(&self, m: &Module, d: &Desc, decl: &str) -> String {
        let named = |kind: &str, name: &str| {
            if name.is_empty() || name == decl {
                kind.to_string()
            } else {
                format!("{} \"{}\"", kind, escape(name))
            }
        };
        match d {
            Desc::Error => "error".into(),
            Desc::Void => "void".into(),
            Desc::Null => "null".into(),
            Desc::Int => "int".into(),
            Desc::Float => "float".into(),
            Desc::Bool => "bool".into(),
            Desc::Str => "string".into(),
            Desc::Any => "any".into(),
            Desc::Func => "fun".into(),
            Desc::Num(n) => n.name().into(),
            Desc::Array(e) => format!("[{}]", self.ty(m, *e)),
            Desc::Map(k, v) => format!("map<{}, {}>", self.ty(m, *k), self.ty(m, *v)),
            Desc::Optional(t) => format!("{}?", self.ty(m, *t)),
            Desc::Future(t) => format!("future<{}>", self.ty(m, *t)),
            Desc::Interface { name } => named("interface", name),
            Desc::Enum { name, variants } => {
                let vs: Vec<String> = variants.iter().map(|v| quoted(v)).collect();
                format!("{} {{ {} }}", named("enum", name), vs.join(", "))
            }
            Desc::Record {
                name,
                fields,
                class,
                implements,
            } => {
                let fs: Vec<String> = fields.iter().map(|(n, t)| format!("{}: {}", quoted(n), self.ty(m, *t))).collect();
                let mut s = format!("{} {{ {} }}", named(if *class { "struct" } else { "record" }, name), fs.join(", "));
                if fs.is_empty() {
                    s = format!("{} {{}}", named(if *class { "struct" } else { "record" }, name));
                }
                if !implements.is_empty() {
                    let is: Vec<String> = implements.iter().map(|t| self.ty(m, *t)).collect();
                    let _ = write!(s, " implements {}", is.join(", "));
                }
                s
            }
        }
    }
}

fn const_text(v: u64) -> String {
    let i = v as i64;
    let fv = f64::from_bits(v);
    if i.unsigned_abs() > (1u64 << 53) && fv.is_finite() && fv != 0.0 {
        let text = format!("{:?}", fv);
        if text.parse::<f64>().map(|x| x.to_bits()) == Ok(v) {
            return text;
        }
        format!("{}", i)
    } else {
        format!("{}", i)
    }
}

pub fn disassemble(m: &Module) -> String {
    let n = Names::new(m);
    let mut o = String::new();
    let _ = writeln!(o, "; bvm module, format {}", crate::FORMAT_VERSION);
    let mut sections = 0;
    if !m.name.is_empty() || m.annotations_of(Target::Module).next().is_some() {
        annotations(&mut o, m, Target::Module, "");
        let _ = writeln!(o, "module {}", quoted(&m.name));
        sections = 1;
    }
    let mut gap = |o: &mut String, had: bool| {
        if had && sections > 0 {
            o.push('\n');
        }
        if had {
            sections += 1;
        }
    };
    let has = m.types.len() > FIRST_USER_TYPE as usize;
    gap(&mut o, has);
    for (i, d) in m.types.iter().enumerate().skip(FIRST_USER_TYPE as usize) {
        let decl = n.types.get(&(i as u32)).cloned();
        let label = decl.clone().unwrap_or_else(|| format!("#{}", i));
        annotations(&mut o, m, Target::Type(i as u32), "");
        let _ = writeln!(o, "type {} = {}", label, n.desc(m, d, decl.as_deref().unwrap_or("")));
    }
    gap(&mut o, !m.strings.is_empty());
    for s in &m.strings {
        let _ = writeln!(o, "string \"{}\"", escape(s));
    }
    gap(&mut o, !m.locs.is_empty());
    for s in &m.locs {
        let _ = writeln!(o, "loc \"{}\"", escape(s));
    }
    gap(&mut o, !m.globals.is_empty());
    for (i, g) in m.globals.iter().enumerate() {
        annotations(&mut o, m, Target::Global(i as u32), "");
        let _ = writeln!(o, "global {}", quoted(g));
    }
    gap(&mut o, !m.imports.is_empty());
    for i in &m.imports {
        let _ = writeln!(o, "import {} {}", quoted(&i.name), i.argc);
    }
    gap(&mut o, !m.tables.is_empty());
    for t in &m.tables {
        let es: Vec<String> = t.entries.iter().map(|(tid, f)| format!("{}: {}", n.ty(m, *tid), fref(&n, *f))).collect();
        if es.is_empty() {
            let _ = writeln!(o, "table {} {} {{}}", quoted(&t.name), t.argc);
        } else {
            let _ = writeln!(o, "table {} {} {{ {} }}", quoted(&t.name), t.argc, es.join(", "));
        }
    }
    gap(&mut o, m.entry.is_some());
    if let Some(e) = m.entry {
        let _ = writeln!(o, "entry {}", n.funcs.get(e as usize).cloned().unwrap_or_else(|| format!("@{}", e)));
    }
    let externs: Vec<usize> = (0..m.funcs.len()).filter(|i| m.funcs[*i].external).collect();
    gap(&mut o, !externs.is_empty());
    for i in externs {
        annotations(&mut o, m, Target::Func(i as u32), "");
        let _ = writeln!(o, "extern func {}", head(m, &n, &m.funcs[i], false));
    }
    for (i, f) in m.funcs.iter().enumerate() {
        if f.external {
            continue;
        }
        if sections > 0 {
            o.push('\n');
        }
        sections += 1;
        annotations(&mut o, m, Target::Func(i as u32), "");
        func(&mut o, m, &n, f);
    }
    o
}

fn value_text(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format!("{:?}", f),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => format!("\"{}\"", escape(s)),
    }
}

fn annotations(o: &mut String, m: &Module, target: Target, indent: &str) {
    for a in m.annotations_of(target) {
        let _ = write!(o, "{}@{}", indent, a.name);
        if let [(k, v)] = a.args.as_slice() {
            if k == "value" {
                let _ = writeln!(o, " {}", value_text(v));
                continue;
            }
        }
        for (k, v) in &a.args {
            let _ = write!(o, " {}={}", quoted(k), value_text(v));
        }
        o.push('\n');
    }
}

fn names_usable(f: &Function) -> bool {
    let set: HashSet<&str> = f.names.iter().map(|s| s.as_str()).collect();
    f.params <= f.locals
        && f.names.len() == f.locals as usize
        && set.len() == f.names.len()
        && f.names.iter().all(|s| is_ident(s) && Op::simple(s).is_none() && s != "_")
}

fn head(m: &Module, n: &Names, f: &Function, named: bool) -> String {
    let named = named || (f.external && names_usable(f));
    let pname = |i: usize| if named { f.names[i].clone() } else { "_".to_string() };
    match &f.sig {
        Some(sig) => {
            let ps: Vec<String> = (0..f.params as usize)
                .map(|i| format!("{}: {}", pname(i), sig.params.get(i).map(|t| n.ty(m, *t)).unwrap_or_else(|| "any".into())))
                .collect();
            format!("{}({}): {}", quoted(&f.name), ps.join(", "), n.ty(m, sig.ret))
        }
        None if named => {
            let ps: Vec<String> = (0..f.params as usize).map(pname).collect();
            format!("{}({})", quoted(&f.name), ps.join(", "))
        }
        None => format!("{}({})", quoted(&f.name), f.params),
    }
}

fn func(o: &mut String, m: &Module, n: &Names, f: &Function) {
    let named = names_usable(f);
    let _ = writeln!(o, "func {}", head(m, n, f, named));
    if named {
        for l in &f.names[f.params as usize..] {
            let _ = writeln!(o, "    local {}", l);
        }
    } else {
        if f.locals > f.params {
            let _ = writeln!(o, "    locals {}", f.locals - f.params);
        }
    }
    let local = |s: u32| {
        if named {
            f.names.get(s as usize).cloned().unwrap_or_else(|| s.to_string())
        } else {
            s.to_string()
        }
    };
    let mut targets: BTreeMap<u32, String> = BTreeMap::new();
    for op in &f.code {
        if let Some(t) = op.jump_target() {
            targets.insert(t, String::new());
        }
    }
    for (i, v) in targets.values_mut().enumerate() {
        *v = format!("L{}", i);
    }
    let mut first_str: HashMap<&str, usize> = HashMap::new();
    for (i, s) in m.strings.iter().enumerate() {
        first_str.entry(s.as_str()).or_insert(i);
    }
    let loc = |l: u32| if l == NO_LOC { String::new() } else { format!(" {}", l) };
    for (i, op) in f.code.iter().enumerate() {
        if let Some(l) = targets.get(&(i as u32)) {
            let _ = writeln!(o, "{}:", l);
        }
        let mn = op.mnemonic();
        let arg = match *op {
            Op::Const(v) => format!(" {}", const_text(v)),
            Op::TypeConst(t) => format!(" {}", n.ty(m, t)),
            Op::LocConst(l) => format!(" {}", l),
            Op::Str(s) => match m.strings.get(s as usize) {
                Some(text) if first_str.get(text.as_str()) == Some(&(s as usize)) => format!(" \"{}\"", escape(text)),
                _ => format!(" @{}", s),
            },
            Op::FuncRef(x) | Op::Call(x) => format!(" {}", fref(n, x)),
            Op::Load(s) | Op::Store(s) | Op::Tee(s) => format!(" {}", local(s)),
            Op::GLoad(g) | Op::GStore(g) | Op::GTee(g) => {
                format!(" {}", n.globals.get(g as usize).cloned().unwrap_or_else(|| format!("@{}", g)))
            }
            Op::IDiv(l) | Op::IRem(l) | Op::Index(l) | Op::SetIndex(l) | Op::IAddOv(l) | Op::ISubOv(l) | Op::IMulOv(l) | Op::INegOv(l) => loc(l),
            Op::Jmp(t) | Op::Jz(t) | Op::Jnz(t) | Op::JzKeep(t) | Op::JnzKeep(t) => format!(" {}", targets[&t]),
            Op::CallInd(k) => format!(" {}", k),
            Op::Dispatch(t, _) => format!(" {}", n.tables.get(t as usize).cloned().unwrap_or_else(|| format!("@{}", t))),
            Op::Rt(r) => format!(" {}", rt_name(r)),
            Op::Host(h) => format!(" {}", n.imports.get(h as usize).cloned().unwrap_or_else(|| format!("@{}", h))),
            Op::Spawn(fi, _, t) => format!(" {} {}", fref(n, fi), n.ty(m, t)),
            Op::NewRecord(t, _) => format!(" {}", n.ty(m, t)),
            Op::NewArray(t, k) => format!(" {} {}", n.ty(m, t), k),
            Op::GetField(i) | Op::SetField(i) => format!(" {}", i),
            Op::IncLocal(a, k) => format!(" {} {}", a, k),
            Op::JCmpLL(c, a, b, t) => format!(" {} {} {} {}", c.suffix(), a, b, t),
            Op::JCmpLC(c, a, k, t) => format!(" {} {} {} {}", c.suffix(), a, k, t),
            Op::LoadField(a, i) | Op::Load2(a, i) => format!(" {} {}", a, i),
            Op::LoadK(a, k) => format!(" {} {}", a, k),
            _ => String::new(),
        };
        let _ = writeln!(o, "    {}{}", mn, arg);
    }
    if let Some(l) = targets.get(&(f.code.len() as u32)) {
        let _ = writeln!(o, "{}:", l);
    }
    o.push_str("end\n");
}

fn fref(n: &Names, f: u32) -> String {
    n.funcs.get(f as usize).cloned().unwrap_or_else(|| format!("@{}", f))
}

pub fn cmp_names() -> Vec<&'static str> {
    Cmp::ALL.iter().map(|c| c.suffix()).collect()
}

use crate::diag::Diagnostic;
use crate::source::{FileId, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Template(Vec<TplPart>),
    Fun,
    Var,
    Const,
    Def,
    If,
    Else,
    While,
    For,
    In,
    Return,
    Break,
    Continue,
    True,
    False,
    Null,
    Import,
    Pub,
    Priv,
    Async,
    Await,
    Is,
    As,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    EqEq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Colon,
    ColonColon,
    Dot,
    DotDot,
    DotDotEq,
    Question,
    Arrow,
    At,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TplPart {
    Lit(String),
    Expr(Vec<Token>, Span),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: Tok,
    pub span: Span,
    pub nl_before: bool,
}

pub fn keyword(s: &str) -> Option<Tok> {
    Some(match s {
        "fun" => Tok::Fun,
        "var" => Tok::Var,
        "const" => Tok::Const,
        "def" => Tok::Def,
        "if" => Tok::If,
        "else" => Tok::Else,
        "while" => Tok::While,
        "for" => Tok::For,
        "in" => Tok::In,
        "return" => Tok::Return,
        "break" => Tok::Break,
        "continue" => Tok::Continue,
        "true" => Tok::True,
        "false" => Tok::False,
        "null" => Tok::Null,
        "import" => Tok::Import,
        "pub" => Tok::Pub,
        "priv" => Tok::Priv,
        "async" => Tok::Async,
        "await" => Tok::Await,
        "is" => Tok::Is,
        "as" => Tok::As,
        _ => return None,
    })
}

pub fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(s) => format!("identifier `{}`", s),
        Tok::Int(v) => format!("number `{}`", v),
        Tok::Float(v) => format!("number `{}`", v),
        Tok::Str(_) | Tok::Template(_) => "string literal".into(),
        Tok::Eof => "end of file".into(),
        other => format!("`{}`", symbol(other)),
    }
}

pub fn symbol(t: &Tok) -> &'static str {
    match t {
        Tok::Fun => "fun",
        Tok::Var => "var",
        Tok::Const => "const",
        Tok::Def => "def",
        Tok::If => "if",
        Tok::Else => "else",
        Tok::While => "while",
        Tok::For => "for",
        Tok::In => "in",
        Tok::Return => "return",
        Tok::Break => "break",
        Tok::Continue => "continue",
        Tok::True => "true",
        Tok::False => "false",
        Tok::Null => "null",
        Tok::Import => "import",
        Tok::Pub => "pub",
        Tok::Priv => "priv",
        Tok::Async => "async",
        Tok::Await => "await",
        Tok::Is => "is",
        Tok::As => "as",
        Tok::Plus => "+",
        Tok::Minus => "-",
        Tok::Star => "*",
        Tok::Slash => "/",
        Tok::Percent => "%",
        Tok::Assign => "=",
        Tok::PlusEq => "+=",
        Tok::MinusEq => "-=",
        Tok::StarEq => "*=",
        Tok::SlashEq => "/=",
        Tok::PercentEq => "%=",
        Tok::EqEq => "==",
        Tok::NotEq => "!=",
        Tok::Lt => "<",
        Tok::Gt => ">",
        Tok::Le => "<=",
        Tok::Ge => ">=",
        Tok::AndAnd => "&&",
        Tok::OrOr => "||",
        Tok::Bang => "!",
        Tok::LParen => "(",
        Tok::RParen => ")",
        Tok::LBrace => "{",
        Tok::RBrace => "}",
        Tok::LBracket => "[",
        Tok::RBracket => "]",
        Tok::Comma => ",",
        Tok::Semi => ";",
        Tok::Colon => ":",
        Tok::ColonColon => "::",
        Tok::Dot => ".",
        Tok::DotDot => "..",
        Tok::DotDotEq => "..=",
        Tok::Question => "?",
        Tok::Arrow => "->",
        Tok::At => "@",
        _ => "?",
    }
}

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    end: usize,
    file: FileId,
    pub tokens: Vec<Token>,
    pub diags: Vec<Diagnostic>,
    nl: bool,
}

pub fn lex(src: &str, file: FileId) -> (Vec<Token>, Vec<Diagnostic>) {
    lex_range(src, file, 0, src.len())
}

pub fn lex_range(src: &str, file: FileId, start: usize, end: usize) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut l = Lexer {
        src,
        bytes: src.as_bytes(),
        pos: start,
        end,
        file,
        tokens: Vec::new(),
        diags: Vec::new(),
        nl: true,
    };
    l.run();
    (l.tokens, l.diags)
}

impl<'a> Lexer<'a> {
    fn peek(&self, off: usize) -> u8 {
        if self.pos + off < self.end {
            self.bytes[self.pos + off]
        } else {
            0
        }
    }

    fn span(&self, start: usize) -> Span {
        Span::new(self.file, start, self.pos)
    }

    fn push(&mut self, kind: Tok, start: usize) {
        let span = self.span(start);
        self.tokens.push(Token {
            kind,
            span,
            nl_before: self.nl,
        });
        self.nl = false;
    }

    fn error(&mut self, start: usize, msg: impl Into<String>) {
        let span = Span::new(self.file, start, self.pos.max(start + 1).min(self.end.max(start)));
        self.diags.push(Diagnostic::error(span, msg));
    }

    fn run(&mut self) {
        if self.pos == 0 && self.src.starts_with("#!") {
            while self.pos < self.end && self.bytes[self.pos] != b'\n' {
                self.pos += 1;
            }
        }
        loop {
            self.skip_trivia();
            if self.pos >= self.end {
                let p = self.end;
                self.tokens.push(Token {
                    kind: Tok::Eof,
                    span: Span::new(self.file, p, p),
                    nl_before: true,
                });
                return;
            }
            let start = self.pos;
            let c = self.bytes[self.pos];
            if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
                self.ident(start);
                continue;
            }
            if c.is_ascii_digit() {
                self.number(start);
                continue;
            }
            if c == b'"' || c == b'\'' {
                self.string(start, c);
                continue;
            }
            let two = [c, self.peek(1)];
            let (tok, len) = match &two {
                b"+=" => (Tok::PlusEq, 2),
                b"-=" => (Tok::MinusEq, 2),
                b"*=" => (Tok::StarEq, 2),
                b"/=" => (Tok::SlashEq, 2),
                b"%=" => (Tok::PercentEq, 2),
                b"==" => (Tok::EqEq, 2),
                b"!=" => (Tok::NotEq, 2),
                b"<=" => (Tok::Le, 2),
                b">=" => (Tok::Ge, 2),
                b"&&" => (Tok::AndAnd, 2),
                b"||" => (Tok::OrOr, 2),
                b"->" => (Tok::Arrow, 2),
                b".." => {
                    if self.peek(2) == b'=' {
                        (Tok::DotDotEq, 3)
                    } else {
                        (Tok::DotDot, 2)
                    }
                }
                _ => match c {
                    b'+' => (Tok::Plus, 1),
                    b'-' => (Tok::Minus, 1),
                    b'*' => (Tok::Star, 1),
                    b'/' => (Tok::Slash, 1),
                    b'%' => (Tok::Percent, 1),
                    b'=' => (Tok::Assign, 1),
                    b'<' => (Tok::Lt, 1),
                    b'>' => (Tok::Gt, 1),
                    b'!' => (Tok::Bang, 1),
                    b'(' => (Tok::LParen, 1),
                    b')' => (Tok::RParen, 1),
                    b'{' => (Tok::LBrace, 1),
                    b'}' => (Tok::RBrace, 1),
                    b'[' => (Tok::LBracket, 1),
                    b']' => (Tok::RBracket, 1),
                    b',' => (Tok::Comma, 1),
                    b';' => (Tok::Semi, 1),
                    b':' if self.src.as_bytes().get(self.pos + 1) == Some(&b':') => (Tok::ColonColon, 2),
                    b':' => (Tok::Colon, 1),
                    b'.' => (Tok::Dot, 1),
                    b'?' => (Tok::Question, 1),
                    b'@' => (Tok::At, 1),
                    _ => {
                        let ch = self.src[self.pos..].chars().next().unwrap_or('?');
                        self.pos += ch.len_utf8();
                        let span = Span::new(self.file, start, self.pos);
                        let mut d = Diagnostic::error(span, format!("unexpected character `{}`", ch));
                        if ch == '&' || ch == '|' {
                            let op = format!("{}{}", ch, ch);
                            d = d.fix(format!("use `{}` for logical {}", op, if ch == '&' { "and" } else { "or" }), span, op);
                            self.diags.push(d);
                            self.push(if ch == '&' { Tok::AndAnd } else { Tok::OrOr }, start);
                            continue;
                        }
                        self.diags.push(d);
                        continue;
                    }
                },
            };
            self.pos += len;
            self.push(tok, start);
        }
    }

    fn skip_trivia(&mut self) {
        while self.pos < self.end {
            let c = self.bytes[self.pos];
            match c {
                b'\n' => {
                    self.nl = true;
                    self.pos += 1;
                }
                b' ' | b'\t' | b'\r' => self.pos += 1,
                b'/' if self.peek(1) == b'/' => {
                    while self.pos < self.end && self.bytes[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                }
                b'/' if self.peek(1) == b'*' => {
                    let start = self.pos;
                    self.pos += 2;
                    let mut depth = 1;
                    while self.pos < self.end && depth > 0 {
                        if self.bytes[self.pos] == b'*' && self.peek(1) == b'/' {
                            depth -= 1;
                            self.pos += 2;
                        } else if self.bytes[self.pos] == b'/' && self.peek(1) == b'*' {
                            depth += 1;
                            self.pos += 2;
                        } else {
                            if self.bytes[self.pos] == b'\n' {
                                self.nl = true;
                            }
                            self.pos += 1;
                        }
                    }
                    if depth > 0 {
                        self.error(start, "unterminated block comment");
                    }
                }
                _ => return,
            }
        }
    }

    fn ident(&mut self, start: usize) {
        while self.pos < self.end {
            let ch = self.src[self.pos..].chars().next().unwrap();
            if ch.is_alphanumeric() || ch == '_' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        let text = &self.src[start..self.pos];
        let tok = keyword(text).unwrap_or_else(|| Tok::Ident(text.to_string()));
        self.push(tok, start);
    }

    fn number(&mut self, start: usize) {
        if self.bytes[self.pos] == b'0' && matches!(self.peek(1), b'x' | b'X' | b'b' | b'B' | b'o' | b'O') {
            let radix = match self.peek(1) {
                b'x' | b'X' => 16,
                b'b' | b'B' => 2,
                _ => 8,
            };
            self.pos += 2;
            let ds = self.pos;
            while self.pos < self.end && (self.bytes[self.pos].is_ascii_alphanumeric() || self.bytes[self.pos] == b'_') {
                self.pos += 1;
            }
            let digits: String = self.src[ds..self.pos].chars().filter(|c| *c != '_').collect();
            match i64::from_str_radix(&digits, radix).or_else(|_| u64::from_str_radix(&digits, radix).map(|v| v as i64)) {
                Ok(v) => self.push(Tok::Int(v), start),
                Err(_) => {
                    self.error(start, "invalid number literal");
                    self.push(Tok::Int(0), start);
                }
            }
            return;
        }
        let mut float = false;
        while self.pos < self.end && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'_') {
            self.pos += 1;
        }
        if self.peek(0) == b'.' && self.peek(1).is_ascii_digit() {
            float = true;
            self.pos += 1;
            while self.pos < self.end && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'_') {
                self.pos += 1;
            }
        }
        if matches!(self.peek(0), b'e' | b'E') && (self.peek(1).is_ascii_digit() || (matches!(self.peek(1), b'+' | b'-') && self.peek(2).is_ascii_digit())) {
            float = true;
            self.pos += 2;
            while self.pos < self.end && self.bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let text: String = self.src[start..self.pos].chars().filter(|c| *c != '_').collect();
        if float {
            self.push(Tok::Float(text.parse().unwrap_or(0.0)), start);
        } else {
            match text.parse::<i64>() {
                Ok(v) => self.push(Tok::Int(v), start),
                Err(_) => match text.parse::<f64>() {
                    Ok(v) => {
                        self.error(start, "integer literal is too large");
                        self.push(Tok::Float(v), start)
                    }
                    Err(_) => {
                        self.error(start, "invalid number literal");
                        self.push(Tok::Int(0), start)
                    }
                },
            }
        }
    }

    fn string(&mut self, start: usize, quote: u8) {
        self.pos += 1;
        let mut parts: Vec<TplPart> = Vec::new();
        let mut cur = String::new();
        let mut closed = false;
        while self.pos < self.end {
            let c = self.bytes[self.pos];
            if c == quote {
                self.pos += 1;
                closed = true;
                break;
            }
            if c == b'\n' {
                break;
            }
            if c == b'\\' {
                let esc_start = self.pos;
                self.pos += 1;
                let e = self.peek(0);
                self.pos += 1;
                match e {
                    b'n' => cur.push('\n'),
                    b't' => cur.push('\t'),
                    b'r' => cur.push('\r'),
                    b'0' => cur.push('\0'),
                    b'\\' => cur.push('\\'),
                    b'"' => cur.push('"'),
                    b'\'' => cur.push('\''),
                    b'$' => cur.push('$'),
                    b'u' => {
                        if self.peek(0) == b'{' {
                            let hs = self.pos + 1;
                            let mut he = hs;
                            while he < self.end && self.bytes[he] != b'}' {
                                he += 1;
                            }
                            let code = u32::from_str_radix(&self.src[hs..he], 16).ok().and_then(char::from_u32);
                            self.pos = (he + 1).min(self.end);
                            match code {
                                Some(ch) => cur.push(ch),
                                None => self.error(esc_start, "invalid unicode escape"),
                            }
                        } else {
                            self.error(esc_start, "expected `{` after `\\u`");
                        }
                    }
                    _ => {
                        self.error(esc_start, "unknown escape sequence");
                    }
                }
                continue;
            }
            if c == b'$' && self.peek(1) == b'{' {
                let expr_start = self.pos + 2;
                let mut i = expr_start;
                let mut depth = 1;
                while i < self.end && depth > 0 {
                    match self.bytes[i] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        b'"' | b'\'' => {
                            let q = self.bytes[i];
                            i += 1;
                            while i < self.end && self.bytes[i] != q && self.bytes[i] != b'\n' {
                                if self.bytes[i] == b'\\' {
                                    i += 1;
                                }
                                i += 1;
                            }
                        }
                        b'\n' => break,
                        _ => {}
                    }
                    if depth > 0 {
                        i += 1;
                    }
                }
                if depth != 0 {
                    self.pos = i;
                    self.error(start, "unterminated `${` in string template");
                    break;
                }
                if !cur.is_empty() {
                    parts.push(TplPart::Lit(std::mem::take(&mut cur)));
                }
                let (mut toks, diags) = lex_range(self.src, self.file, expr_start, i);
                self.diags.extend(diags);
                if let Some(t) = toks.first_mut() {
                    t.nl_before = false;
                }
                parts.push(TplPart::Expr(toks, Span::new(self.file, expr_start, i)));
                self.pos = i + 1;
                continue;
            }
            let ch = self.src[self.pos..].chars().next().unwrap();
            cur.push(ch);
            self.pos += ch.len_utf8();
        }
        if !closed {
            self.error(start, "unterminated string literal");
        }
        if parts.is_empty() {
            self.push(Tok::Str(cur), start);
        } else {
            if !cur.is_empty() {
                parts.push(TplPart::Lit(cur));
            }
            self.push(Tok::Template(parts), start);
        }
    }
}

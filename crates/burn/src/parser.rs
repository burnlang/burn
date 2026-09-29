use crate::ast::*;
use crate::diag::Diagnostic;
use crate::lexer::{describe, Tok, Token};
use crate::source::{FileId, Span};

type PResult<T> = Result<T, ()>;

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
    file: FileId,
    pub diags: Vec<Diagnostic>,
    speculative: usize,
    no_struct: bool,
}

pub fn parse_module(toks: Vec<Token>, file: FileId) -> (Module, Vec<Diagnostic>) {
    let mut p = Parser::new(toks, file);
    let items = p.items();
    (Module { items }, p.diags)
}

pub fn parse_expr_tokens(toks: Vec<Token>, file: FileId) -> (Option<Expr>, Vec<Diagnostic>) {
    let mut p = Parser::new(toks, file);
    let e = p.expr().ok();
    if e.is_some() && !p.at(&Tok::Eof) {
        let t = p.peek().clone();
        p.err(t.span, format!("unexpected {} in string template", describe(&t.kind)));
    }
    (e, p.diags)
}

fn is_old_def_word(s: &str) -> bool {
    matches!(s, "type" | "class" | "interface" | "enum")
}

impl Parser {
    pub fn new(toks: Vec<Token>, file: FileId) -> Parser {
        let toks = if toks.is_empty() {
            vec![Token {
                kind: Tok::Eof,
                span: Span::new(file, 0, 0),
                nl_before: true,
            }]
        } else {
            toks
        };
        Parser {
            toks,
            pos: 0,
            file,
            diags: Vec::new(),
            speculative: 0,
            no_struct: false,
        }
    }

    fn peek(&self) -> &Token {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }

    fn peek_at(&self, n: usize) -> &Token {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)]
    }

    fn at(&self, t: &Tok) -> bool {
        &self.peek().kind == t
    }

    fn at_ident(&self, s: &str) -> bool {
        matches!(&self.peek().kind, Tok::Ident(x) if x == s)
    }

    fn advance(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.at(t) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn prev_span(&self) -> Span {
        if self.pos == 0 {
            self.peek().span
        } else {
            self.toks[self.pos - 1].span
        }
    }

    fn err(&mut self, span: Span, msg: impl Into<String>) {
        if self.speculative == 0 {
            self.diags.push(Diagnostic::error(span, msg));
        }
    }

    fn expect(&mut self, t: Tok, what: &str) -> PResult<Span> {
        if self.at(&t) {
            Ok(self.advance().span)
        } else {
            let tok = self.peek().clone();
            let span = if tok.nl_before && self.pos > 0 { self.prev_span() } else { tok.span };
            self.err(span, format!("expected {} but found {}", what, describe(&tok.kind)));
            Err(())
        }
    }

    fn ident(&mut self, what: &str) -> PResult<Ident> {
        match &self.peek().kind {
            Tok::Ident(s) => {
                let s = s.clone();
                let span = self.advance().span;
                Ok(Ident { name: s, span })
            }
            other => {
                let msg = format!("expected {} but found {}", what, describe(other));
                let span = self.peek().span;
                self.err(span, msg);
                Err(())
            }
        }
    }

    fn sync(&mut self) {
        let start = self.pos;
        let mut depth = 0i32;
        loop {
            let t = self.peek();
            if t.kind == Tok::Eof {
                return;
            }
            if self.pos > start && depth == 0 && t.nl_before {
                return;
            }
            match t.kind {
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => {
                    if depth == 0 {
                        if self.pos == start {
                            self.advance();
                        }
                        return;
                    }
                    depth -= 1;
                }
                Tok::Semi if depth == 0 => {
                    self.advance();
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    fn end_stmt(&mut self) {
        if self.eat(&Tok::Semi) {
            return;
        }
        let t = self.peek();
        if t.nl_before || matches!(t.kind, Tok::RBrace | Tok::Eof) {
            return;
        }
        let t = t.clone();
        self.err(t.span, format!("expected end of statement but found {}", describe(&t.kind)));
        self.sync();
    }

    pub fn items(&mut self) -> Vec<Item> {
        let mut items = Vec::new();
        while !self.at(&Tok::Eof) {
            if self.eat(&Tok::Semi) {
                continue;
            }
            let before = self.pos;
            match self.item() {
                Ok(it) => items.push(it),
                Err(()) => {
                    if self.pos == before {
                        self.advance();
                    }
                    self.sync();
                }
            }
        }
        items
    }

    fn item(&mut self) -> PResult<Item> {
        let start = self.peek().span;
        let vis = if self.eat(&Tok::Pub) {
            Vis::Pub
        } else if self.eat(&Tok::Priv) {
            Vis::Priv
        } else {
            Vis::Default
        };
        let kind = match &self.peek().kind {
            Tok::Import => {
                self.advance();
                ItemKind::Import(self.import_list()?)
            }
            Tok::Fun | Tok::Async if !self.is_lambda_start() => ItemKind::Fun(self.fun_decl(false)?),
            Tok::Def => {
                self.advance();
                if self.at(&Tok::Fun) || self.at(&Tok::Async) {
                    ItemKind::Fun(self.fun_decl(false)?)
                } else {
                    ItemKind::Def(self.def()?)
                }
            }
            Tok::Ident(w) if is_old_def_word(w) && matches!(self.peek_at(1).kind, Tok::Ident(_)) && !self.peek_at(1).nl_before => {
                let w = w.clone();
                let span = self.peek().span;
                if self.speculative == 0 {
                    self.diags
                        .push(Diagnostic::error(span, "definitions use the `def` keyword".to_string()).note(format!(
                            "write `def {} {}` instead",
                            w,
                            self.peek_at(1).kind.ident_name()
                        )));
                }
                ItemKind::Def(self.def()?)
            }
            _ => {
                if vis != Vis::Default && !matches!(self.peek().kind, Tok::Var | Tok::Const) && !self.looks_like_typed_decl() {
                    let span = self.peek().span;
                    self.err(span, "`pub` and `priv` can only be used on declarations");
                }
                ItemKind::Stmt(self.stmt()?)
            }
        };
        let span = start.to(self.prev_span());
        Ok(Item { kind, vis, span })
    }

    fn is_lambda_start(&self) -> bool {
        let mut i = 0;
        if self.peek_at(0).kind == Tok::Async {
            i = 1;
        }
        self.peek_at(i).kind == Tok::Fun && self.peek_at(i + 1).kind == Tok::LParen
    }

    fn import_list(&mut self) -> PResult<Vec<(String, Span)>> {
        let mut out = Vec::new();
        if self.eat(&Tok::LParen) {
            while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
                let t = self.advance();
                match t.kind {
                    Tok::Str(s) => out.push((s, t.span)),
                    Tok::Comma | Tok::Semi => {}
                    other => {
                        self.err(t.span, format!("expected import path string but found {}", describe(&other)));
                        return Err(());
                    }
                }
            }
            self.expect(Tok::RParen, "`)` to close import list")?;
        } else {
            let t = self.advance();
            match t.kind {
                Tok::Str(s) => out.push((s, t.span)),
                other => {
                    self.err(t.span, format!("expected import path string but found {}", describe(&other)));
                    return Err(());
                }
            }
        }
        self.end_stmt();
        Ok(out)
    }

    fn fun_decl(&mut self, in_class: bool) -> PResult<FunDecl> {
        let start = self.peek().span;
        let mut is_static = false;
        if in_class && self.at_ident("static") {
            self.advance();
            is_static = true;
        }
        let is_async = self.eat(&Tok::Async);
        self.expect(Tok::Fun, "`fun`")?;
        let name = self.ident("function name")?;
        let (params, ret) = self.signature()?;
        let body = self.block()?;
        Ok(FunDecl {
            name,
            params,
            ret,
            body,
            is_async,
            is_static,
            span: start.to(self.prev_span()),
        })
    }

    fn signature(&mut self) -> PResult<(Vec<Param>, Option<TypeExpr>)> {
        self.expect(Tok::LParen, "`(` to start parameters")?;
        let mut params = Vec::new();
        while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
            params.push(self.param()?);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(Tok::RParen, "`)` to close parameters")?;
        let ret = if self.eat(&Tok::Colon) || self.eat(&Tok::Arrow) {
            Some(self.ty()?)
        } else {
            None
        };
        Ok((params, ret))
    }

    fn param(&mut self) -> PResult<Param> {
        if self.at(&Tok::LBrace) && (self.peek_at(1).nl_before || self.peek_at(1).kind == Tok::RBrace) {
            let span = self.peek().span;
            self.err(span, "expected parameter name or `)` to close the parameter list");
            return Err(());
        }
        if let Tok::Ident(_) = self.peek().kind {
            if self.peek_at(1).kind == Tok::Colon {
                let name = self.ident("parameter name")?;
                self.advance();
                let ty = self.ty()?;
                return Ok(Param { name, ty });
            }
        }
        let ty = self.ty()?;
        let name = self.ident("parameter name")?;
        Ok(Param { name, ty })
    }

    fn def(&mut self) -> PResult<Def> {
        let kw = match &self.peek().kind {
            Tok::Ident(s) => s.clone(),
            other => {
                let msg = format!(
                    "expected `type`, `interface`, `class`, `enum` or `fun` after `def` but found {}",
                    describe(other)
                );
                let span = self.peek().span;
                self.err(span, msg);
                return Err(());
            }
        };
        let kw_span = self.advance().span;
        let name = self.ident("a name")?;
        match kw.as_str() {
            "type" | "struct" | "record" => {
                if self.eat(&Tok::Assign) {
                    let ty = self.ty()?;
                    self.end_stmt();
                    return Ok(Def::Alias { name, ty });
                }
                self.expect(Tok::LBrace, "`{`")?;
                let mut fields = Vec::new();
                while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
                    if self.eat(&Tok::Comma) || self.eat(&Tok::Semi) {
                        continue;
                    }
                    match self.field(Vis::Default) {
                        Ok(f) => fields.push(f),
                        Err(()) => self.sync_member(),
                    }
                }
                self.expect(Tok::RBrace, "`}`")?;
                Ok(Def::Type { name, fields })
            }
            "interface" | "trait" => {
                self.expect(Tok::LBrace, "`{`")?;
                let mut methods = Vec::new();
                while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
                    if self.eat(&Tok::Comma) || self.eat(&Tok::Semi) {
                        continue;
                    }
                    let r: PResult<FunSig> = (|| {
                        let is_async = self.eat(&Tok::Async);
                        self.expect(Tok::Fun, "`fun` in interface body")?;
                        let mname = self.ident("method name")?;
                        let (params, ret) = self.signature()?;
                        if self.at(&Tok::LBrace) {
                            let s = self.peek().span;
                            self.err(s, "interface methods cannot have a body");
                            self.block()?;
                        }
                        Ok(FunSig {
                            name: mname,
                            params,
                            ret,
                            is_async,
                        })
                    })();
                    match r {
                        Ok(m) => methods.push(m),
                        Err(()) => self.sync_member(),
                    }
                }
                self.expect(Tok::RBrace, "`}`")?;
                Ok(Def::Interface { name, methods })
            }
            "class" => {
                let mut implements = Vec::new();
                if self.eat(&Tok::Colon) {
                    loop {
                        implements.push(self.ident("interface name")?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                }
                self.expect(Tok::LBrace, "`{`")?;
                let mut fields = Vec::new();
                let mut methods = Vec::new();
                while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
                    if self.eat(&Tok::Comma) || self.eat(&Tok::Semi) {
                        continue;
                    }
                    let vis = if self.eat(&Tok::Pub) {
                        Vis::Pub
                    } else if self.eat(&Tok::Priv) {
                        Vis::Priv
                    } else {
                        Vis::Default
                    };
                    let is_method =
                        self.at(&Tok::Fun) || self.at(&Tok::Async) || (self.at_ident("static") && matches!(self.peek_at(1).kind, Tok::Fun | Tok::Async));
                    if is_method {
                        match self.fun_decl(true) {
                            Ok(f) => methods.push((vis, f)),
                            Err(()) => self.sync_member(),
                        }
                    } else {
                        if self.at(&Tok::Var) || self.at(&Tok::Const) {
                            self.advance();
                        }
                        match self.field(vis) {
                            Ok(f) => fields.push(f),
                            Err(()) => self.sync_member(),
                        }
                    }
                }
                self.expect(Tok::RBrace, "`}`")?;
                Ok(Def::Class {
                    name,
                    implements,
                    fields,
                    methods,
                })
            }
            "enum" => {
                self.expect(Tok::LBrace, "`{`")?;
                let mut variants = Vec::new();
                while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
                    if self.eat(&Tok::Comma) || self.eat(&Tok::Semi) {
                        continue;
                    }
                    variants.push(self.ident("enum variant")?);
                }
                self.expect(Tok::RBrace, "`}`")?;
                Ok(Def::Enum { name, variants })
            }
            other => {
                self.err(
                    kw_span,
                    format!("unknown definition kind `{}` (expected `type`, `interface`, `class`, `enum` or `fun`)", other),
                );
                Err(())
            }
        }
    }

    fn sync_member(&mut self) {
        let mut depth = 0i32;
        let start = self.pos;
        loop {
            let t = self.peek();
            match t.kind {
                Tok::Eof => return,
                Tok::RBrace if depth == 0 => return,
                _ => {}
            }
            if self.pos > start && depth == 0 && t.nl_before {
                return;
            }
            match t.kind {
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => depth -= 1,
                _ => {}
            }
            self.advance();
        }
    }

    fn field(&mut self, vis: Vis) -> PResult<Field> {
        let (name, ty) = if matches!(self.peek().kind, Tok::Ident(_)) && self.peek_at(1).kind == Tok::Colon {
            let name = self.ident("field name")?;
            self.advance();
            (name, self.ty()?)
        } else {
            let ty = self.ty()?;
            (self.ident("field name")?, ty)
        };
        let default = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
        Ok(Field { name, ty, default, vis })
    }

    pub fn ty(&mut self) -> PResult<TypeExpr> {
        let start = self.peek().span;
        let mut t = match self.peek().kind.clone() {
            Tok::Ident(name) => {
                self.advance();
                let mut args = Vec::new();
                if self.at(&Tok::Lt) && !self.peek().nl_before {
                    self.advance();
                    loop {
                        args.push(self.ty()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::Gt, "`>`")?;
                }
                TypeExpr {
                    kind: TypeExprKind::Named(name, args),
                    span: start.to(self.prev_span()),
                }
            }
            Tok::Null => {
                self.advance();
                TypeExpr {
                    kind: TypeExprKind::Named("null".into(), vec![]),
                    span: start,
                }
            }
            Tok::LBracket => {
                self.advance();
                let inner = self.ty()?;
                self.expect(Tok::RBracket, "`]`")?;
                TypeExpr {
                    kind: TypeExprKind::Array(Box::new(inner)),
                    span: start.to(self.prev_span()),
                }
            }
            Tok::LBrace => {
                self.advance();
                let k = self.ty()?;
                self.expect(Tok::Colon, "`:` in map type")?;
                let v = self.ty()?;
                self.expect(Tok::RBrace, "`}`")?;
                TypeExpr {
                    kind: TypeExprKind::Map(Box::new(k), Box::new(v)),
                    span: start.to(self.prev_span()),
                }
            }
            Tok::Fun => {
                self.advance();
                self.expect(Tok::LParen, "`(`")?;
                let mut ps = Vec::new();
                while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
                    if matches!(self.peek().kind, Tok::Ident(_)) && self.peek_at(1).kind == Tok::Colon {
                        self.advance();
                        self.advance();
                    }
                    ps.push(self.ty()?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RParen, "`)`")?;
                let ret = if self.eat(&Tok::Colon) || self.eat(&Tok::Arrow) {
                    Some(Box::new(self.ty()?))
                } else {
                    None
                };
                TypeExpr {
                    kind: TypeExprKind::Func(ps, ret),
                    span: start.to(self.prev_span()),
                }
            }
            other => {
                let span = self.peek().span;
                self.err(span, format!("expected a type but found {}", describe(&other)));
                return Err(());
            }
        };
        while self.at(&Tok::Question) && !self.peek().nl_before {
            self.advance();
            t = TypeExpr {
                kind: TypeExprKind::Optional(Box::new(t)),
                span: start.to(self.prev_span()),
            };
        }
        Ok(t)
    }

    fn block(&mut self) -> PResult<Block> {
        let start = self.expect(Tok::LBrace, "`{`")?;
        let saved = self.no_struct;
        self.no_struct = false;
        let mut stmts = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
            if self.eat(&Tok::Semi) {
                continue;
            }
            let before = self.pos;
            if matches!(self.peek().kind, Tok::Pub | Tok::Priv | Tok::Import | Tok::Def) || (self.at(&Tok::Fun) && !self.is_lambda_start()) {
                let span = self.peek().span;
                self.err(span, "declarations like this are only allowed at the top level");
                self.sync();
                continue;
            }
            match self.stmt() {
                Ok(s) => stmts.push(s),
                Err(()) => {
                    if self.pos == before {
                        self.advance();
                    }
                    self.sync();
                }
            }
        }
        self.no_struct = saved;
        self.expect(Tok::RBrace, "`}`")?;
        Ok(Block {
            stmts,
            span: start.to(self.prev_span()),
        })
    }

    fn looks_like_typed_decl(&mut self) -> bool {
        match self.peek().kind {
            Tok::Ident(_) | Tok::LBracket | Tok::Fun | Tok::LBrace => {}
            _ => return false,
        }
        if let Tok::Ident(_) = self.peek().kind {
            if let Tok::Ident(_) = self.peek_at(1).kind {
                return !self.peek_at(1).nl_before;
            }
            if !matches!(self.peek_at(1).kind, Tok::Question | Tok::Lt) {
                return false;
            }
        }
        if self.at(&Tok::Fun) && self.peek_at(1).kind != Tok::LParen {
            return false;
        }
        let save = self.pos;
        self.speculative += 1;
        let ok = self.ty().is_ok() && matches!(self.peek().kind, Tok::Ident(_)) && !self.peek().nl_before && {
            let n = self.peek_at(1);
            n.nl_before || matches!(n.kind, Tok::Assign | Tok::Semi | Tok::RBrace | Tok::Eof)
        };
        self.speculative -= 1;
        self.pos = save;
        ok
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        let start = self.peek().span;
        let kind = match self.peek().kind.clone() {
            Tok::Var => {
                self.advance();
                let s = self.var_rest(false)?;
                self.end_stmt();
                s
            }
            Tok::Const => {
                self.advance();
                let s = if self.looks_like_typed_decl() {
                    self.typed_decl(true)?
                } else {
                    self.var_rest(true)?
                };
                self.end_stmt();
                s
            }
            Tok::If => self.if_stmt()?,
            Tok::While => {
                self.advance();
                let cond = self.cond()?;
                let body = self.block()?;
                StmtKind::While { cond, body }
            }
            Tok::For => self.for_stmt()?,
            Tok::Return => {
                self.advance();
                let t = self.peek();
                let value = if t.nl_before || matches!(t.kind, Tok::Semi | Tok::RBrace | Tok::Eof) {
                    None
                } else {
                    Some(self.expr()?)
                };
                self.end_stmt();
                StmtKind::Return(value)
            }
            Tok::Break => {
                self.advance();
                self.end_stmt();
                StmtKind::Break
            }
            Tok::Continue => {
                self.advance();
                self.end_stmt();
                StmtKind::Continue
            }
            Tok::LBrace if !self.looks_like_typed_decl() => StmtKind::Block(self.block()?),
            _ => {
                if self.looks_like_typed_decl() {
                    let s = self.typed_decl(false)?;
                    self.end_stmt();
                    s
                } else {
                    let e = self.expr()?;
                    self.end_stmt();
                    StmtKind::Expr(e)
                }
            }
        };
        Ok(Stmt {
            kind,
            span: start.to(self.prev_span()),
        })
    }

    fn typed_decl(&mut self, is_const: bool) -> PResult<StmtKind> {
        let ty = self.ty()?;
        let name = self.ident("variable name")?;
        let init = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
        if is_const && init.is_none() {
            self.err(name.span, "constants must be initialized");
        }
        Ok(StmtKind::Var {
            name,
            ty: Some(ty),
            init,
            is_const,
        })
    }

    fn var_rest(&mut self, is_const: bool) -> PResult<StmtKind> {
        let name = self.ident("variable name")?;
        let ty = if self.eat(&Tok::Colon) { Some(self.ty()?) } else { None };
        let init = if self.eat(&Tok::Assign) { Some(self.expr()?) } else { None };
        if is_const && init.is_none() {
            self.err(name.span, "constants must be initialized");
        }
        if ty.is_none() && init.is_none() {
            self.err(name.span, format!("cannot infer the type of `{}`: add a type or an initial value", name.name));
        }
        Ok(StmtKind::Var { name, ty, init, is_const })
    }

    fn cond(&mut self) -> PResult<Expr> {
        let saved = self.no_struct;
        self.no_struct = true;
        let r = self.expr();
        self.no_struct = saved;
        r
    }

    fn if_stmt(&mut self) -> PResult<StmtKind> {
        self.advance();
        let cond = self.cond()?;
        let then = self.block()?;
        let els = if self.eat(&Tok::Else) {
            if self.at(&Tok::If) {
                let start = self.peek().span;
                let k = self.if_stmt()?;
                let span = start.to(self.prev_span());
                Some(Block {
                    stmts: vec![Stmt { kind: k, span }],
                    span,
                })
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };
        Ok(StmtKind::If { cond, then, els })
    }

    fn for_stmt(&mut self) -> PResult<StmtKind> {
        self.advance();
        let paren = self.at(&Tok::LParen) && {
            let a = &self.peek_at(1).kind;
            let b = &self.peek_at(2).kind;
            let c = &self.peek_at(3).kind;
            let d = &self.peek_at(4).kind;
            let is_in = matches!(a, Tok::Ident(_)) && (*b == Tok::In || (*b == Tok::Comma && matches!(c, Tok::Ident(_)) && *d == Tok::In));
            is_in || self.is_c_for_paren()
        };
        if paren {
            self.advance();
        }
        let a = self.peek().kind.clone();
        let is_in = matches!(a, Tok::Ident(_))
            && (self.peek_at(1).kind == Tok::In
                || (self.peek_at(1).kind == Tok::Comma && matches!(self.peek_at(2).kind, Tok::Ident(_)) && self.peek_at(3).kind == Tok::In));
        if is_in {
            let first = self.ident("loop variable")?;
            let (index, var) = if self.eat(&Tok::Comma) {
                let second = self.ident("loop variable")?;
                (Some(first), second)
            } else {
                (None, first)
            };
            self.expect(Tok::In, "`in`")?;
            let saved = self.no_struct;
            self.no_struct = !paren;
            let e = self.expr();
            let iter = match e {
                Ok(e) => {
                    if self.at(&Tok::DotDot) || self.at(&Tok::DotDotEq) {
                        let inclusive = self.advance().kind == Tok::DotDotEq;
                        let end = self.expr();
                        self.no_struct = saved;
                        ForIter::Range(e, end?, inclusive)
                    } else {
                        self.no_struct = saved;
                        ForIter::Expr(e)
                    }
                }
                Err(()) => {
                    self.no_struct = saved;
                    return Err(());
                }
            };
            if paren {
                self.expect(Tok::RParen, "`)`")?;
            }
            let body = self.block()?;
            return Ok(StmtKind::ForIn { var, index, iter, body });
        }
        let saved = self.no_struct;
        self.no_struct = !paren;
        let init = if self.at(&Tok::Semi) {
            None
        } else {
            let start = self.peek().span;
            let k = if self.eat(&Tok::Var) {
                self.var_rest(false)?
            } else if self.looks_like_typed_decl() {
                self.typed_decl(false)?
            } else {
                StmtKind::Expr(self.expr()?)
            };
            Some(Box::new(Stmt {
                kind: k,
                span: start.to(self.prev_span()),
            }))
        };
        self.expect(Tok::Semi, "`;` after for-loop initializer")?;
        let cond = if self.at(&Tok::Semi) { None } else { Some(self.expr()?) };
        self.expect(Tok::Semi, "`;` after for-loop condition")?;
        let step = if self.at(&Tok::RParen) || self.at(&Tok::LBrace) {
            None
        } else {
            Some(self.expr()?)
        };
        self.no_struct = saved;
        if paren {
            self.expect(Tok::RParen, "`)`")?;
        }
        let body = self.block()?;
        Ok(StmtKind::For { init, cond, step, body })
    }

    fn is_c_for_paren(&self) -> bool {
        let mut depth = 0;
        let mut i = self.pos;
        while i < self.toks.len() {
            match self.toks[i].kind {
                Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBracket | Tok::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return false;
                    }
                }
                Tok::Semi if depth == 1 => return true,
                Tok::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    pub fn expr(&mut self) -> PResult<Expr> {
        self.assignment()
    }

    fn assignment(&mut self) -> PResult<Expr> {
        let lhs = self.or()?;
        let op = match self.peek().kind {
            Tok::Assign => None,
            Tok::PlusEq => Some(BinOp::Add),
            Tok::MinusEq => Some(BinOp::Sub),
            Tok::StarEq => Some(BinOp::Mul),
            Tok::SlashEq => Some(BinOp::Div),
            Tok::PercentEq => Some(BinOp::Mod),
            _ => return Ok(lhs),
        };
        let op_span = self.advance().span;
        if !matches!(lhs.kind, ExprKind::Ident(_) | ExprKind::Field { .. } | ExprKind::Index { .. }) {
            self.err(lhs.span.to(op_span), "invalid assignment target");
        }
        let value = self.assignment()?;
        let span = lhs.span.to(value.span);
        Ok(Expr {
            kind: ExprKind::Assign {
                target: Box::new(lhs),
                op,
                value: Box::new(value),
            },
            span,
        })
    }

    fn bin(&mut self, lhs: Expr, op: BinOp, rhs: Expr) -> Expr {
        let span = lhs.span.to(rhs.span);
        Expr {
            kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
            span,
        }
    }

    fn or(&mut self) -> PResult<Expr> {
        let mut e = self.and()?;
        while self.at(&Tok::OrOr) {
            self.advance();
            let r = self.and()?;
            e = self.bin(e, BinOp::Or, r);
        }
        Ok(e)
    }

    fn and(&mut self) -> PResult<Expr> {
        let mut e = self.equality()?;
        while self.at(&Tok::AndAnd) {
            self.advance();
            let r = self.equality()?;
            e = self.bin(e, BinOp::And, r);
        }
        Ok(e)
    }

    fn equality(&mut self) -> PResult<Expr> {
        let mut e = self.comparison()?;
        loop {
            let op = match self.peek().kind {
                Tok::EqEq => BinOp::Eq,
                Tok::NotEq => BinOp::Ne,
                _ => return Ok(e),
            };
            self.advance();
            let r = self.comparison()?;
            e = self.bin(e, op, r);
        }
    }

    fn comparison(&mut self) -> PResult<Expr> {
        let mut e = self.term()?;
        loop {
            let op = match self.peek().kind {
                Tok::Lt => BinOp::Lt,
                Tok::Gt => BinOp::Gt,
                Tok::Le => BinOp::Le,
                Tok::Ge => BinOp::Ge,
                Tok::Is => {
                    self.advance();
                    let t = self.ty()?;
                    let span = e.span.to(t.span);
                    e = Expr {
                        kind: ExprKind::Is(Box::new(e), t),
                        span,
                    };
                    continue;
                }
                Tok::Bang if self.peek_at(1).kind == Tok::Is && !self.peek().nl_before => {
                    let bang = self.advance().span;
                    self.advance();
                    let t = self.ty()?;
                    let span = e.span.to(t.span);
                    let is = Expr {
                        kind: ExprKind::Is(Box::new(e), t),
                        span,
                    };
                    e = Expr {
                        kind: ExprKind::Unary(UnOp::Not, Box::new(is)),
                        span: span.to(bang),
                    };
                    continue;
                }
                _ => return Ok(e),
            };
            self.advance();
            let r = self.term()?;
            e = self.bin(e, op, r);
        }
    }

    fn term(&mut self) -> PResult<Expr> {
        let mut e = self.factor()?;
        loop {
            let op = match self.peek().kind {
                Tok::Plus => BinOp::Add,
                Tok::Minus if !self.peek().nl_before => BinOp::Sub,
                _ => return Ok(e),
            };
            self.advance();
            let r = self.factor()?;
            e = self.bin(e, op, r);
        }
    }

    fn factor(&mut self) -> PResult<Expr> {
        let mut e = self.cast()?;
        loop {
            let op = match self.peek().kind {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                _ => return Ok(e),
            };
            self.advance();
            let r = self.cast()?;
            e = self.bin(e, op, r);
        }
    }

    fn cast(&mut self) -> PResult<Expr> {
        let mut e = self.unary()?;
        while self.at(&Tok::As) {
            self.advance();
            let t = self.ty()?;
            let span = e.span.to(t.span);
            e = Expr {
                kind: ExprKind::As(Box::new(e), t),
                span,
            };
        }
        Ok(e)
    }

    fn unary(&mut self) -> PResult<Expr> {
        let start = self.peek().span;
        match self.peek().kind {
            Tok::Minus => {
                self.advance();
                let e = self.unary()?;
                let span = start.to(e.span);
                if let ExprKind::Int(v) = e.kind {
                    return Ok(Expr {
                        kind: ExprKind::Int(v.wrapping_neg()),
                        span,
                    });
                }
                if let ExprKind::Float(v) = e.kind {
                    return Ok(Expr {
                        kind: ExprKind::Float(-v),
                        span,
                    });
                }
                Ok(Expr {
                    kind: ExprKind::Unary(UnOp::Neg, Box::new(e)),
                    span,
                })
            }
            Tok::Bang => {
                self.advance();
                let e = self.unary()?;
                let span = start.to(e.span);
                Ok(Expr {
                    kind: ExprKind::Unary(UnOp::Not, Box::new(e)),
                    span,
                })
            }
            Tok::Await => {
                self.advance();
                let e = self.unary()?;
                let span = start.to(e.span);
                Ok(Expr {
                    kind: ExprKind::Await(Box::new(e)),
                    span,
                })
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            let t = self.peek().clone();
            match t.kind {
                Tok::LParen if !t.nl_before => {
                    self.advance();
                    let saved = self.no_struct;
                    self.no_struct = false;
                    let mut args = Vec::new();
                    while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
                        args.push(self.expr()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.no_struct = saved;
                    self.expect(Tok::RParen, "`)` to close arguments")?;
                    let span = e.span.to(self.prev_span());
                    e = Expr {
                        kind: ExprKind::Call { callee: Box::new(e), args },
                        span,
                    };
                }
                Tok::LBracket if !t.nl_before => {
                    self.advance();
                    let saved = self.no_struct;
                    self.no_struct = false;
                    let index = self.expr();
                    self.no_struct = saved;
                    let index = index?;
                    self.expect(Tok::RBracket, "`]`")?;
                    let span = e.span.to(self.prev_span());
                    e = Expr {
                        kind: ExprKind::Index {
                            obj: Box::new(e),
                            index: Box::new(index),
                        },
                        span,
                    };
                }
                Tok::Dot => {
                    self.advance();
                    let name = match self.peek().kind.clone() {
                        Tok::Int(i) => {
                            let span = self.advance().span;
                            Ident { name: i.to_string(), span }
                        }
                        _ => self.ident("field or method name")?,
                    };
                    let span = e.span.to(name.span);
                    e = Expr {
                        kind: ExprKind::Field { obj: Box::new(e), name },
                        span,
                    };
                }
                Tok::Bang if !t.nl_before && self.peek_at(1).kind == Tok::Bang => {
                    self.advance();
                    self.advance();
                    let span = e.span.to(self.prev_span());
                    e = Expr {
                        kind: ExprKind::NotNull(Box::new(e)),
                        span,
                    };
                }
                _ => return Ok(e),
            }
        }
    }

    fn primary(&mut self) -> PResult<Expr> {
        let t = self.peek().clone();
        let span = t.span;
        let kind = match t.kind {
            Tok::Int(v) => {
                self.advance();
                ExprKind::Int(v)
            }
            Tok::Float(v) => {
                self.advance();
                ExprKind::Float(v)
            }
            Tok::Str(s) => {
                self.advance();
                ExprKind::Str(s)
            }
            Tok::Template(parts) => {
                self.advance();
                let file = self.file;
                let mut diags = Vec::new();
                let tpl = template_from(parts, &mut |toks, sp| {
                    let (e, d) = parse_expr_tokens(toks, file);
                    diags.extend(d);
                    e.unwrap_or(Expr {
                        kind: ExprKind::Str(String::new()),
                        span: sp,
                    })
                });
                if self.speculative == 0 {
                    self.diags.extend(diags);
                }
                ExprKind::Template(tpl)
            }
            Tok::True => {
                self.advance();
                ExprKind::Bool(true)
            }
            Tok::False => {
                self.advance();
                ExprKind::Bool(false)
            }
            Tok::Null => {
                self.advance();
                ExprKind::Null
            }
            Tok::Ident(name) => {
                self.advance();
                if self.at(&Tok::LBrace) && !self.no_struct && !self.peek().nl_before && self.brace_is_struct_lit() {
                    let ty = Ident { name, span };
                    let fields = self.struct_fields()?;
                    return Ok(Expr {
                        kind: ExprKind::StructLit { ty: Some(ty), fields },
                        span: span.to(self.prev_span()),
                    });
                }
                ExprKind::Ident(name)
            }
            Tok::LParen => {
                self.advance();
                let saved = self.no_struct;
                self.no_struct = false;
                let e = self.expr();
                self.no_struct = saved;
                let mut e = e?;
                self.expect(Tok::RParen, "`)`")?;
                e.span = span.to(self.prev_span());
                return Ok(e);
            }
            Tok::LBracket => {
                self.advance();
                let saved = self.no_struct;
                self.no_struct = false;
                let mut items = Vec::new();
                while !self.at(&Tok::RBracket) && !self.at(&Tok::Eof) {
                    match self.expr() {
                        Ok(e) => items.push(e),
                        Err(()) => {
                            self.no_struct = saved;
                            return Err(());
                        }
                    }
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.no_struct = saved;
                self.expect(Tok::RBracket, "`]` to close array")?;
                ExprKind::Array(items)
            }
            Tok::LBrace if !self.no_struct => {
                if self.brace_is_struct_lit() {
                    let fields = self.struct_fields()?;
                    ExprKind::StructLit { ty: None, fields }
                } else {
                    self.advance();
                    let mut pairs = Vec::new();
                    while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
                        let k = self.expr()?;
                        self.expect(Tok::Colon, "`:` in map literal")?;
                        let v = self.expr()?;
                        pairs.push((k, v));
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect(Tok::RBrace, "`}` to close map literal")?;
                    ExprKind::MapLit(pairs)
                }
            }
            Tok::Fun | Tok::Async => {
                let start = self.peek().span;
                let is_async = self.eat(&Tok::Async);
                self.expect(Tok::Fun, "`fun`")?;
                let (params, ret) = self.signature()?;
                let saved = self.no_struct;
                self.no_struct = false;
                let body = self.block();
                self.no_struct = saved;
                let body = body?;
                let span = start.to(self.prev_span());
                let f = FunDecl {
                    name: Ident {
                        name: "<lambda>".into(),
                        span: start,
                    },
                    params,
                    ret,
                    body,
                    is_async,
                    is_static: false,
                    span,
                };
                ExprKind::Lambda(Box::new(f))
            }
            other => {
                let s = if t.nl_before && self.pos > 0 { self.prev_span() } else { span };
                self.err(s, format!("expected an expression but found {}", describe(&other)));
                return Err(());
            }
        };
        Ok(Expr {
            kind,
            span: span.to(self.prev_span()),
        })
    }

    fn brace_is_struct_lit(&self) -> bool {
        let a = &self.peek_at(1).kind;
        let b = &self.peek_at(2).kind;
        match a {
            Tok::RBrace => true,
            Tok::Ident(_) => matches!(b, Tok::Colon | Tok::Comma | Tok::RBrace),
            _ => false,
        }
    }

    fn struct_fields(&mut self) -> PResult<Vec<(Ident, Expr)>> {
        self.expect(Tok::LBrace, "`{`")?;
        let saved = self.no_struct;
        self.no_struct = false;
        let mut fields = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
            let name = match self.ident("field name") {
                Ok(n) => n,
                Err(()) => {
                    self.no_struct = saved;
                    return Err(());
                }
            };
            let value = if self.eat(&Tok::Colon) {
                match self.expr() {
                    Ok(e) => e,
                    Err(()) => {
                        self.no_struct = saved;
                        return Err(());
                    }
                }
            } else {
                Expr {
                    kind: ExprKind::Ident(name.name.clone()),
                    span: name.span,
                }
            };
            fields.push((name, value));
            if !self.eat(&Tok::Comma) && !self.eat(&Tok::Semi) && !self.peek().nl_before {
                break;
            }
        }
        self.no_struct = saved;
        self.expect(Tok::RBrace, "`}` to close struct literal")?;
        Ok(fields)
    }
}

trait IdentName {
    fn ident_name(&self) -> String;
}

impl IdentName for Tok {
    fn ident_name(&self) -> String {
        match self {
            Tok::Ident(s) => s.clone(),
            _ => "Name".into(),
        }
    }
}

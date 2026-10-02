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
    has_destroy: bool,
    splits: Vec<usize>,
}

pub fn parse_module(toks: Vec<Token>, file: FileId) -> (Module, Vec<Diagnostic>) {
    let mut p = Parser::new(toks, file);
    let items = p.items();
    let has_destroy = p.has_destroy;
    (Module { items, has_destroy }, p.diags)
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
            has_destroy: false,
            splits: Vec::new(),
        }
    }

    fn expect_gt(&mut self) -> PResult<Span> {
        let rest = match self.peek().kind {
            Tok::Shr => Tok::Gt,
            Tok::UShr => Tok::Shr,
            Tok::Ge => Tok::Assign,
            Tok::ShrEq => Tok::Ge,
            Tok::UShrEq => Tok::ShrEq,
            _ => return self.expect(Tok::Gt, "`>`"),
        };
        let t = self.peek().clone();
        let pos = self.pos.min(self.toks.len() - 1);
        let first = Span::new(t.span.file, t.span.start as usize, t.span.start as usize + 1);
        let second = Span::new(t.span.file, t.span.start as usize + 1, t.span.end as usize);
        self.toks[pos] = Token {
            kind: Tok::Gt,
            span: first,
            nl_before: t.nl_before,
        };
        self.toks.insert(
            pos + 1,
            Token {
                kind: rest,
                span: second,
                nl_before: false,
            },
        );
        self.splits.push(pos);
        Ok(self.advance().span)
    }

    fn unsplit(&mut self, mark: usize) {
        while self.splits.len() > mark {
            let pos = self.splits.pop().unwrap();
            let rest = self.toks.remove(pos + 1);
            let kind = match rest.kind {
                Tok::Gt => Tok::Shr,
                Tok::Shr => Tok::UShr,
                Tok::Assign => Tok::Ge,
                Tok::Ge => Tok::ShrEq,
                _ => Tok::UShrEq,
            };
            let t = &mut self.toks[pos];
            t.kind = kind;
            t.span = Span::new(t.span.file, t.span.start as usize, rest.span.end as usize);
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
            let mut d = Diagnostic::error(span, format!("expected {} but found {}", what, describe(&tok.kind)));
            let closer = match t {
                Tok::RParen => Some(")"),
                Tok::RBracket => Some("]"),
                _ => None,
            };
            if let (Some(c), true) = (closer, tok.nl_before && self.pos > 0) {
                let end = self.prev_span().end as usize;
                d = d.maybe_fix(format!("add the missing `{}`", c), Span::new(span.file, end, end), c);
            }
            if self.speculative == 0 {
                self.diags.push(d);
            }
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

    fn annotations(&mut self) -> PResult<Vec<Annotation>> {
        let mut out = Vec::new();
        while self.at(&Tok::At) {
            let start = self.advance().span;
            let name = self.ident("annotation name")?;
            let mut args = Vec::new();
            if self.at(&Tok::LParen) && !self.peek().nl_before {
                self.advance();
                while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
                    let key = if matches!(self.peek().kind, Tok::Ident(_)) && self.peek_at(1).kind == Tok::Colon {
                        let k = self.ident("argument name")?;
                        self.advance();
                        Some(k)
                    } else {
                        None
                    };
                    args.push((key, self.expr()?));
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RParen, "`)` to close the annotation")?;
            }
            out.push(Annotation {
                name,
                args,
                span: start.to(self.prev_span()),
            });
        }
        Ok(out)
    }

    fn item(&mut self) -> PResult<Item> {
        let start = self.peek().span;
        let annotations = self.annotations()?;
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
                    let mut d = self.def()?;
                    self.expand_accessors(&mut d, &annotations);
                    ItemKind::Def(d)
                }
            }
            Tok::Ident(w) if is_old_def_word(w) && matches!(self.peek_at(1).kind, Tok::Ident(_)) && !self.peek_at(1).nl_before => {
                let w = w.clone();
                let span = self.peek().span;
                if self.speculative == 0 {
                    let at = Span::new(span.file, span.start as usize, span.start as usize);
                    self.diags.push(Diagnostic::error(span, "definitions use the `def` keyword").fix(
                        format!("write `def {} {}`", w, self.peek_at(1).kind.ident_name()),
                        at,
                        "def ",
                    ));
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
        let mut kind = kind;
        let mut item_annotations = annotations;
        match &mut kind {
            ItemKind::Fun(f) => f.annotations = std::mem::take(&mut item_annotations),
            ItemKind::Def(_) => {}
            _ => {
                if let Some(a) = item_annotations.first() {
                    let s = a.span;
                    self.err(s, "annotations can only be placed on definitions, functions, fields and methods");
                }
                item_annotations.clear();
            }
        }
        Ok(Item {
            kind,
            vis,
            span,
            annotations: item_annotations,
        })
    }

    fn expand_accessors(&mut self, d: &mut Def, annotations: &[Annotation]) {
        let on = |anns: &[Annotation], n: &str| anns.iter().find(|a| a.name.name == n).map(|a| a.span);
        let class_get = on(annotations, "Getter");
        let class_set = on(annotations, "Setter");
        match d {
            Def::Struct {
                fields,
                methods,
                params,
                param_anns,
                ..
            } => {
                let mut extra = Vec::new();
                let param_fields: Vec<Field> = params
                    .iter()
                    .zip(param_anns.iter())
                    .map(|(p, a)| Field {
                        name: p.name.clone(),
                        ty: p.ty.clone(),
                        default: None,
                        vis: Vis::Default,
                        annotations: a.clone(),
                    })
                    .collect();
                for f in param_fields.iter().chain(fields.iter()) {
                    let get = on(&f.annotations, "Getter").or(class_get);
                    let set = on(&f.annotations, "Setter").or(class_set);
                    let cap = {
                        let mut c = f.name.name.chars();
                        match c.next() {
                            Some(h) => h.to_uppercase().collect::<String>() + c.as_str(),
                            None => String::new(),
                        }
                    };
                    let field_ref = |span: Span| Expr {
                        kind: ExprKind::Field {
                            obj: Box::new(Expr {
                                kind: ExprKind::Ident("self".into()),
                                span,
                            }),
                            name: Ident {
                                name: f.name.name.clone(),
                                span,
                            },
                        },
                        span,
                    };
                    if let Some(span) = get {
                        let is_bool = matches!(&f.ty.kind, TypeExprKind::Named(n, a) if n == "bool" && a.is_empty());
                        let name = format!("{}{}", if is_bool { "is" } else { "get" }, cap);
                        if !methods.iter().any(|(_, m)| m.name.name == name) {
                            extra.push((
                                f.vis,
                                FunDecl {
                                    name: Ident { name, span },
                                    params: Vec::new(),
                                    ret: Some(f.ty.clone()),
                                    body: Block {
                                        stmts: vec![Stmt {
                                            kind: StmtKind::Return(Some(field_ref(span))),
                                            span,
                                        }],
                                        span,
                                    },
                                    is_async: false,
                                    is_static: false,
                                    span,
                                    annotations: Vec::new(),
                                    bodyless: false,
                                    is_abstract: false,
                                    tparams: Vec::new(),
                                },
                            ));
                        }
                    }
                    if let Some(span) = set {
                        let name = format!("set{}", cap);
                        if !methods.iter().any(|(_, m)| m.name.name == name) {
                            let value = Expr {
                                kind: ExprKind::Ident(f.name.name.clone()),
                                span,
                            };
                            extra.push((
                                f.vis,
                                FunDecl {
                                    name: Ident { name, span },
                                    params: vec![Param {
                                        name: Ident {
                                            name: f.name.name.clone(),
                                            span,
                                        },
                                        ty: f.ty.clone(),
                                    }],
                                    ret: None,
                                    body: Block {
                                        stmts: vec![Stmt {
                                            kind: StmtKind::Expr(Expr {
                                                kind: ExprKind::Assign {
                                                    target: Box::new(field_ref(span)),
                                                    op: None,
                                                    value: Box::new(value),
                                                },
                                                span,
                                            }),
                                            span,
                                        }],
                                        span,
                                    },
                                    is_async: false,
                                    is_static: false,
                                    span,
                                    annotations: Vec::new(),
                                    bodyless: false,
                                    is_abstract: false,
                                    tparams: Vec::new(),
                                },
                            ));
                        }
                    }
                }
                methods.extend(extra);
            }
            _ => {
                let spans: Vec<Span> = annotations
                    .iter()
                    .filter(|a| a.name.name == "Getter" || a.name.name == "Setter")
                    .map(|a| a.span)
                    .collect();
                for s in spans {
                    self.err(s, "@Getter and @Setter generate methods, so they can only be used on structs and their fields");
                }
                if let Def::Type { fields, .. } | Def::Annotation { fields, .. } = d {
                    let spans: Vec<Span> = fields
                        .iter()
                        .flat_map(|f| f.annotations.iter())
                        .filter(|a| a.name.name == "Getter" || a.name.name == "Setter")
                        .map(|a| a.span)
                        .collect();
                    for s in spans {
                        self.err(s, "@Getter and @Setter generate methods, so they can only be used on structs and their fields");
                    }
                }
            }
        }
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
        let tparams = self.type_params()?;
        let (params, ret) = self.signature()?;
        let bodyless = !self.at(&Tok::LBrace) && (self.peek().nl_before || matches!(self.peek().kind, Tok::Eof | Tok::Semi | Tok::RBrace));
        let body = if bodyless {
            Block {
                stmts: Vec::new(),
                span: self.prev_span(),
            }
        } else {
            self.block()?
        };
        Ok(FunDecl {
            name,
            params,
            ret,
            body,
            is_async,
            is_static,
            span: start.to(self.prev_span()),
            annotations: Vec::new(),
            bodyless,
            is_abstract: false,
            tparams,
        })
    }

    fn type_params(&mut self) -> PResult<Vec<Ident>> {
        let mut out = Vec::new();
        if !self.at(&Tok::Lt) || self.peek().nl_before {
            return Ok(out);
        }
        self.advance();
        loop {
            out.push(self.ident("type parameter name")?);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect_gt()?;
        Ok(out)
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
        let mut kind = StructKind::Normal;
        for (word, k) in [("abstract", StructKind::Abstract), ("static", StructKind::Static)] {
            if self.at_ident(word) && matches!(&self.peek_at(1).kind, Tok::Ident(n) if n == "struct" || n == "class") {
                self.advance();
                kind = k;
            }
        }
        let kw = match &self.peek().kind {
            Tok::Ident(s) => s.clone(),
            other => {
                let msg = format!(
                    "expected `type`, `interface`, `struct`, `enum`, `annotation` or `fun` after `def` but found {}",
                    describe(other)
                );
                let span = self.peek().span;
                self.err(span, msg);
                return Err(());
            }
        };
        let kw_span = self.advance().span;
        let name = self.ident("a name")?;
        let tparams = if matches!(kw.as_str(), "struct" | "class" | "type" | "record") {
            self.type_params()?
        } else {
            Vec::new()
        };
        match kw.as_str() {
            "struct" | "class" => {
                if kw == "class" {
                    let note = format!("write `def struct {}(...)` and create objects with `new {}(...)`", name.name, name.name);
                    if self.speculative == 0 {
                        self.diags.push(Diagnostic::error(kw_span, "classes are now structs").help(note).maybe_fix(
                            "declare it as a struct",
                            kw_span,
                            "struct",
                        ));
                    }
                }
                self.struct_def(name, kind, tparams)
            }
            "type" | "record" | "annotation" => {
                if kw != "annotation" && self.at(&Tok::Assign) {
                    if let Some(t) = tparams.first() {
                        self.err(t.span, "type aliases cannot have type parameters");
                    }
                    self.advance();
                    let ty = self.ty()?;
                    self.end_stmt();
                    return Ok(Def::Alias { name, ty });
                }
                let fields = if kw == "annotation" && !self.at(&Tok::LBrace) {
                    Vec::new()
                } else {
                    self.expect(Tok::LBrace, "`{`")?;
                    let mut fields = Vec::new();
                    while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
                        if self.eat(&Tok::Comma) || self.eat(&Tok::Semi) {
                            continue;
                        }
                        let r = self.annotations().and_then(|anns| {
                            self.field(Vis::Default).map(|mut f| {
                                f.annotations = anns;
                                f
                            })
                        });
                        match r {
                            Ok(f) => fields.push(f),
                            Err(()) => self.sync_member(),
                        }
                    }
                    self.expect(Tok::RBrace, "`}`")?;
                    fields
                };
                if kw == "annotation" {
                    return Ok(Def::Annotation { name, fields });
                }
                Ok(Def::Type { name, fields, tparams })
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
                    format!(
                        "unknown definition kind `{}` (expected `type`, `interface`, `struct`, `enum`, `annotation` or `fun`)",
                        other
                    ),
                );
                Err(())
            }
        }
    }

    fn struct_def(&mut self, name: Ident, kind: StructKind, tparams: Vec<Ident>) -> PResult<Def> {
        let mut params = Vec::new();
        let mut param_anns = Vec::new();
        if self.at(&Tok::LParen) && !self.peek().nl_before {
            self.advance();
            while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
                param_anns.push(self.annotations()?);
                params.push(self.param()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect(Tok::RParen, "`)` to close the constructor parameters")?;
        }
        let mut extends = None;
        let mut supers = Vec::new();
        if self.eat(&Tok::Colon) {
            let s = self.ident("the name of the struct to extend")?;
            let args = if self.at(&Tok::LParen) && !self.peek().nl_before {
                Some(self.call_args()?)
            } else {
                None
            };
            extends = Some((s, args));
            if self.eat(&Tok::Comma) {
                while matches!(self.peek().kind, Tok::Ident(_)) {
                    let s = self.ident("a name")?;
                    let args = if self.at(&Tok::LParen) && !self.peek().nl_before {
                        Some(self.call_args()?)
                    } else {
                        None
                    };
                    supers.push((s, args));
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
            }
        }
        let colon_extra = supers.len();
        if self.eat(&Tok::ColonColon) {
            loop {
                let s = self.ident("an interface or abstract struct name")?;
                let args = if self.at(&Tok::LParen) && !self.peek().nl_before {
                    Some(self.call_args()?)
                } else {
                    None
                };
                supers.push((s, args));
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        self.expect(Tok::LBrace, "`{`")?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut statics = Vec::new();
        while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
            if self.eat(&Tok::Comma) || self.eat(&Tok::Semi) {
                continue;
            }
            let member_anns = match self.annotations() {
                Ok(a) => a,
                Err(()) => {
                    self.sync_member();
                    continue;
                }
            };
            let vis = if self.eat(&Tok::Pub) {
                Vis::Pub
            } else if self.eat(&Tok::Priv) {
                Vis::Priv
            } else {
                Vis::Default
            };
            if self.at_ident("abstract") && matches!(self.peek_at(1).kind, Tok::Fun | Tok::Async) {
                let abs_span = self.advance().span;
                match self.fun_decl(true) {
                    Ok(mut f) => {
                        if !f.bodyless {
                            self.err(f.name.span, format!("abstract method `{}` cannot have a body", f.name.name));
                        }
                        if kind != StructKind::Abstract {
                            self.err(abs_span, "abstract methods can only be declared in an `abstract struct`");
                        }
                        f.bodyless = true;
                        f.is_abstract = true;
                        f.body.stmts.clear();
                        f.annotations = member_anns;
                        methods.push((vis, f));
                    }
                    Err(()) => self.sync_member(),
                }
                continue;
            }
            let is_method = self.at(&Tok::Fun) || self.at(&Tok::Async) || (self.at_ident("static") && matches!(self.peek_at(1).kind, Tok::Fun | Tok::Async));
            if is_method {
                match self.fun_decl(true) {
                    Ok(mut f) => {
                        if kind == StructKind::Static {
                            f.is_static = true;
                        }
                        f.annotations = member_anns;
                        methods.push((vis, f))
                    }
                    Err(()) => self.sync_member(),
                }
                continue;
            }
            let static_val = kind == StructKind::Static || self.at_ident("static");
            if static_val {
                if self.at_ident("static") {
                    self.advance();
                }
                match self.static_val(vis) {
                    Ok(v) => statics.push(v),
                    Err(()) => self.sync_member(),
                }
                continue;
            }
            if self.at(&Tok::Var) || self.at(&Tok::Const) {
                self.advance();
            }
            match self.field(vis) {
                Ok(mut f) => {
                    f.annotations = member_anns;
                    fields.push(f)
                }
                Err(()) => self.sync_member(),
            }
        }
        self.expect(Tok::RBrace, "`}`")?;
        Ok(Def::Struct {
            name,
            kind,
            params,
            param_anns,
            extends,
            supers,
            colon_extra,
            fields,
            methods,
            statics,
            tparams,
        })
    }

    fn static_val(&mut self, vis: Vis) -> PResult<StaticVal> {
        let kw = if self.at(&Tok::Var) || self.at(&Tok::Const) {
            Some(self.advance().kind == Tok::Const)
        } else {
            None
        };
        let (name, ty) = if kw.is_some() && !self.looks_like_typed_decl() {
            let name = self.ident("a name")?;
            let ty = if self.eat(&Tok::Colon) { Some(self.ty()?) } else { None };
            (name, ty)
        } else if matches!(self.peek().kind, Tok::Ident(_)) && self.peek_at(1).kind == Tok::Colon {
            let name = self.ident("a name")?;
            self.advance();
            (name, Some(self.ty()?))
        } else {
            let ty = self.ty()?;
            (self.ident("a name")?, Some(ty))
        };
        if !self.at(&Tok::Assign) {
            let s = name.span;
            self.err(s, format!("static value `{}` needs an initial value", name.name));
            return Err(());
        }
        self.advance();
        let init = self.expr()?;
        Ok(StaticVal {
            name,
            ty,
            init,
            is_const: kw == Some(true),
            vis,
        })
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
        Ok(Field {
            name,
            ty,
            default,
            vis,
            annotations: Vec::new(),
        })
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
                    self.expect_gt()?;
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
        if self.at_ident("match") && !self.peek_at(1).nl_before && self.looks_like_match() {
            return false;
        }
        match self.peek().kind {
            Tok::Ident(_) | Tok::LBracket | Tok::Fun | Tok::LBrace => {}
            _ => return false,
        }
        if let Tok::Ident(first) = &self.peek().kind {
            if let Tok::Ident(_) = self.peek_at(1).kind {
                if first == "new" && self.peek_at(2).kind == Tok::LParen {
                    return false;
                }
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
        let mark = self.splits.len();
        self.speculative += 1;
        let ok = self.ty().is_ok() && matches!(self.peek().kind, Tok::Ident(_)) && !self.peek().nl_before && {
            let n = self.peek_at(1);
            n.nl_before || matches!(n.kind, Tok::Assign | Tok::Semi | Tok::RBrace | Tok::Eof)
        };
        self.speculative -= 1;
        self.pos = save;
        self.unsplit(mark);
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
            Tok::Ident(w)
                if w == "destroy" && matches!(self.peek_at(1).kind, Tok::Ident(_)) && !self.peek_at(1).nl_before && self.peek_at(2).kind == Tok::LParen =>
            {
                self.advance();
                let target = self.ident("object name")?;
                let args = self.call_args()?;
                self.end_stmt();
                self.has_destroy = true;
                StmtKind::Destroy { target, args }
            }
            Tok::Ident(_) if self.peek_at(1).kind == Tok::Dot && self.is_extension() => {
                let target = self.ident("object name")?;
                self.advance();
                let name = self.ident("function name")?;
                let (params, ret) = self.signature()?;
                let body = self.block()?;
                StmtKind::Extend {
                    target,
                    func: Box::new(FunDecl {
                        span: name.span.to(self.prev_span()),
                        name,
                        params,
                        ret,
                        body,
                        is_async: false,
                        is_static: false,
                        annotations: Vec::new(),
                        bodyless: false,
                        is_abstract: false,
                        tparams: Vec::new(),
                    }),
                }
            }
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

    fn is_extension(&mut self) -> bool {
        if !matches!(self.peek_at(2).kind, Tok::Ident(_)) || self.peek_at(3).kind != Tok::LParen || self.peek_at(3).nl_before {
            return false;
        }
        let save = self.pos;
        self.speculative += 1;
        self.advance();
        self.advance();
        self.advance();
        let ok = self.signature().is_ok() && self.at(&Tok::LBrace) && !self.peek().nl_before;
        self.speculative -= 1;
        self.pos = save;
        ok
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
            Tok::AmpEq => Some(BinOp::BitAnd),
            Tok::PipeEq => Some(BinOp::BitOr),
            Tok::CaretEq => Some(BinOp::BitXor),
            Tok::ShlEq => Some(BinOp::Shl),
            Tok::ShrEq => Some(BinOp::Shr),
            Tok::UShrEq => Some(BinOp::UShr),
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
        let mut e = self.coalesce()?;
        loop {
            let op = match self.peek().kind {
                Tok::EqEq => BinOp::Eq,
                Tok::NotEq => BinOp::Ne,
                _ => return Ok(e),
            };
            self.advance();
            let r = self.coalesce()?;
            e = self.bin(e, op, r);
        }
    }

    fn coalesce(&mut self) -> PResult<Expr> {
        let e = self.comparison()?;
        if !self.at(&Tok::QuestionQuestion) {
            return Ok(e);
        }
        self.advance();
        let r = self.coalesce()?;
        let span = e.span.to(r.span);
        Ok(Expr {
            kind: ExprKind::Coalesce(Box::new(e), Box::new(r)),
            span,
        })
    }

    fn comparison(&mut self) -> PResult<Expr> {
        let mut e = self.bit_or()?;
        loop {
            let op = match self.peek().kind {
                Tok::Lt => BinOp::Lt,
                Tok::Gt => BinOp::Gt,
                Tok::Le => BinOp::Le,
                Tok::Ge => BinOp::Ge,
                Tok::Is if !self.peek().nl_before => {
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
            let r = self.bit_or()?;
            e = self.bin(e, op, r);
        }
    }

    fn bit_or(&mut self) -> PResult<Expr> {
        let mut e = self.bit_xor()?;
        while self.at(&Tok::Pipe) {
            self.advance();
            let r = self.bit_xor()?;
            e = self.bin(e, BinOp::BitOr, r);
        }
        Ok(e)
    }

    fn bit_xor(&mut self) -> PResult<Expr> {
        let mut e = self.bit_and()?;
        while self.at(&Tok::Caret) {
            self.advance();
            let r = self.bit_and()?;
            e = self.bin(e, BinOp::BitXor, r);
        }
        Ok(e)
    }

    fn bit_and(&mut self) -> PResult<Expr> {
        let mut e = self.shift()?;
        while self.at(&Tok::Amp) {
            self.advance();
            let r = self.shift()?;
            e = self.bin(e, BinOp::BitAnd, r);
        }
        Ok(e)
    }

    fn shift(&mut self) -> PResult<Expr> {
        let mut e = self.term()?;
        loop {
            let op = match self.peek().kind {
                Tok::Shl => BinOp::Shl,
                Tok::Shr => BinOp::Shr,
                Tok::UShr => BinOp::UShr,
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
        while self.at(&Tok::As) && !self.peek().nl_before {
            self.advance();
            let safe = self.at(&Tok::Question) && !self.peek().nl_before;
            if safe {
                self.advance();
            }
            let t = self.ty()?;
            let span = e.span.to(t.span);
            e = Expr {
                kind: if safe {
                    ExprKind::SafeAs(Box::new(e), t)
                } else {
                    ExprKind::As(Box::new(e), t)
                },
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
            Tok::Tilde => {
                self.advance();
                let e = self.unary()?;
                let span = start.to(e.span);
                Ok(Expr {
                    kind: ExprKind::Unary(UnOp::BitNot, Box::new(e)),
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

    fn call_args(&mut self) -> PResult<Vec<Expr>> {
        self.expect(Tok::LParen, "`(`")?;
        let saved = self.no_struct;
        self.no_struct = false;
        let mut args = Vec::new();
        while !self.at(&Tok::RParen) && !self.at(&Tok::Eof) {
            match self.expr() {
                Ok(a) => args.push(a),
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
        self.expect(Tok::RParen, "`)` to close arguments")?;
        Ok(args)
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            let t = self.peek().clone();
            match t.kind {
                Tok::LParen if !t.nl_before => {
                    let args = self.call_args()?;
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
                Tok::QuestionDot => {
                    self.advance();
                    let name = self.ident("field or method name")?;
                    let args = if self.at(&Tok::LParen) && !self.peek().nl_before {
                        Some(self.call_args()?)
                    } else {
                        None
                    };
                    let span = e.span.to(self.prev_span());
                    e = Expr {
                        kind: ExprKind::SafeGet { obj: Box::new(e), name, args },
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
            Tok::Ident(name) if name == "new" && matches!(self.peek_at(1).kind, Tok::Ident(_)) && !self.peek_at(1).nl_before => {
                self.advance();
                let ty = self.ident("struct name")?;
                let mut targs = Vec::new();
                if self.at(&Tok::Lt) && !self.peek().nl_before {
                    self.advance();
                    loop {
                        targs.push(self.ty()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect_gt()?;
                }
                let args = if self.at(&Tok::LParen) && !self.peek().nl_before {
                    self.call_args()?
                } else {
                    let s = self.prev_span();
                    self.err(s, format!("expected `(` after `new {}`", ty.name));
                    return Err(());
                };
                return Ok(Expr {
                    kind: ExprKind::New { ty, targs, args },
                    span: span.to(self.prev_span()),
                });
            }
            Tok::Ident(name) if name == "match" && !self.peek_at(1).nl_before && self.looks_like_match() => {
                return self.match_expr();
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
                    annotations: Vec::new(),
                    bodyless: false,
                    is_abstract: false,
                    span,
                    tparams: Vec::new(),
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

    fn looks_like_match(&mut self) -> bool {
        if self.peek_at(1).kind == Tok::LBrace {
            return true;
        }
        let save = self.pos;
        let mark = self.splits.len();
        let saved = self.no_struct;
        self.speculative += 1;
        self.advance();
        self.no_struct = true;
        let ok = self.expr().is_ok() && self.at(&Tok::LBrace) && !self.peek().nl_before;
        self.no_struct = saved;
        self.speculative -= 1;
        self.pos = save;
        self.unsplit(mark);
        ok
    }

    fn match_expr(&mut self) -> PResult<Expr> {
        let start = self.advance().span;
        let subject = if self.at(&Tok::LBrace) {
            None
        } else {
            let saved = self.no_struct;
            self.no_struct = true;
            let e = self.expr();
            self.no_struct = saved;
            Some(Box::new(e?))
        };
        self.expect(Tok::LBrace, "`{` to start the match arms")?;
        let saved = self.no_struct;
        self.no_struct = false;
        let arms = self.match_arms(subject.is_some());
        self.no_struct = saved;
        let arms = arms?;
        self.expect(Tok::RBrace, "`}` to close the match")?;
        Ok(Expr {
            kind: ExprKind::Match { subject, arms },
            span: start.to(self.prev_span()),
        })
    }

    fn match_arms(&mut self, has_subject: bool) -> PResult<Vec<MatchArm>> {
        let mut arms = Vec::new();
        loop {
            while self.eat(&Tok::Semi) {}
            if self.at(&Tok::RBrace) || self.at(&Tok::Eof) {
                return Ok(arms);
            }
            let start = self.peek().span;
            let mut patterns = Vec::new();
            if !self.eat(&Tok::Else) {
                loop {
                    patterns.push(self.pattern(has_subject)?);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
            }
            let guard = if self.eat(&Tok::If) { Some(self.expr()?) } else { None };
            self.expect(Tok::FatArrow, "`=>` after the pattern")?;
            let body = if self.at(&Tok::LBrace) {
                ArmBody::Block(self.block()?)
            } else {
                ArmBody::Expr(self.expr()?)
            };
            let span = start.to(self.prev_span());
            arms.push(MatchArm { patterns, guard, body, span });
            if self.eat(&Tok::Comma) || self.at(&Tok::RBrace) || self.at(&Tok::Semi) || self.peek().nl_before {
                continue;
            }
            let t = self.peek().clone();
            self.err(
                t.span,
                format!("expected a new line or `,` after the match arm but found {}", describe(&t.kind)),
            );
            return Err(());
        }
    }

    fn pattern(&mut self, has_subject: bool) -> PResult<Pattern> {
        if !has_subject {
            return Ok(Pattern::Value(self.or()?));
        }
        if self.eat(&Tok::Is) {
            let t = self.ty()?;
            let bind = match self.peek().kind.clone() {
                Tok::Ident(name) if !self.peek().nl_before => {
                    let span = self.advance().span;
                    Some(Ident { name, span })
                }
                _ => None,
            };
            return Ok(Pattern::Is(t, bind));
        }
        let e = self.bit_or()?;
        let inclusive = match self.peek().kind {
            Tok::DotDot => false,
            Tok::DotDotEq => true,
            _ => return Ok(Pattern::Value(e)),
        };
        self.advance();
        let end = self.bit_or()?;
        Ok(Pattern::Range(e, end, inclusive))
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

use super::json::Json;
use super::{path_to_uri, range_json, sources, uri_to_path, Analysis, Server};
use crate::lexer::{self, Tok, Token};
use crate::loader::{Loaded, STDLIB};
use crate::source::{SourceFile, Span};
use crate::types::{Ty, TyId};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub enum Target {
    File(PathBuf),
    Std(&'static str),
    Lib(PathBuf),
}

pub struct Export {
    pub name: String,
    pub kind: i32,
    pub detail: String,
}

pub fn import_links(loaded: &Loaded, root: usize) -> Vec<(Span, Target)> {
    let m = &loaded.modules[root];
    let mut out = Vec::new();
    for (mi, span) in &m.imports {
        let f = loaded.sm.file(loaded.modules[*mi].file);
        let target = match &f.path {
            Some(p) => Some(Target::File(p.clone())),
            None => sources::std_display(&f.name)
                .and_then(|n| STDLIB.iter().find(|s| s.name == n))
                .map(|s| Target::Std(s.name)),
        };
        if let Some(t) = target {
            out.push((*span, t));
        }
    }
    for (li, span) in &m.libs {
        out.push((*span, Target::Lib(loaded.libs[*li].path.clone())));
    }
    out
}

fn pos(file: &SourceFile, offset: usize) -> Json {
    let (l, c) = file.line_utf16_col(offset);
    Json::obj(vec![("line", Json::num(l as f64)), ("character", Json::num(c as f64))])
}

fn range(file: &SourceFile, start: usize, end: usize) -> Json {
    Json::obj(vec![("start", pos(file, start)), ("end", pos(file, end))])
}

fn line_range(start_line: usize, end_line: usize) -> Json {
    let p = |l: usize| Json::obj(vec![("line", Json::num(l as f64)), ("character", Json::num(0))]);
    Json::obj(vec![("start", p(start_line)), ("end", p(end_line))])
}

pub fn exports_of(text: &str) -> Vec<Export> {
    let (tokens, _) = lexer::lex(text, 0);
    let ident = |i: usize| match tokens.get(i).map(|t| &t.kind) {
        Some(Tok::Ident(n)) => Some(n.as_str()),
        _ => None,
    };
    let line_of = |t: &Token| {
        let s = t.span.start as usize;
        let start = text[..s].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let end = text[s..].find('\n').map(|i| s + i).unwrap_or(text.len());
        text[start..end].trim().trim_end_matches('{').trim().to_string()
    };
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut private = false;
    for (i, t) in tokens.iter().enumerate() {
        if t.nl_before {
            private = false;
        }
        match &t.kind {
            Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
            Tok::RBrace | Tok::RParen | Tok::RBracket => depth -= 1,
            Tok::Priv if depth == 0 => private = true,
            Tok::Fun if depth == 0 && !private => {
                if let Some(name) = ident(i + 1) {
                    out.push(Export {
                        name: name.to_string(),
                        kind: 3,
                        detail: line_of(t),
                    });
                }
            }
            Tok::Def if depth == 0 && !private => {
                let mut k = i + 1;
                while matches!(ident(k), Some("abstract" | "static")) {
                    k += 1;
                }
                let kind = match ident(k) {
                    Some("struct") => 22,
                    Some("interface") => 8,
                    Some("enum") => 13,
                    Some("type" | "annotation") => 22,
                    _ => continue,
                };
                if let Some(name) = ident(k + 1) {
                    out.push(Export {
                        name: name.to_string(),
                        kind,
                        detail: line_of(t),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

pub fn std_exports() -> &'static [(&'static str, Vec<Export>)] {
    static ALL: std::sync::OnceLock<Vec<(&'static str, Vec<Export>)>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| STDLIB.iter().map(|s| (s.name, exports_of(s.src))).collect())
}

fn matching_brace(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, t) in tokens.iter().enumerate().skip(open) {
        match t.kind {
            Tok::LBrace => depth += 1,
            Tok::RBrace => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn body_end(tokens: &[Token], name_end: u32) -> Option<usize> {
    let start = tokens.iter().position(|t| t.span.start >= name_end)?;
    let mut parens = 0i32;
    for (i, t) in tokens.iter().enumerate().skip(start) {
        match t.kind {
            Tok::LParen | Tok::LBracket => parens += 1,
            Tok::RParen | Tok::RBracket => parens -= 1,
            Tok::LBrace if parens == 0 => {
                return matching_brace(tokens, i).map(|j| tokens[j].span.end as usize);
            }
            Tok::RBrace | Tok::Eof => return None,
            _ => {}
        }
        if i > start && t.nl_before && parens == 0 && !matches!(t.kind, Tok::LBrace) {
            return None;
        }
    }
    None
}

fn line_start(src: &str, offset: usize) -> usize {
    src[..offset.min(src.len())].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn module_spec(from: &Path, to: &Path) -> String {
    let base = from.parent().unwrap_or(Path::new("."));
    let mut b: Vec<_> = base.components().collect();
    let t: Vec<_> = to.components().collect();
    let mut common = 0;
    while common < b.len() && common < t.len() && b[common] == t[common] {
        common += 1;
    }
    b.truncate(common);
    let mut parts: Vec<String> = Vec::new();
    for _ in common..base.components().count() {
        parts.push("..".into());
    }
    for c in &t[common..] {
        parts.push(c.as_os_str().to_string_lossy().into_owned());
    }
    parts.join("/")
}

impl Analysis {
    pub fn import_at(&self, off: usize) -> Option<&(Span, Target)> {
        self.links
            .iter()
            .find(|(s, _)| s.file == self.root_file && (s.start as usize) <= off && off <= (s.end as usize))
    }

    fn type_decl(&self, mut t: TyId) -> Option<Span> {
        for _ in 0..4 {
            match self.types.get(t) {
                Ty::Optional(inner) | Ty::Array(inner) => t = *inner,
                _ => break,
            }
        }
        self.type_names.iter().find(|n| n.1 == t).map(|n| n.2)
    }

    fn type_at(&self, off: usize) -> Option<TyId> {
        let inside = |s: &Span| s.file == self.root_file && (s.start as usize) <= off && off <= (s.end as usize);
        if let Some(l) = self.index.locals.iter().filter(|l| inside(&l.decl)).min_by_key(|l| l.decl.end - l.decl.start) {
            return Some(l.ty);
        }
        self.index
            .expr_types
            .iter()
            .filter(|(s, _)| inside(s))
            .min_by_key(|(s, _)| s.end - s.start)
            .map(|(_, t)| *t)
    }
}

impl Server {
    fn target_location(&self, a: &Analysis, span: Span, target: &Target) -> Option<(PathBuf, usize)> {
        match target {
            Target::File(p) => Some((p.clone(), 0)),
            Target::Std(name) => sources::std_file(name).map(|p| (p, 0)),
            Target::Lib(p) => {
                let lib = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "library".into());
                let funcs: Vec<(String, String)> = a.funcs.iter().filter(|f| f.2 == span).map(|f| (f.0.clone(), f.1.clone())).collect();
                sources::library_stub(&lib, &funcs, "").map(|(p, _)| (p, 0))
            }
        }
    }

    pub fn import_definition(&self, a: &Analysis, off: usize) -> Option<Json> {
        let (span, target) = a.import_at(off)?;
        let (path, line) = self.target_location(a, *span, target)?;
        Some(Json::obj(vec![("uri", Json::str(path_to_uri(&path))), ("range", line_range(line, line))]))
    }

    fn module_text(&self, target: &Target) -> Option<String> {
        match target {
            Target::Std(name) => STDLIB.iter().find(|s| s.name == *name).map(|s| s.src.to_string()),
            Target::File(p) => {
                let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
                self.docs
                    .iter()
                    .find(|(u, _)| std::fs::canonicalize(uri_to_path(u)).ok().as_ref() == Some(&c))
                    .map(|(_, t)| t.clone())
                    .or_else(|| std::fs::read_to_string(p).ok())
            }
            Target::Lib(_) => None,
        }
    }

    pub fn import_hover(&self, a: &Analysis, off: usize) -> Option<Json> {
        let (span, target) = a.import_at(off)?;
        let spec = a.sm.file(a.root_file).text(*span).to_string();
        let mut md = format!("```burn\nimport {}\n```\n", spec);
        match target {
            Target::Std(name) => md.push_str(&format!("Standard library module `std/{}`\n", name)),
            Target::File(p) => md.push_str(&format!("`{}`\n", p.display())),
            Target::Lib(p) => {
                md.push_str(&format!("Bytecode library `{}`\n", p.display()));
                let names: Vec<String> = a.funcs.iter().filter(|f| f.2 == *span).map(|f| format!("`{}`", f.0)).collect();
                if !names.is_empty() {
                    md.push_str(&format!("\nExports {}\n", names.join(", ")));
                }
            }
        }
        if let Some(text) = self.module_text(target) {
            let names: Vec<String> = exports_of(&text).into_iter().map(|e| format!("`{}`", e.name)).collect();
            if !names.is_empty() {
                let shown = names.len().min(40);
                md.push_str(&format!(
                    "\nExports {}{}\n",
                    names[..shown].join(", "),
                    if names.len() > shown { ", ..." } else { "" }
                ));
            }
        }
        Some(Json::obj(vec![
            ("contents", Json::obj(vec![("kind", Json::str("markdown")), ("value", Json::str(md))])),
            ("range", range_json(&a.sm, *span)),
        ]))
    }

    pub fn document_links(&self, uri: &str) -> Json {
        let Some(a) = self.analyses.get(uri) else {
            return Json::Arr(vec![]);
        };
        let file = a.sm.file(a.root_file);
        let mut out = Vec::new();
        for (span, target) in &a.links {
            if span.file != a.root_file {
                continue;
            }
            let Some((path, _)) = self.target_location(a, *span, target) else {
                continue;
            };
            let (s, e) = (span.start as usize, span.end as usize);
            let (s, e) = if file.src[s..e].starts_with('"') && e - s >= 2 {
                (s + 1, e - 1)
            } else {
                (s, e)
            };
            let tip = match target {
                Target::Std(n) => format!("Open std/{}", n),
                _ => format!("Open {}", path.display()),
            };
            out.push(Json::obj(vec![
                ("range", range(file, s, e)),
                ("target", Json::str(path_to_uri(&path))),
                ("tooltip", Json::str(tip)),
            ]));
        }
        Json::Arr(out)
    }

    pub fn folding_ranges(&self, uri: &str) -> Json {
        let Some(text) = self.docs.get(uri) else {
            return Json::Arr(vec![]);
        };
        let file = SourceFile::new(String::new(), None, text.clone());
        let line = |o: usize| file.line_utf16_col(o).0;
        let (tokens, _) = lexer::lex(text, 0);
        let mut out = Vec::new();
        let mut push = |s: usize, e: usize, kind: Option<&str>| {
            if e > s {
                let mut f = vec![("startLine", Json::num(s as f64)), ("endLine", Json::num(e as f64))];
                if let Some(k) = kind {
                    f.push(("kind", Json::str(k)));
                }
                out.push(Json::obj(f));
            }
        };
        let mut stack: Vec<usize> = Vec::new();
        for t in &tokens {
            match t.kind {
                Tok::LBrace | Tok::LParen | Tok::LBracket => stack.push(line(t.span.start as usize)),
                Tok::RBrace | Tok::RParen | Tok::RBracket => {
                    if let Some(s) = stack.pop() {
                        let e = line(t.span.start as usize);
                        if e > s + 1 {
                            push(s, e - 1, None);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut gap_start = 0;
        let mut comment_lines: Vec<usize> = Vec::new();
        for t in tokens.iter() {
            let gap = &text[gap_start..t.span.start as usize];
            let mut i = 0;
            while i < gap.len() {
                if gap[i..].starts_with("/*") {
                    let end = gap[i..].find("*/").map(|e| i + e + 2).unwrap_or(gap.len());
                    push(line(gap_start + i), line(gap_start + end - 1), Some("comment"));
                    i = end;
                } else if gap[i..].starts_with("//") {
                    comment_lines.push(line(gap_start + i));
                    i += gap[i..].find('\n').unwrap_or(gap.len() - i);
                } else {
                    i += gap[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                }
            }
            gap_start = t.span.end as usize;
        }
        let mut run: Option<(usize, usize)> = None;
        for l in comment_lines {
            run = match run {
                Some((s, e)) if l == e + 1 => Some((s, l)),
                Some((s, e)) => {
                    push(s, e, Some("comment"));
                    Some((l, l))
                }
                None => Some((l, l)),
            };
        }
        if let Some((s, e)) = run {
            push(s, e, Some("comment"));
        }
        let mut imports: Option<(usize, usize)> = None;
        let mut depth = 0i32;
        for (i, t) in tokens.iter().enumerate() {
            match t.kind {
                Tok::LBrace => depth += 1,
                Tok::RBrace => depth -= 1,
                Tok::Import if depth == 0 => {
                    let l = line(t.span.start as usize);
                    let end = match tokens.get(i + 1).map(|n| &n.kind) {
                        Some(Tok::LParen) => tokens[i + 1..]
                            .iter()
                            .find(|n| n.kind == Tok::RParen)
                            .map(|n| line(n.span.start as usize))
                            .unwrap_or(l),
                        _ => l,
                    };
                    imports = match imports {
                        Some((s, _)) => Some((s, end)),
                        None => Some((l, end)),
                    };
                }
                _ => {}
            }
        }
        if let Some((s, e)) = imports {
            push(s, e, Some("imports"));
        }
        Json::Arr(out)
    }

    pub fn document_symbols(&self, uri: &str) -> Json {
        let Some(a) = self.analyses.get(uri) else {
            return Json::Arr(vec![]);
        };
        let file = a.sm.file(a.root_file);
        let src = &file.src;
        let (tokens, _) = lexer::lex(src, 0);
        let sym = |name: &str, detail: &str, kind: i32, name_span: Span, children: Vec<Json>| {
            let start = line_start(src, name_span.start as usize);
            let end = body_end(&tokens, name_span.end).unwrap_or(name_span.end as usize).max(name_span.end as usize);
            let mut f = vec![
                ("name", Json::str(name)),
                ("kind", Json::num(kind as f64)),
                ("range", range(file, start, end)),
                ("selectionRange", range(file, name_span.start as usize, name_span.end as usize)),
            ];
            if !detail.is_empty() {
                f.push(("detail", Json::str(detail)));
            }
            if !children.is_empty() {
                f.push(("children", Json::Arr(children)));
            }
            (name_span.start, Json::obj(f))
        };
        let mine = |s: &Span| s.file == a.root_file;
        let mut out: Vec<(u32, Json)> = Vec::new();
        for t in a.type_names.iter().filter(|t| t.3 == a.root_module && mine(&t.2)) {
            let mut children: Vec<(u32, Json)> = Vec::new();
            let body = body_end(&tokens, t.2.end).unwrap_or(t.2.end as usize);
            let within = |s: &Span| mine(s) && s.start > t.2.end && (s.end as usize) <= body;
            let kind = match a.types.get(t.1) {
                Ty::Record(r) => {
                    let rec = &a.types.records[*r as usize];
                    for f in rec.fields.iter().filter(|f| within(&f.span)) {
                        children.push(sym(&f.name, &a.types.display(f.ty), 8, f.span, vec![]));
                    }
                    let prefix = format!("{}.", t.0);
                    let ctor = format!("new {}", t.0);
                    for f in a.funcs.iter().filter(|f| within(&f.2)) {
                        if let Some(m) = f.0.strip_prefix(&prefix) {
                            children.push(sym(m, &f.1, 6, f.2, vec![]));
                        } else if f.0 == ctor {
                            children.push(sym("constructor", &f.1, 9, f.2, vec![]));
                        }
                    }
                    if rec.is_class {
                        5
                    } else {
                        23
                    }
                }
                Ty::Interface(i) => {
                    for m in a.types.ifaces[*i as usize].methods.iter().filter(|m| within(&m.span)) {
                        children.push(sym(&m.name, "", 6, m.span, vec![]));
                    }
                    11
                }
                Ty::Enum(e) => {
                    for (v, s) in a.types.enums[*e as usize].variants.iter().filter(|v| within(&v.1)) {
                        children.push(sym(v, "", 22, *s, vec![]));
                    }
                    10
                }
                _ => 26,
            };
            children.sort_by_key(|c| c.0);
            out.push(sym(&t.0, "", kind, t.2, children.into_iter().map(|c| c.1).collect()));
        }
        for f in a
            .funcs
            .iter()
            .filter(|f| f.3 == a.root_module && mine(&f.2) && !f.0.contains('.') && !f.0.starts_with("new "))
        {
            out.push(sym(&f.0, &f.1, 12, f.2, vec![]));
        }
        out.sort_by_key(|o| o.0);
        Json::Arr(out.into_iter().map(|o| o.1).collect())
    }

    pub fn type_definition(&self, uri: &str, params: &Json) -> Json {
        let Some((off, a)) = self.offset(uri, params) else {
            return Json::Null;
        };
        let Some(t) = a.type_at(off) else {
            return Json::Null;
        };
        let Some(d) = a.type_decl(t) else {
            return Json::Null;
        };
        match a.uri_of(d, uri) {
            Some(u) => Json::obj(vec![("uri", Json::str(u)), ("range", range_json(&a.sm, d))]),
            None => Json::Null,
        }
    }

    pub fn implementation(&self, uri: &str, params: &Json) -> Json {
        let Some((off, a)) = self.offset(uri, params) else {
            return Json::Null;
        };
        let Some(d) = a.target_at(off) else {
            return Json::Arr(vec![]);
        };
        let mut spans: Vec<Span> = Vec::new();
        if let Some(t) = a.type_names.iter().find(|t| t.2 == d) {
            match a.types.get(t.1) {
                Ty::Interface(i) => {
                    for r in a.types.records.iter().filter(|r| r.implements.contains(i)) {
                        if let Some(s) = a.type_decl(r.ty) {
                            spans.push(s);
                        }
                    }
                }
                Ty::Record(ri) => {
                    let mut frontier = vec![*ri];
                    while let Some(p) = frontier.pop() {
                        for (ci, r) in a.types.records.iter().enumerate() {
                            if r.parent == Some(p) {
                                frontier.push(ci as u32);
                                if let Some(s) = a.type_decl(r.ty) {
                                    spans.push(s);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        } else {
            let iface_methods: Vec<Span> = a.types.ifaces.iter().flat_map(|i| i.methods.iter().map(|m| m.span)).collect();
            spans = a.related(d).into_iter().filter(|s| *s != d && !iface_methods.contains(s)).collect();
        }
        Json::Arr(
            spans
                .into_iter()
                .filter_map(|s| {
                    a.uri_of(s, uri)
                        .map(|u| Json::obj(vec![("uri", Json::str(u)), ("range", range_json(&a.sm, s))]))
                })
                .collect(),
        )
    }

    pub fn import_edit(&self, uri: &str, spec: &str) -> Option<Json> {
        let text = self.docs.get(uri)?;
        let (tokens, _) = lexer::lex(text, 0);
        let file = SourceFile::new(String::new(), None, text.clone());
        let mut insert_line: Option<usize> = None;
        let mut depth = 0i32;
        for (i, t) in tokens.iter().enumerate() {
            match t.kind {
                Tok::LBrace => depth += 1,
                Tok::RBrace => depth -= 1,
                Tok::Import if depth == 0 => {
                    let end = match tokens.get(i + 1).map(|n| &n.kind) {
                        Some(Tok::LParen) => tokens[i + 1..].iter().find(|n| n.kind == Tok::RParen).map(|n| n.span.start as usize),
                        _ => tokens.get(i + 1).map(|n| n.span.start as usize),
                    }
                    .unwrap_or(t.span.start as usize);
                    insert_line = Some(file.line_utf16_col(end).0 + 1);
                }
                _ => {}
            }
        }
        let (line, new_text) = match insert_line {
            Some(l) => (l, format!("import \"{}\"\n", spec)),
            None => (0, format!("import \"{}\"\n\n", spec)),
        };
        Some(Json::obj(vec![("range", line_range(line, line)), ("newText", Json::str(new_text))]))
    }

    fn imported(&self, a: &Analysis, std_name: Option<&str>, file: Option<&Path>) -> bool {
        a.links.iter().any(|(_, t)| match t {
            Target::Std(n) => Some(*n) == std_name,
            Target::File(p) => file.map(|f| std::fs::canonicalize(p).ok() == std::fs::canonicalize(f).ok()).unwrap_or(false),
            Target::Lib(_) => false,
        })
    }

    pub fn import_candidates(&self, uri: &str, a: &Analysis, name: &str, with_project: bool) -> Vec<String> {
        let mut out = Vec::new();
        for (module, exports) in std_exports() {
            if exports.iter().any(|e| e.name == name) && !self.imported(a, Some(module), None) {
                out.push(format!("std/{}", module));
            }
        }
        if with_project {
            let current = uri_to_path(uri);
            for f in self.project_files(&current) {
                if std::fs::canonicalize(&current).ok().as_ref() == Some(&f) || self.imported(a, None, Some(&f)) {
                    continue;
                }
                let text = self
                    .docs
                    .iter()
                    .find(|(u, _)| std::fs::canonicalize(uri_to_path(u)).ok().as_ref() == Some(&f))
                    .map(|(_, t)| t.clone())
                    .or_else(|| std::fs::read_to_string(&f).ok())
                    .unwrap_or_default();
                if exports_of(&text).iter().any(|e| e.name == name) {
                    out.push(module_spec(&current, &f));
                }
            }
        }
        out
    }

    pub fn auto_import_items(&self, uri: &str, a: &Analysis, visible: &std::collections::HashSet<String>) -> Vec<Json> {
        let mut out = Vec::new();
        for (module, exports) in std_exports() {
            if self.imported(a, Some(module), None) {
                continue;
            }
            let spec = format!("std/{}", module);
            let Some(edit) = self.import_edit(uri, &spec) else { continue };
            for e in exports {
                if visible.contains(&e.name) {
                    continue;
                }
                out.push(Json::obj(vec![
                    ("label", Json::str(e.name.clone())),
                    ("kind", Json::num(e.kind as f64)),
                    ("detail", Json::str(format!("{}  (import \"{}\")", e.detail, spec))),
                    ("labelDetails", Json::obj(vec![("description", Json::str(spec.clone()))])),
                    ("sortText", Json::str(format!("9{}", e.name))),
                    ("additionalTextEdits", Json::Arr(vec![edit.clone()])),
                ]));
            }
        }
        out
    }

    pub fn import_actions(&self, uri: &str, params: &Json) -> Vec<Json> {
        let Some(a) = self.analyses.get(uri) else {
            return vec![];
        };
        let mut out = Vec::new();
        for d in params.at(&["context", "diagnostics"]).as_arr() {
            let msg = d.get("message").as_str().unwrap_or("");
            let name = ["cannot find `", "unknown type `", "unknown struct `", "unknown interface `"]
                .iter()
                .find_map(|p| msg.strip_prefix(p))
                .and_then(|rest| rest.split('`').next())
                .map(|s| s.to_string());
            let Some(name) = name else { continue };
            let specs = self.import_candidates(uri, a, &name, true);
            let single = specs.len() == 1;
            for spec in specs {
                let Some(edit) = self.import_edit(uri, &spec) else { continue };
                out.push(Json::obj(vec![
                    ("title", Json::str(format!("Import `{}` from \"{}\"", name, spec))),
                    ("kind", Json::str("quickfix")),
                    ("isPreferred", Json::Bool(single)),
                    ("diagnostics", Json::Arr(vec![d.clone()])),
                    ("edit", Json::obj(vec![("changes", Json::obj(vec![(uri, Json::Arr(vec![edit]))]))])),
                ]));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_skip_private_and_nested_names() {
        let src = "pub fun a() {\n    fun inner() {}\n}\npriv fun b() {}\nfun c(x: int): int {\n    return x\n}\ndef struct P {\n    int x\n}\n";
        let names: Vec<String> = exports_of(src).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["a", "c", "P"]);
    }

    #[test]
    fn module_spec_is_relative() {
        assert_eq!(module_spec(Path::new("/p/src/main.bn"), Path::new("/p/src/util.bn")), "util.bn");
        assert_eq!(module_spec(Path::new("/p/src/main.bn"), Path::new("/p/lib/geo.bn")), "../lib/geo.bn");
    }
}

pub mod json;

use crate::check::{self, builtins, CheckOptions, Index};
use crate::diag::Severity;
use crate::loader::Loader;
use crate::source::{FileId, SourceMap, Span};
use crate::types::{Ty, TyId, Types, T_ARR_ANY, T_STR};
use json::Json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Analysis {
    sm: SourceMap,
    root_file: FileId,
    root_module: usize,
    index: Index,
    types: Types,
    globals: Vec<(String, TyId, usize)>,
    funcs: Vec<(String, String, Span, usize)>,
    type_names: Vec<(String, TyId, Span, usize)>,
}

#[derive(Default)]
struct Server {
    docs: HashMap<String, String>,
    analyses: HashMap<String, Analysis>,
    published: HashMap<String, Vec<String>>,
    fixes: HashMap<String, Vec<QuickFix>>,
}

struct QuickFix {
    start: usize,
    end: usize,
    title: String,
    preferred: bool,
    diagnostic: Json,
    edits: Vec<Json>,
}

fn read_message(r: &mut impl BufRead) -> Option<Json> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let t = line.trim();
        if t.is_empty() {
            break;
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            len = v.trim().parse().ok();
        }
    }
    let n = len?;
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf).ok()?;
    json::parse(&String::from_utf8_lossy(&buf)).ok()
}

fn send(msg: &Json) {
    let body = msg.encode();
    let out = std::io::stdout();
    let mut l = out.lock();
    let _ = write!(l, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = l.flush();
}

fn respond(id: &Json, result: Json) {
    send(&Json::obj(vec![("jsonrpc", Json::str("2.0")), ("id", id.clone()), ("result", result)]));
}

fn respond_err(id: &Json, code: i32, msg: &str) {
    send(&Json::obj(vec![
        ("jsonrpc", Json::str("2.0")),
        ("id", id.clone()),
        ("error", Json::obj(vec![("code", Json::num(code)), ("message", Json::str(msg))])),
    ]));
}

fn notify(method: &str, params: Json) {
    send(&Json::obj(vec![
        ("jsonrpc", Json::str("2.0")),
        ("method", Json::str(method)),
        ("params", params),
    ]));
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

pub fn uri_to_path(uri: &str) -> PathBuf {
    let rest = uri.strip_prefix("file://").unwrap_or(uri);
    let bytes = rest.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(a * 16 + b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let mut s = String::from_utf8_lossy(&out).into_owned();
    if s.len() > 2 && s.as_bytes()[0] == b'/' && s.as_bytes()[2] == b':' {
        s.remove(0);
    }
    PathBuf::from(s)
}

pub fn path_to_uri(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn pos_json(sm: &SourceMap, file: FileId, offset: usize) -> Json {
    let (l, c) = sm.file(file).line_utf16_col(offset);
    Json::obj(vec![("line", Json::num(l as f64)), ("character", Json::num(c as f64))])
}

fn range_json(sm: &SourceMap, span: Span) -> Json {
    let end = if span.end <= span.start { span.start + 1 } else { span.end };
    let end = (end as usize).min(sm.file(span.file).src.len());
    Json::obj(vec![
        ("start", pos_json(sm, span.file, span.start as usize)),
        ("end", pos_json(sm, span.file, end)),
    ])
}

fn hover_markdown(text: &str) -> String {
    match text.split_once('\u{1}') {
        Some((code, doc)) => format!("```burn\n{}\n```\n---\n{}", code, doc),
        None => format!("```burn\n{}\n```", text),
    }
}

fn edit_range_json(sm: &SourceMap, span: Span) -> Json {
    Json::obj(vec![
        ("start", pos_json(sm, span.file, span.start as usize)),
        ("end", pos_json(sm, span.file, span.end as usize)),
    ])
}

const KEYWORDS: &[&str] = &[
    "fun",
    "var",
    "const",
    "def",
    "type",
    "interface",
    "struct",
    "abstract",
    "new",
    "destroy",
    "enum",
    "annotation",
    "static",
    "if",
    "else",
    "while",
    "for",
    "in",
    "return",
    "break",
    "continue",
    "true",
    "false",
    "null",
    "import",
    "pub",
    "priv",
    "async",
    "await",
    "is",
    "as",
    "self",
];

const STR_METHODS: &[&str] = &[
    "length",
    "upper",
    "lower",
    "trim",
    "split",
    "contains",
    "indexOf",
    "replace",
    "startsWith",
    "endsWith",
    "repeat",
    "chars",
    "substring",
    "charAt",
    "charCode",
];
const ARR_METHODS: &[&str] = &[
    "length", "push", "pop", "insert", "remove", "contains", "indexOf", "join", "reverse", "sort", "slice", "copy", "clear",
];
const MAP_METHODS: &[&str] = &["length", "keys", "values", "has", "get", "remove"];

impl Server {
    fn analyze(&mut self, uri: &str) {
        let path = uri_to_path(uri);
        let mut loader = Loader::new();
        for (u, text) in &self.docs {
            let p = uri_to_path(u);
            let c = std::fs::canonicalize(&p).unwrap_or(p);
            loader.overrides.insert(c, text.clone());
        }
        let root = match loader.load_file(&path) {
            Ok(r) => r,
            Err(_) => {
                let text = self.docs.get(uri).cloned().unwrap_or_default();
                loader.load_source(
                    &path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    text,
                    path.parent().map(|p| p.to_path_buf()),
                )
            }
        };
        let loaded = loader.finish(root);
        let root_file = loaded.modules[root].file;
        let result = check::check(
            &loaded,
            CheckOptions {
                skip_before: None,
                want_index: true,
                repl_echo: false,
            },
        );
        let mut diags = loaded.diags.clone();
        diags.extend(result.diags);
        let sm = loaded.sm;
        let mut by_uri: HashMap<String, Vec<Json>> = HashMap::new();
        let mut fixes = Vec::new();
        by_uri.insert(uri.to_string(), Vec::new());
        for d in &diags {
            if (d.span.file as usize) >= sm.files.len() {
                continue;
            }
            let file = sm.file(d.span.file);
            let target = if d.span.file == root_file {
                uri.to_string()
            } else {
                match &file.path {
                    Some(p) => path_to_uri(p),
                    None => continue,
                }
            };
            let mut message = d.message.clone();
            for n in d.notes.iter().chain(d.helps.iter()) {
                message.push('\n');
                message.push_str(n);
            }
            let dj = Json::obj(vec![
                ("range", range_json(&sm, d.span)),
                ("severity", Json::num(if d.severity == Severity::Error { 1 } else { 2 })),
                ("source", Json::str("burn")),
                ("message", Json::str(message)),
            ]);
            if d.span.file == root_file {
                for s in &d.suggestions {
                    if s.edits.iter().any(|(sp, _)| sp.file != root_file) {
                        continue;
                    }
                    fixes.push(QuickFix {
                        start: d.span.start as usize,
                        end: d.span.end as usize,
                        title: s.message.clone(),
                        preferred: s.applicable,
                        diagnostic: dj.clone(),
                        edits: s
                            .edits
                            .iter()
                            .map(|(sp, t)| Json::obj(vec![("range", edit_range_json(&sm, *sp)), ("newText", Json::str(t.clone()))]))
                            .collect(),
                    });
                }
            }
            by_uri.entry(target).or_default().push(dj);
        }
        let previous = self.published.remove(uri).unwrap_or_default();
        let mut now = Vec::new();
        for (u, ds) in by_uri {
            now.push(u.clone());
            notify(
                "textDocument/publishDiagnostics",
                Json::obj(vec![("uri", Json::str(u)), ("diagnostics", Json::Arr(ds))]),
            );
        }
        for u in previous {
            if !now.contains(&u) {
                notify(
                    "textDocument/publishDiagnostics",
                    Json::obj(vec![("uri", Json::str(u)), ("diagnostics", Json::Arr(vec![]))]),
                );
            }
        }
        self.published.insert(uri.to_string(), now);
        self.fixes.insert(uri.to_string(), fixes);
        self.analyses.insert(
            uri.to_string(),
            Analysis {
                sm,
                root_file,
                root_module: root,
                index: result.index,
                types: result.types,
                globals: result.globals,
                funcs: result.funcs,
                type_names: result.type_names,
            },
        );
    }

    fn code_actions(&self, uri: &str, params: &Json) -> Json {
        let (Some(a), Some(fixes)) = (self.analyses.get(uri), self.fixes.get(uri)) else {
            return Json::Arr(vec![]);
        };
        let f = a.sm.file(a.root_file);
        let pos = |k: &str| {
            let line = params.at(&["range", k, "line"]).as_f64().unwrap_or(0.0) as usize;
            let ch = params.at(&["range", k, "character"]).as_f64().unwrap_or(0.0) as usize;
            f.offset_of_utf16(line, ch)
        };
        let (start, end) = (pos("start"), pos("end"));
        let mut out = Vec::new();
        for q in fixes {
            if q.start <= end && start <= q.end {
                out.push(Json::obj(vec![
                    ("title", Json::str(q.title.clone())),
                    ("kind", Json::str("quickfix")),
                    ("isPreferred", Json::Bool(q.preferred)),
                    ("diagnostics", Json::Arr(vec![q.diagnostic.clone()])),
                    ("edit", Json::obj(vec![("changes", Json::obj(vec![(uri, Json::Arr(q.edits.clone()))]))])),
                ]));
            }
        }
        Json::Arr(out)
    }

    fn offset(&self, uri: &str, params: &Json) -> Option<(usize, &Analysis)> {
        let a = self.analyses.get(uri)?;
        let line = params.at(&["position", "line"]).as_f64()? as usize;
        let ch = params.at(&["position", "character"]).as_f64()? as usize;
        Some((a.sm.file(a.root_file).offset_of_utf16(line, ch), a))
    }

    fn hover(&self, uri: &str, params: &Json) -> Json {
        let (off, a) = match self.offset(uri, params) {
            Some(x) => x,
            None => return Json::Null,
        };
        let mut best: Option<&(Span, String)> = None;
        for h in &a.index.hovers {
            if h.0.file == a.root_file
                && (h.0.start as usize) <= off
                && off <= (h.0.end as usize)
                && best.map(|b| (h.0.end - h.0.start) < (b.0.end - b.0.start)).unwrap_or(true)
            {
                best = Some(h);
            }
        }
        match best {
            Some((span, text)) => Json::obj(vec![
                (
                    "contents",
                    Json::obj(vec![("kind", Json::str("markdown")), ("value", Json::str(hover_markdown(text)))]),
                ),
                ("range", range_json(&a.sm, *span)),
            ]),
            None => {
                let src = &a.sm.file(a.root_file).src;
                let word = word_at(src, off);
                if builtins::is_builtin(&word) {
                    return Json::obj(vec![(
                        "contents",
                        Json::obj(vec![
                            ("kind", Json::str("markdown")),
                            (
                                "value",
                                Json::str(match crate::doc::builtins::find(&word) {
                                    Some(b) => hover_markdown(&format!("{}\u{1}{}", b.sig, crate::doc::comment::to_markdown(&b.doc))),
                                    None => format!("```burn\n{}\n```", builtins::signature(&word)),
                                }),
                            ),
                        ]),
                    )]);
                }
                Json::Null
            }
        }
    }

    fn definition(&self, uri: &str, params: &Json) -> Json {
        let (off, a) = match self.offset(uri, params) {
            Some(x) => x,
            None => return Json::Null,
        };
        for (use_span, def) in &a.index.defs {
            if use_span.file == a.root_file && (use_span.start as usize) <= off && off <= (use_span.end as usize) {
                let target = if def.file == a.root_file {
                    uri.to_string()
                } else {
                    match &a.sm.file(def.file).path {
                        Some(p) => path_to_uri(p),
                        None => return Json::Null,
                    }
                };
                return Json::obj(vec![("uri", Json::str(target)), ("range", range_json(&a.sm, *def))]);
            }
        }
        Json::Null
    }

    fn member_items(&self, a: &Analysis, t: TyId, statics: bool) -> Vec<Json> {
        let mut items = Vec::new();
        let item =
            |label: &str, kind: i32, detail: String| Json::obj(vec![("label", Json::str(label)), ("kind", Json::num(kind)), ("detail", Json::str(detail))]);
        match a.types.get(t) {
            Ty::Record(r) => {
                let rec = &a.types.records[*r as usize];
                if statics {
                    let mut names: Vec<&String> = rec.statics.keys().collect();
                    names.sort();
                    for n in names {
                        items.push(item(n, 2, format!("static fun {}.{}", rec.name, n)));
                    }
                } else {
                    for f in &rec.fields {
                        items.push(item(&f.name, 5, a.types.display(f.ty)));
                    }
                    let mut names: Vec<&String> = rec.methods.keys().collect();
                    names.sort();
                    for n in names {
                        items.push(item(n, 2, format!("fun {}.{}", rec.name, n)));
                    }
                }
            }
            Ty::Interface(i) => {
                for m in &a.types.ifaces[*i as usize].methods {
                    items.push(item(&m.name, 2, format!("fun {}", m.name)));
                }
            }
            Ty::Enum(e) if statics => {
                for (v, _) in &a.types.enums[*e as usize].variants {
                    items.push(item(v, 20, a.types.enums[*e as usize].name.clone()));
                }
            }
            Ty::Str => STR_METHODS.iter().for_each(|m| items.push(item(m, 2, "string".into()))),
            Ty::Array(_) => ARR_METHODS.iter().for_each(|m| items.push(item(m, 2, a.types.display(t)))),
            Ty::Map(..) => MAP_METHODS.iter().for_each(|m| items.push(item(m, 2, a.types.display(t)))),
            _ => {}
        }
        items
    }

    fn resolve_chain(&self, a: &Analysis, chain: &[String], off: usize) -> Option<(TyId, bool)> {
        let first = chain.first()?;
        let mut cur: Option<(TyId, bool)> = None;
        let mut best: Option<&check::LocalInfo> = None;
        for l in &a.index.locals {
            if l.name == *first
                && l.decl.file == a.root_file
                && (l.decl.start as usize) < off
                && off <= l.scope.end as usize
                && best.map(|b| l.decl.start > b.decl.start).unwrap_or(true)
            {
                best = Some(l);
            }
        }
        if let Some(l) = best {
            cur = Some((l.ty, false));
        }
        if cur.is_none() {
            if let Some(g) = a.globals.iter().find(|g| g.0 == *first && g.2 == a.root_module) {
                cur = Some((g.1, false));
            }
        }
        if cur.is_none() {
            if let Some(t) = a.type_names.iter().find(|t| t.0 == *first) {
                cur = Some((t.1, true));
            }
        }
        if cur.is_none() && first == "self" {
            for (span, t) in &a.index.expr_types {
                if span.file == a.root_file && (span.start as usize) < off {
                    let text = &a.sm.file(a.root_file).src[span.start as usize..span.end as usize];
                    if text == "self" {
                        cur = Some((*t, false));
                    }
                }
            }
        }
        let mut cur = cur?;
        for seg in &chain[1..] {
            let (t, _) = cur;
            let next = match a.types.get(t) {
                Ty::Record(r) => a.types.records[*r as usize].fields.iter().find(|f| f.name == *seg).map(|f| f.ty),
                Ty::Str if seg == "length" => Some(crate::types::T_INT),
                _ => None,
            }?;
            cur = (next, false);
        }
        Some(cur)
    }

    fn completion(&self, uri: &str, params: &Json) -> Json {
        let text = match self.docs.get(uri) {
            Some(t) => t.clone(),
            None => return Json::Arr(vec![]),
        };
        let a = match self.analyses.get(uri) {
            Some(a) => a,
            None => return Json::Arr(vec![]),
        };
        let line = params.at(&["position", "line"]).as_f64().unwrap_or(0.0) as usize;
        let ch = params.at(&["position", "character"]).as_f64().unwrap_or(0.0) as usize;
        let tmp = crate::source::SourceFile::new(String::new(), None, text.clone());
        let off = tmp.offset_of_utf16(line, ch);
        let before = &text[..off];
        let mut start = before.len();
        for (i, c) in before.char_indices().rev() {
            if c.is_alphanumeric() || c == '_' {
                start = i;
            } else {
                break;
            }
        }
        let prefix_end = start;
        if before[..prefix_end].ends_with('.') {
            let mut chain = Vec::new();
            let mut rest = &before[..prefix_end - 1];
            loop {
                let mut s = rest.len();
                for (i, c) in rest.char_indices().rev() {
                    if c.is_alphanumeric() || c == '_' {
                        s = i;
                    } else {
                        break;
                    }
                }
                if s == rest.len() {
                    break;
                }
                chain.insert(0, rest[s..].to_string());
                if s > 0 && rest[..s].ends_with('.') {
                    rest = &rest[..s - 1];
                } else {
                    break;
                }
            }
            if let Some((t, statics)) = self.resolve_chain(a, &chain, off) {
                return Json::Arr(self.member_items(a, t, statics));
            }
            return Json::Arr(self.member_items(a, T_STR, false).into_iter().take(0).collect());
        }
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut push = |label: &str, kind: i32, detail: String, items: &mut Vec<Json>| {
            if seen.insert(label.to_string()) {
                items.push(Json::obj(vec![
                    ("label", Json::str(label)),
                    ("kind", Json::num(kind)),
                    ("detail", Json::str(detail)),
                ]));
            }
        };
        for l in a.index.locals.iter().rev() {
            if l.decl.file == a.root_file && (l.decl.start as usize) < off && off <= l.scope.end as usize {
                push(&l.name, 6, a.types.display(l.ty), &mut items);
            }
        }
        for g in &a.globals {
            if g.2 == a.root_module {
                push(&g.0, 6, a.types.display(g.1), &mut items);
            }
        }
        for f in &a.funcs {
            if !f.0.contains('.') && !f.0.starts_with('<') {
                push(&f.0, 3, f.1.clone(), &mut items);
            }
        }
        for t in &a.type_names {
            let kind = match a.types.get(t.1) {
                Ty::Interface(_) => 8,
                Ty::Enum(_) => 13,
                Ty::Record(r) if a.types.records[*r as usize].is_class => 7,
                _ => 22,
            };
            push(&t.0, kind, a.types.display(t.1), &mut items);
        }
        for b in builtins::BUILTINS {
            if !b.starts_with("__") {
                push(b, 3, builtins::signature(b).to_string(), &mut items);
            }
        }
        for k in KEYWORDS {
            push(k, 14, "keyword".into(), &mut items);
        }
        for t in ["int", "float", "string", "bool", "any", "void"] {
            push(t, 22, "built-in type".into(), &mut items);
        }
        let _ = T_ARR_ANY;
        Json::Arr(items)
    }

    fn symbols(&self, uri: &str) -> Json {
        let a = match self.analyses.get(uri) {
            Some(a) => a,
            None => return Json::Arr(vec![]),
        };
        let mut out = Vec::new();
        let loc = |span: Span| Json::obj(vec![("uri", Json::str(uri)), ("range", range_json(&a.sm, span))]);
        for f in &a.funcs {
            if f.3 == a.root_module && f.2.file == a.root_file && !f.0.starts_with('<') {
                let kind = if f.0.contains('.') { 6 } else { 12 };
                out.push(Json::obj(vec![
                    ("name", Json::str(f.0.clone())),
                    ("kind", Json::num(kind)),
                    ("location", loc(f.2)),
                ]));
            }
        }
        for t in &a.type_names {
            if t.3 == a.root_module && t.2.file == a.root_file {
                let kind = match a.types.get(t.1) {
                    Ty::Interface(_) => 11,
                    Ty::Enum(_) => 10,
                    Ty::Record(r) if a.types.records[*r as usize].is_class => 5,
                    _ => 23,
                };
                out.push(Json::obj(vec![
                    ("name", Json::str(t.0.clone())),
                    ("kind", Json::num(kind)),
                    ("location", loc(t.2)),
                ]));
            }
        }
        Json::Arr(out)
    }

    fn formatting(&self, uri: &str) -> Json {
        let text = match self.docs.get(uri) {
            Some(t) => t,
            None => return Json::Null,
        };
        let formatted = crate::fmt::format(text);
        if &formatted == text {
            return Json::Arr(vec![]);
        }
        let lines = text.lines().count() + 1;
        Json::Arr(vec![Json::obj(vec![
            (
                "range",
                Json::obj(vec![
                    ("start", Json::obj(vec![("line", Json::num(0)), ("character", Json::num(0))])),
                    ("end", Json::obj(vec![("line", Json::num(lines as f64)), ("character", Json::num(0))])),
                ]),
            ),
            ("newText", Json::str(formatted)),
        ])])
    }
}

fn word_at(src: &str, off: usize) -> String {
    let b = src.as_bytes();
    let mut s = off.min(b.len());
    while s > 0 && (b[s - 1].is_ascii_alphanumeric() || b[s - 1] == b'_') {
        s -= 1;
    }
    let mut e = off.min(b.len());
    while e < b.len() && (b[e].is_ascii_alphanumeric() || b[e] == b'_') {
        e += 1;
    }
    src[s..e].to_string()
}

pub fn run() -> ExitCode {
    burn_runtime::io::set_panic_mode(true);
    crate::repl::install_quiet_hook();
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut server = Server::default();
    let mut shutdown = false;
    while let Some(msg) = read_message(&mut reader) {
        let method = msg.get("method").as_str().unwrap_or("").to_string();
        let id = msg.get("id").clone();
        let params = msg.get("params").clone();
        let uri = params.at(&["textDocument", "uri"]).as_str().unwrap_or("").to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match method.as_str() {
            "initialize" => {
                respond(
                    &id,
                    Json::obj(vec![
                        (
                            "capabilities",
                            Json::obj(vec![
                                ("textDocumentSync", Json::num(1)),
                                ("hoverProvider", Json::Bool(true)),
                                ("definitionProvider", Json::Bool(true)),
                                ("documentSymbolProvider", Json::Bool(true)),
                                ("documentFormattingProvider", Json::Bool(true)),
                                ("codeActionProvider", Json::Bool(true)),
                                ("completionProvider", Json::obj(vec![("triggerCharacters", Json::Arr(vec![Json::str(".")]))])),
                            ]),
                        ),
                        (
                            "serverInfo",
                            Json::obj(vec![("name", Json::str("burn")), ("version", Json::str(env!("CARGO_PKG_VERSION")))]),
                        ),
                    ]),
                );
            }
            "initialized" | "$/cancelRequest" | "workspace/didChangeConfiguration" | "workspace/didChangeWatchedFiles" => {}
            "shutdown" => {
                shutdown = true;
                respond(&id, Json::Null);
            }
            "textDocument/didOpen" => {
                let text = params.at(&["textDocument", "text"]).as_str().unwrap_or("").to_string();
                server.docs.insert(uri.clone(), text);
                server.analyze(&uri);
            }
            "textDocument/didChange" => {
                if let Some(last) = params.get("contentChanges").as_arr().last() {
                    if let Some(t) = last.get("text").as_str() {
                        server.docs.insert(uri.clone(), t.to_string());
                    }
                }
                server.analyze(&uri);
            }
            "textDocument/didSave" => server.analyze(&uri),
            "textDocument/didClose" => {
                server.docs.remove(&uri);
                server.analyses.remove(&uri);
                if let Some(list) = server.published.remove(&uri) {
                    for u in list {
                        notify(
                            "textDocument/publishDiagnostics",
                            Json::obj(vec![("uri", Json::str(u)), ("diagnostics", Json::Arr(vec![]))]),
                        );
                    }
                }
            }
            "textDocument/hover" => respond(&id, server.hover(&uri, &params)),
            "textDocument/definition" => respond(&id, server.definition(&uri, &params)),
            "textDocument/completion" => respond(&id, server.completion(&uri, &params)),
            "textDocument/documentSymbol" => respond(&id, server.symbols(&uri)),
            "textDocument/formatting" => respond(&id, server.formatting(&uri)),
            "textDocument/codeAction" => respond(&id, server.code_actions(&uri, &params)),
            _ => {
                if !id.is_null() {
                    respond_err(&id, -32601, &format!("method not found: {}", method));
                }
            }
        }));
        if result.is_err() && !id.is_null() {
            respond_err(&id, -32603, "internal error");
        }
        if method == "exit" {
            return if shutdown { ExitCode::SUCCESS } else { ExitCode::from(1) };
        }
    }
    ExitCode::SUCCESS
}

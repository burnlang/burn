mod assist;
mod complete;
mod ide;
pub mod json;
mod nav;
mod reload;
mod repair;
pub mod sources;

use crate::check::{self, builtins, CheckOptions, Index};
use crate::diag::Severity;
use crate::loader::Loader;
use crate::source::{FileId, SourceMap, Span};
use crate::types::{Ty, TyId, Types, T_ARR_ANY, T_ERROR};
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
    links: Vec<(Span, ide::Target)>,
    inserts: Vec<usize>,
    no_std: bool,
}

#[derive(Default)]
struct Server {
    docs: HashMap<String, String>,
    analyses: HashMap<String, Analysis>,
    published: HashMap<String, Vec<String>>,
    fixes: HashMap<String, Vec<QuickFix>>,
    roots: Vec<PathBuf>,
    watch: bool,
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

fn import_prefix(before: &str) -> Option<String> {
    let line = before.rsplit('\n').next().unwrap_or("");
    let quote = line.rfind('"')?;
    if line[..quote].matches('"').count() % 2 != 0 {
        return None;
    }
    let head = line[..quote].trim();
    let in_block = head.is_empty() && {
        let prior = &before[..before.len() - line.len()];
        match (prior.rfind("import ("), prior.rfind(')')) {
            (Some(open), Some(close)) => open > close,
            (Some(_), None) => true,
            _ => false,
        }
    };
    if head == "import" || in_block {
        Some(line[quote + 1..].to_string())
    } else {
        None
    }
}

fn import_items(file: &Path, typed: &str) -> Vec<Json> {
    let mut out: Vec<Json> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut add = |label: String, kind: i32, detail: &str, out: &mut Vec<Json>| {
        if label.to_lowercase().starts_with(&typed.to_lowercase()) && seen.insert(label.clone()) {
            out.push(Json::obj(vec![
                ("label", Json::str(&label)),
                ("kind", Json::num(kind)),
                ("detail", Json::str(detail)),
            ]));
        }
    };
    let project = crate::project::find_root(file).and_then(|root| crate::project::load(&root).ok());
    if project.as_ref().map(|p| p.manifest.std).unwrap_or(true) {
        for s in crate::loader::STDLIB {
            add(s.name.to_string(), 9, "standard library", &mut out);
        }
    }
    if let Some(p) = project {
        add(p.manifest.name.clone(), 9, "this project", &mut out);
        for (d, _) in &p.manifest.dependencies {
            add(d.clone(), 9, "dependency", &mut out);
        }
        for l in &p.lock {
            add(l.name.clone(), 9, "installed package", &mut out);
        }
        if let Some((name, _)) = crate::project::split_package_path(typed.trim_end_matches('/')) {
            if let Ok(dir) = p.resolve(&name) {
                let sub = typed[name.len()..].trim_start_matches('/');
                let (folder, _) = sub.rsplit_once('/').unwrap_or(("", sub));
                let base = if folder.is_empty() { name.clone() } else { format!("{}/{}", name, folder) };
                list_sources(&dir.join(folder), &base, &mut |l, k| add(l, k, "package file", &mut out));
            }
        }
    }
    if let Some(dir) = file.parent() {
        let (folder, _) = typed.rsplit_once('/').unwrap_or(("", typed));
        let base = folder.to_string();
        let target = if folder.is_empty() { dir.to_path_buf() } else { dir.join(folder) };
        let own = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        list_sources(&target, &base, &mut |l, k| {
            if !(folder.is_empty() && l == own) {
                add(l, k, "file", &mut out)
            }
        });
    }
    out
}

fn list_sources(dir: &Path, base: &str, add: &mut dyn FnMut(String, i32)) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<(String, bool)> = rd
        .filter_map(|e| e.ok())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path().is_dir()))
        .filter(|(n, _)| !n.starts_with('.') && n != "build" && n != "target" && n != "node_modules")
        .collect();
    names.sort();
    for (n, is_dir) in names.into_iter().take(200) {
        let full = if base.is_empty() { n.clone() } else { format!("{}/{}", base, n) };
        if is_dir {
            add(format!("{}/", full), 19);
        } else if let Some(stem) = full.strip_suffix(".bn") {
            add(stem.to_string(), 17);
        } else if full.ends_with(".bvmc") || full.ends_with(".bar") {
            add(full, 17);
        }
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
    "match",
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
    fn loader(&self) -> Loader {
        let mut loader = Loader::new();
        for (u, text) in &self.docs {
            let p = uri_to_path(u);
            let c = std::fs::canonicalize(&p).unwrap_or(p);
            loader.overrides.insert(c, text.clone());
        }
        loader
    }

    fn build(loaded: crate::loader::Loaded, root: usize, inserts: Vec<usize>) -> (Analysis, Vec<crate::diag::Diagnostic>) {
        let root_file = loaded.modules[root].file;
        let result = check::check(
            &loaded,
            CheckOptions {
                skip_before: None,
                want_index: true,
                repl_echo: false,
            },
        );
        let links = ide::import_links(&loaded, root);
        let no_std = loaded.no_std;
        let mut diags = loaded.diags;
        diags.extend(result.diags);
        (
            Analysis {
                sm: loaded.sm,
                root_file,
                root_module: root,
                index: result.index,
                types: result.types,
                globals: result.globals,
                funcs: result.funcs,
                type_names: result.type_names,
                links,
                inserts,
                no_std,
            },
            diags,
        )
    }

    fn quick_analysis(&self, path: &Path) -> Option<Analysis> {
        let mut loader = self.loader();
        let root = loader.load_file(path).ok()?;
        let loaded = loader.finish(root);
        Some(Self::build(loaded, root, Vec::new()).0)
    }

    fn repaired(&self, path: &Path, text: &str, errors: usize) -> Option<Analysis> {
        let (fixed, inserts) = repair::close_braces(text)?;
        let mut loader = self.loader();
        let c = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        loader.overrides.insert(c, fixed.clone());
        let root = match loader.load_file(path) {
            Ok(r) => r,
            Err(_) => loader.load_source(
                &path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                fixed,
                path.parent().map(|p| p.to_path_buf()),
            ),
        };
        let loaded = loader.finish(root);
        let root_file = loaded.modules[root].file;
        if loaded
            .diags
            .iter()
            .filter(|d| d.span.file == root_file && d.severity == Severity::Error)
            .count()
            >= errors
        {
            return None;
        }
        Some(Self::build(loaded, root, inserts).0)
    }

    fn analyze(&mut self, uri: &str) {
        let path = uri_to_path(uri);
        if sources::is_stub(&path) {
            notify(
                "textDocument/publishDiagnostics",
                Json::obj(vec![("uri", Json::str(uri)), ("diagnostics", Json::Arr(vec![]))]),
            );
            return;
        }
        let mut loader = self.loader();
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
        let syntax_errors = loaded
            .diags
            .iter()
            .filter(|d| d.span.file == root_file && d.severity == Severity::Error)
            .count();
        let (analysis, diags) = Self::build(loaded, root, Vec::new());
        let sm = &analysis.sm;
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
                ("range", range_json(sm, d.span)),
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
                            .map(|(sp, t)| Json::obj(vec![("range", edit_range_json(sm, *sp)), ("newText", Json::str(t.clone()))]))
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
        let better = if syntax_errors > 0 {
            self.docs.get(uri).and_then(|text| self.repaired(&path, text, syntax_errors))
        } else {
            None
        };
        self.analyses.insert(uri.to_string(), better.unwrap_or(analysis));
    }

    fn code_actions(&self, uri: &str, params: &Json) -> Json {
        let (Some(a), Some(fixes)) = (self.analyses.get(uri), self.fixes.get(uri)) else {
            return Json::Arr(vec![]);
        };
        let only: Vec<String> = params
            .at(&["context", "only"])
            .as_arr()
            .iter()
            .filter_map(|k| k.as_str().map(|s| s.to_string()))
            .collect();
        let wants = |kind: &str| only.is_empty() || only.iter().any(|o| kind == o || kind.starts_with(&format!("{}.", o)));
        let edit_of = |edits: Vec<Json>| Json::obj(vec![("changes", Json::obj(vec![(uri, Json::Arr(edits))]))]);
        let mut out = Vec::new();
        if wants("quickfix") {
            let f = a.sm.file(a.root_file);
            let pos = |k: &str| {
                let line = params.at(&["range", k, "line"]).as_f64().unwrap_or(0.0) as usize;
                let ch = params.at(&["range", k, "character"]).as_f64().unwrap_or(0.0) as usize;
                repair::unshift(&a.inserts, f.offset_of_utf16(line, ch))
            };
            let (start, end) = (pos("start"), pos("end"));
            for q in fixes {
                if q.start <= end && start <= q.end {
                    out.push(Json::obj(vec![
                        ("title", Json::str(q.title.clone())),
                        ("kind", Json::str("quickfix")),
                        ("isPreferred", Json::Bool(q.preferred)),
                        ("diagnostics", Json::Arr(vec![q.diagnostic.clone()])),
                        ("edit", edit_of(q.edits.clone())),
                    ]));
                }
            }
            out.extend(self.import_actions(uri, params));
            out.extend(self.reload_actions(uri, params));
        }
        let mut all: Vec<&QuickFix> = Vec::new();
        for q in fixes.iter().filter(|q| q.preferred) {
            if all.iter().all(|p| q.end < p.start || p.end < q.start) {
                all.push(q);
            }
        }
        if !all.is_empty() && (wants("source.fixAll.burn") || (wants("quickfix") && all.len() > 1 && only.is_empty())) {
            let kind = if only.iter().any(|o| o.starts_with("source")) {
                "source.fixAll.burn"
            } else {
                "quickfix"
            };
            out.push(Json::obj(vec![
                ("title", Json::str(format!("Fix all auto-fixable problems in this file ({})", all.len()))),
                ("kind", Json::str(kind)),
                ("edit", edit_of(all.iter().flat_map(|q| q.edits.clone()).collect())),
            ]));
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
        if let Some(h) = self.import_hover(a, off) {
            return h;
        }
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
                if builtins::is_builtin(&word) && !word.starts_with("__") {
                    return Json::obj(vec![(
                        "contents",
                        Json::obj(vec![
                            ("kind", Json::str("markdown")),
                            (
                                "value",
                                Json::str(match crate::doc::builtins::find(&word) {
                                    Some(b) => hover_markdown(&format!("{}\u{1}{}", b.sig, b.markdown())),
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

    fn member_items(&self, uri: &str, a: &Analysis, t: TyId, statics: bool) -> Vec<Json> {
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut add = |label: &str, kind: i32, detail: String, doc: Option<String>, call: bool, items: &mut Vec<Json>| {
            if !seen.insert(label.to_string()) {
                return;
            }
            let mut f = vec![
                ("label", Json::str(label)),
                ("kind", Json::num(kind as f64)),
                ("detail", Json::str(detail)),
                ("sortText", Json::str(format!("{}{}", if kind == 5 { 0 } else { 1 }, label))),
            ];
            if let Some(d) = doc {
                f.push(("documentation", Json::obj(vec![("kind", Json::str("markdown")), ("value", Json::str(d))])));
            }
            if call {
                f.push(("insertText", Json::str(format!("{}($0)", label))));
                f.push(("insertTextFormat", Json::num(2)));
                f.push((
                    "command",
                    Json::obj(vec![("title", Json::str("")), ("command", Json::str("editor.action.triggerParameterHints"))]),
                ));
            }
            items.push(Json::obj(f));
        };
        let sig_of = |full: &str| a.funcs.iter().find(|f| f.0 == full).map(|f| f.1.clone());
        match a.types.get(t) {
            Ty::Record(r) => {
                let mut cur = Some(*r);
                while let Some(ri) = cur {
                    let rec = &a.types.records[ri as usize];
                    if statics {
                        let mut names: Vec<&String> = rec.statics.keys().collect();
                        names.sort();
                        for n in names {
                            let full = format!("{}.{}", rec.name, n);
                            add(n, 2, sig_of(&full).unwrap_or(format!("static fun {}", full)), None, true, &mut items);
                        }
                    } else {
                        for f in &rec.fields {
                            add(&f.name, 5, a.types.display(f.ty), None, false, &mut items);
                        }
                        let mut names: Vec<&String> = rec.methods.keys().collect();
                        names.sort();
                        for n in names {
                            let full = format!("{}.{}", rec.name, n);
                            add(n, 2, sig_of(&full).unwrap_or(format!("fun {}", full)), None, true, &mut items);
                        }
                    }
                    cur = rec.parent;
                }
                if statics {
                    return items;
                }
            }
            Ty::Interface(i) if statics && !a.types.ifaces[*i as usize].variants.is_empty() => {
                let en = &a.types.ifaces[*i as usize];
                for v in &en.variants {
                    let rec = &a.types.records[v.record as usize];
                    let fs: Vec<String> = rec.fields.iter().map(|f| format!("{}: {}", f.name, a.types.display(f.ty))).collect();
                    let detail = if fs.is_empty() {
                        en.name.clone()
                    } else {
                        format!("{}.{}({})", en.name, v.name, fs.join(", "))
                    };
                    add(&v.name, 20, detail, None, false, &mut items);
                }
                return items;
            }
            Ty::Interface(i) => {
                for m in &a.types.ifaces[*i as usize].methods {
                    let ps: Vec<String> = m.params.iter().map(|p| a.types.display(*p)).collect();
                    add(
                        &m.name,
                        2,
                        format!("fun {}({}): {}", m.name, ps.join(", "), a.types.display(m.ret)),
                        None,
                        true,
                        &mut items,
                    );
                }
                return items;
            }
            Ty::Enum(e) if statics => {
                for (v, _) in &a.types.enums[*e as usize].variants {
                    add(v, 20, a.types.enums[*e as usize].name.clone(), None, false, &mut items);
                }
                return items;
            }
            _ if statics => return items,
            _ => {}
        }
        let shown = a.types.display(t);
        let receiver = |first: &str| -> bool {
            let first = first.trim();
            match a.types.get(t) {
                Ty::Str => first == "string",
                Ty::Array(_) => first == "[T]" || first == shown,
                Ty::Map(..) => first.starts_with('{'),
                Ty::Bool => first == "bool",
                _ if a.types.is_numeric(t) => first == "float" || first == "int" || first == shown,
                _ => first == shown,
            }
        };
        for b in crate::doc::builtins::all() {
            let Some(params) = b.sig.split_once('(').map(|(_, r)| r) else { continue };
            let first = params.split([',', ')']).next().unwrap_or("");
            let ty = first.split_once(':').map(|(_, t)| t.split('=').next().unwrap_or("")).unwrap_or("");
            let returns = b.sig.rsplit_once(')').map(|(_, r)| r.trim().starts_with(':')).unwrap_or(false);
            if !b.name.starts_with("__") && !ty.trim().is_empty() && receiver(ty) && (returns || !a.types.is_numeric(t)) {
                add(&b.name, 2, b.sig.clone(), Some(b.markdown()), true, &mut items);
                if let (Some(m), Some(Json::Obj(fields))) = (b.module(), items.last_mut()) {
                    if !self.imported(a, Some(m), None) && fields.iter().any(|(k, v)| k == "label" && v.as_str() == Some(b.name.as_str())) {
                        if let Some(edit) = self.import_edit(uri, &format!("std/{}", m)) {
                            fields.push(("additionalTextEdits".into(), Json::Arr(vec![edit])));
                            fields.push(("labelDetails".into(), Json::obj(vec![("description", Json::str(format!("std/{}", m)))])));
                        }
                    }
                }
            }
        }
        if matches!(a.types.get(t), Ty::Str) {
            STR_METHODS.iter().for_each(|m| add(m, 2, "string".into(), None, true, &mut items));
        }
        if matches!(a.types.get(t), Ty::Array(_)) {
            ARR_METHODS.iter().for_each(|m| add(m, 2, shown.clone(), None, true, &mut items));
        }
        if matches!(a.types.get(t), Ty::Map(..)) {
            MAP_METHODS.iter().for_each(|m| add(m, 2, shown.clone(), None, true, &mut items));
        }
        for f in &a.funcs {
            if f.0.contains('.') || f.0.starts_with("new ") {
                continue;
            }
            let Some(params) = f.1.split_once('(').map(|(_, r)| r) else { continue };
            let first = params.split([',', ')']).next().unwrap_or("");
            if let Some((_, ty)) = first.split_once(':') {
                if ty.trim() == shown {
                    add(&f.0, 2, f.1.clone(), None, true, &mut items);
                }
            }
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
        let doc_off = tmp.offset_of_utf16(line, ch);
        let before = &text[..doc_off];
        let off = repair::shift(&a.inserts, doc_off);
        if let Some(typed) = import_prefix(before) {
            return Json::Arr(import_items(&uri_to_path(uri), &typed));
        }
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
                return Json::Arr(self.member_items(uri, a, t, statics));
            }
            return Json::Arr(vec![]);
        }
        match complete::context(&before[..prefix_end]) {
            complete::Ctx::Annotation => return Json::Arr(self.annotation_items(uri, a)),
            complete::Ctx::AnnotationArgs(name) => return Json::Arr(self.annotation_arg_items(uri, a, &name)),
            complete::Ctx::DefKind => return Json::Arr(Self::def_kind_items()),
            complete::Ctx::Type => return Json::Arr(self.type_items(uri, a)),
            complete::Ctx::General => {}
        }
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut push = |label: &str, kind: i32, detail: String, items: &mut Vec<Json>| {
            if seen.insert(label.to_string()) {
                let rank = match kind {
                    6 => 0,
                    3 if !builtins::is_builtin(label) => 1,
                    7 | 8 | 13 | 22 => 2,
                    3 => 3,
                    _ => 4,
                };
                let mut f = vec![
                    ("label", Json::str(label)),
                    ("kind", Json::num(kind as f64)),
                    ("detail", Json::str(detail)),
                    ("sortText", Json::str(format!("{}{}", rank, label))),
                ];
                if kind == 3 {
                    f.push(("insertText", Json::str(format!("{}($0)", label))));
                    f.push(("insertTextFormat", Json::num(2)));
                    f.push((
                        "command",
                        Json::obj(vec![("title", Json::str("")), ("command", Json::str("editor.action.triggerParameterHints"))]),
                    ));
                }
                items.push(Json::obj(f));
            }
        };
        let in_new = before[..prefix_end].trim_end().ends_with("new");
        for l in a.index.locals.iter().rev() {
            if l.decl.file == a.root_file && (l.decl.start as usize) < off && off <= l.scope.end as usize && !l.name.starts_with('<') {
                push(&l.name, 6, a.types.display(l.ty), &mut items);
            }
        }
        for g in &a.globals {
            if g.2 == a.root_module {
                push(&g.0, 6, a.types.display(g.1), &mut items);
            }
        }
        if !in_new {
            for f in &a.funcs {
                if !f.0.contains('.') && !f.0.starts_with('<') && !f.0.starts_with("new ") {
                    push(&f.0, 3, f.1.clone(), &mut items);
                }
            }
        }
        for t in &a.type_names {
            let kind = match a.types.get(t.1) {
                Ty::Interface(i) if !a.types.ifaces[*i as usize].variants.is_empty() => 13,
                Ty::Interface(_) => 8,
                Ty::Enum(_) => 13,
                Ty::Record(r) if a.types.records[*r as usize].is_class => 7,
                _ => 22,
            };
            if in_new && kind != 7 {
                continue;
            }
            let detail = if t.1 == T_ERROR { "generic type".to_string() } else { a.types.display(t.1) };
            push(&t.0, kind, detail, &mut items);
        }
        if in_new {
            return Json::Arr(items);
        }
        for b in builtins::BUILTINS {
            let hidden = builtins::home_module(b).map(|m| !self.imported(a, Some(m), None)).unwrap_or(false);
            if !b.starts_with("__") && !hidden {
                push(b, 3, builtins::signature(b).to_string(), &mut items);
                if let (Some(doc), Some(Json::Obj(fields))) = (crate::doc::builtins::find(b), items.last_mut()) {
                    if fields.iter().any(|(k, v)| k == "label" && v.as_str() == Some(b)) {
                        for (k, v) in fields.iter_mut() {
                            if k == "detail" {
                                *v = Json::str(doc.sig.clone());
                            }
                        }
                        fields.push((
                            "documentation".into(),
                            Json::obj(vec![("kind", Json::str("markdown")), ("value", Json::str(doc.markdown()))]),
                        ));
                    }
                }
            }
        }
        for k in KEYWORDS {
            push(k, 14, complete::keyword_doc(k).into(), &mut items);
        }
        for (t, doc) in complete::BUILTIN_TYPES {
            push(t, 22, format!("built-in type: {}", doc.replace('`', "")), &mut items);
        }
        let _ = T_ARR_ANY;
        let visible: std::collections::HashSet<String> = items.iter().filter_map(|i| i.get("label").as_str().map(|s| s.to_string())).collect();
        items.extend(self.auto_import_items(uri, a, &visible));
        Json::Arr(items)
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

#[cfg(target_os = "linux")]
fn cap_memory() {
    #[repr(C)]
    struct Rlimit {
        cur: u64,
        max: u64,
    }
    extern "C" {
        fn getrlimit(resource: i32, rlim: *mut Rlimit) -> i32;
        fn setrlimit(resource: i32, rlim: *const Rlimit) -> i32;
    }
    const RLIMIT_AS: i32 = 9;
    let mb: u64 = std::env::var("BURN_LSP_MEMORY_MB").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(4096);
    if mb == 0 {
        return;
    }
    let want = mb.saturating_mul(1 << 20);
    let mut cur = Rlimit { cur: 0, max: 0 };
    unsafe {
        if getrlimit(RLIMIT_AS, &mut cur) != 0 || cur.cur <= want {
            return;
        }
        let next = Rlimit { cur: want, max: cur.max };
        setrlimit(RLIMIT_AS, &next);
    }
}

#[cfg(not(target_os = "linux"))]
fn cap_memory() {}

pub fn run() -> ExitCode {
    cap_memory();
    burn_runtime::io::set_panic_mode(true);
    crate::repl::install_quiet_hook();
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut server = Server::default();
    let mut shutdown = false;
    while let Some(msg) = read_message(&mut reader) {
        let method = msg.get("method").as_str().unwrap_or("").to_string();
        if method.is_empty() {
            continue;
        }
        let id = msg.get("id").clone();
        let params = msg.get("params").clone();
        let uri = params.at(&["textDocument", "uri"]).as_str().unwrap_or("").to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match method.as_str() {
            "initialize" => {
                server.watch = reload::wants_watchers(&params);
                for f in params.get("workspaceFolders").as_arr() {
                    if let Some(u) = f.get("uri").as_str() {
                        server.roots.push(uri_to_path(u));
                    }
                }
                if server.roots.is_empty() {
                    if let Some(u) = params.get("rootUri").as_str() {
                        server.roots.push(uri_to_path(u));
                    }
                }
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
                                (
                                    "codeActionProvider",
                                    Json::obj(vec![(
                                        "codeActionKinds",
                                        Json::Arr(vec![Json::str("quickfix"), Json::str("source.fixAll.burn")]),
                                    )]),
                                ),
                                (
                                    "completionProvider",
                                    Json::obj(vec![(
                                        "triggerCharacters",
                                        Json::Arr(vec![Json::str("."), Json::str("\""), Json::str("/"), Json::str("@")]),
                                    )]),
                                ),
                                (
                                    "executeCommandProvider",
                                    Json::obj(vec![("commands", Json::Arr(vec![Json::str(reload::RELOAD)]))]),
                                ),
                                ("documentLinkProvider", Json::obj(vec![("resolveProvider", Json::Bool(false))])),
                                ("foldingRangeProvider", Json::Bool(true)),
                                ("typeDefinitionProvider", Json::Bool(true)),
                                ("implementationProvider", Json::Bool(true)),
                                ("referencesProvider", Json::Bool(true)),
                                ("documentHighlightProvider", Json::Bool(true)),
                                ("workspaceSymbolProvider", Json::Bool(true)),
                                ("renameProvider", Json::obj(vec![("prepareProvider", Json::Bool(true))])),
                                ("inlayHintProvider", Json::Bool(true)),
                                (
                                    "signatureHelpProvider",
                                    Json::obj(vec![
                                        ("triggerCharacters", Json::Arr(vec![Json::str("("), Json::str(",")])),
                                        ("retriggerCharacters", Json::Arr(vec![Json::str(",")])),
                                    ]),
                                ),
                            ]),
                        ),
                        (
                            "serverInfo",
                            Json::obj(vec![("name", Json::str("burn")), ("version", Json::str(env!("CARGO_PKG_VERSION")))]),
                        ),
                    ]),
                );
            }
            "initialized" => {
                if server.watch {
                    reload::register_watchers();
                }
            }
            "$/cancelRequest" | "workspace/didChangeConfiguration" => {}
            "workspace/didChangeWatchedFiles" => server.reanalyze_all(),
            "workspace/executeCommand" => {
                if params.get("command").as_str() == Some(reload::RELOAD) {
                    let r = server.reload_project(&params);
                    respond(&id, r);
                } else {
                    respond_err(&id, -32601, "unknown command");
                }
            }
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
            "textDocument/didSave" => {
                let mut open: Vec<String> = server.docs.keys().cloned().collect();
                open.retain(|u| *u != uri);
                open.insert(0, uri.clone());
                for u in open {
                    server.analyze(&u);
                }
            }
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
            "textDocument/documentSymbol" => respond(&id, server.document_symbols(&uri)),
            "textDocument/documentLink" => respond(&id, server.document_links(&uri)),
            "textDocument/foldingRange" => respond(&id, server.folding_ranges(&uri)),
            "textDocument/typeDefinition" => respond(&id, server.type_definition(&uri, &params)),
            "textDocument/implementation" => respond(&id, server.implementation(&uri, &params)),
            "textDocument/formatting" => respond(&id, server.formatting(&uri)),
            "textDocument/codeAction" => respond(&id, server.code_actions(&uri, &params)),
            "textDocument/references" => respond(&id, server.references(&uri, &params)),
            "textDocument/documentHighlight" => respond(&id, server.highlights(&uri, &params)),
            "textDocument/signatureHelp" => respond(&id, server.signature_help(&uri, &params)),
            "textDocument/inlayHint" => respond(&id, server.inlay_hints(&uri, &params)),
            "workspace/symbol" => respond(&id, server.workspace_symbols(&params)),
            "textDocument/prepareRename" => match server.prepare_rename(&uri, &params) {
                Ok(r) => respond(&id, r),
                Err(e) => respond_err(&id, -32803, &e),
            },
            "textDocument/rename" => match server.rename(&uri, &params) {
                Ok(r) => respond(&id, r),
                Err(e) => respond_err(&id, -32803, &e),
            },
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

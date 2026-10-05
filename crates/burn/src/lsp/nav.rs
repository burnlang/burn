use super::json::Json;
use super::{path_to_uri, range_json, sources, uri_to_path, word_at, Analysis, Server, KEYWORDS};
use crate::source::Span;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

type Key = (PathBuf, u32, u32);
type Refs = Vec<(String, Json, Span, bool)>;

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn location(uri: &str, range: Json) -> Json {
    Json::obj(vec![("uri", Json::str(uri)), ("range", range)])
}

fn point(path: &Path, line: usize, col: usize, len: usize) -> Json {
    let pos = |c: usize| Json::obj(vec![("line", Json::num(line as f64)), ("character", Json::num(c as f64))]);
    location(&path_to_uri(path), Json::obj(vec![("start", pos(col)), ("end", pos(col + len))]))
}

pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_') && !KEYWORDS.contains(&name)
}

impl Analysis {
    pub fn file_path(&self, span: Span) -> Option<PathBuf> {
        let f = self.sm.file(span.file);
        match &f.path {
            Some(p) => Some(canon(p)),
            None => sources::std_display(&f.name).and_then(sources::std_file),
        }
    }

    pub fn key(&self, span: Span) -> Option<Key> {
        Some((self.file_path(span)?, span.start, span.end))
    }

    pub fn uri_of(&self, span: Span, root_uri: &str) -> Option<String> {
        if span.file == self.root_file {
            return Some(root_uri.to_string());
        }
        self.file_path(span).map(|p| path_to_uri(&p))
    }

    pub fn target_at(&self, off: usize) -> Option<Span> {
        let inside = |s: &Span| s.file == self.root_file && (s.start as usize) <= off && off <= (s.end as usize);
        let mut best: Option<(Span, Span)> = None;
        for (u, d) in &self.index.defs {
            if inside(u) && best.map(|(b, _)| u.end - u.start < b.end - b.start).unwrap_or(true) {
                best = Some((*u, *d));
            }
        }
        if let Some((_, d)) = best {
            return Some(d);
        }
        let decls = self
            .index
            .defs
            .iter()
            .map(|d| d.1)
            .chain(self.funcs.iter().map(|f| f.2))
            .chain(self.type_names.iter().map(|t| t.2))
            .chain(self.index.locals.iter().map(|l| l.decl))
            .chain(self.types.records.iter().flat_map(|r| r.fields.iter().map(|f| f.span)))
            .chain(self.types.enums.iter().flat_map(|e| e.variants.iter().map(|v| v.1)))
            .chain(self.types.ifaces.iter().flat_map(|i| i.methods.iter().map(|m| m.span)));
        decls.filter(inside).min_by_key(|s| s.end - s.start)
    }

    pub fn related(&self, d: Span) -> Vec<Span> {
        let name = self.sm.file(d.file).text(d).to_string();
        let mut ifaces: Vec<usize> = Vec::new();
        for (ii, iface) in self.types.ifaces.iter().enumerate() {
            if iface.methods.iter().any(|m| m.span == d) {
                ifaces.push(ii);
            }
        }
        for f in &self.funcs {
            if f.2 != d {
                continue;
            }
            let Some((owner, _)) = f.0.rsplit_once('.') else { continue };
            if let Some(rec) = self.types.records.iter().find(|r| r.name == owner) {
                for ii in &rec.implements {
                    if self.types.ifaces[*ii as usize].methods.iter().any(|m| m.name == name) {
                        ifaces.push(*ii as usize);
                    }
                }
            }
        }
        let mut out = vec![d];
        for ii in ifaces {
            let iface = &self.types.ifaces[ii];
            out.extend(iface.methods.iter().filter(|m| m.name == name).map(|m| m.span));
            for rec in self.types.records.iter().filter(|r| r.implements.contains(&(ii as u32))) {
                let full = format!("{}.{}", rec.name, name);
                out.extend(self.funcs.iter().filter(|f| f.0 == full).map(|f| f.2));
            }
        }
        out.sort_by_key(|s| (s.file, s.start));
        out.dedup();
        out
    }

    fn uses_of(&self, targets: &[Key], root_uri: &str, out: &mut Refs) {
        for (u, d) in &self.index.defs {
            if self.key(*d).map(|k| targets.contains(&k)).unwrap_or(false) {
                if let Some(uri) = self.uri_of(*u, root_uri) {
                    out.push((uri, range_json(&self.sm, *u), *u, false));
                }
            }
        }
    }
}

impl Server {
    pub fn definition(&self, uri: &str, params: &Json) -> Json {
        let Some((off, a)) = self.offset(uri, params) else {
            return Json::Null;
        };
        if let Some(j) = self.import_definition(a, off) {
            return j;
        }
        if let Some(d) = a.target_at(off) {
            let text = a.sm.file(d.file).text(d).to_string();
            if text.ends_with(".bvmc\"") || text.ends_with(".bar\"") || text.ends_with(".bvm\"") {
                let name = word_at(&a.sm.file(a.root_file).src, off);
                let lib = text.trim_matches('"').rsplit('/').next().unwrap_or("library").to_string();
                let funcs: Vec<(String, String)> = a.funcs.iter().filter(|f| f.2 == d).map(|f| (f.0.clone(), f.1.clone())).collect();
                if let Some((path, line)) = sources::library_stub(&lib, &funcs, &name) {
                    return point(&path, line, 4, name.len());
                }
            }
            if let Some(target) = a.uri_of(d, uri) {
                return location(&target, range_json(&a.sm, d));
            }
        }
        let word = word_at(&a.sm.file(a.root_file).src, off);
        if crate::check::builtins::is_builtin(&word) {
            if let Some((path, line, col)) = sources::builtin_location(&word) {
                let name_len = crate::doc::builtins::find(&word).map(|b| b.name.len()).unwrap_or(word.len());
                return point(&path, line, col, name_len);
            }
        }
        Json::Null
    }

    pub(super) fn project_files(&self, current: &Path) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = Vec::new();
        if let Some(r) = crate::project::find_root(current) {
            roots.push(r);
        }
        for r in &self.roots {
            if current.starts_with(r) && !roots.iter().any(|x| x.starts_with(r) || r.starts_with(x)) {
                roots.push(r.clone());
            }
        }
        if roots.is_empty() {
            if let Some(p) = current.parent() {
                roots.push(p.to_path_buf());
            }
        }
        let mut out = Vec::new();
        for r in roots {
            collect(&r, 0, &mut out);
        }
        out.sort();
        out.dedup();
        out
    }

    fn references_of(&self, uri: &str, params: &Json, include_decl: bool, everywhere: bool) -> Option<(Refs, String)> {
        let (off, a) = self.offset(uri, params)?;
        let d = a.target_at(off)?;
        let key = a.key(d)?;
        let name = a.sm.file(d.file).text(d).to_string();
        let group = a.related(d);
        let keys: Vec<Key> = group.iter().filter_map(|s| a.key(*s)).collect();
        let mut out = Vec::new();
        if include_decl {
            for s in &group {
                if let Some(u) = a.uri_of(*s, uri) {
                    out.push((u, range_json(&a.sm, *s), *s, true));
                }
            }
        }
        a.uses_of(&keys, uri, &mut out);
        if everywhere {
            let here = canon(&uri_to_path(uri));
            for f in self.project_files(&here) {
                if canon(&f) == here || sources::is_cached(&f) {
                    continue;
                }
                if let Some(other) = self.quick_analysis(&f) {
                    let other_uri = self.uri_for(&f);
                    other.uses_of(&keys, &other_uri, &mut out);
                }
            }
        }
        let mut seen = HashSet::new();
        out.retain(|(u, r, _, _)| seen.insert((u.clone(), r.encode())));
        let _ = key;
        Some((out, name))
    }

    fn uri_for(&self, path: &Path) -> String {
        let c = canon(path);
        self.docs
            .keys()
            .find(|u| canon(&uri_to_path(u)) == c)
            .cloned()
            .unwrap_or_else(|| path_to_uri(&c))
    }

    pub fn references(&self, uri: &str, params: &Json) -> Json {
        let include = params.at(&["context", "includeDeclaration"]).as_bool().unwrap_or(true);
        match self.references_of(uri, params, include, true) {
            Some((refs, _)) => Json::Arr(refs.into_iter().map(|(u, r, _, _)| location(&u, r)).collect()),
            None => Json::Arr(vec![]),
        }
    }

    pub fn highlights(&self, uri: &str, params: &Json) -> Json {
        match self.references_of(uri, params, true, false) {
            Some((refs, _)) => Json::Arr(
                refs.into_iter()
                    .filter(|(u, _, _, _)| u == uri)
                    .map(|(_, r, _, decl)| Json::obj(vec![("range", r), ("kind", Json::num(if decl { 3 } else { 2 }))]))
                    .collect(),
            ),
            None => Json::Arr(vec![]),
        }
    }

    pub fn prepare_rename(&self, uri: &str, params: &Json) -> Result<Json, String> {
        let (off, a) = self.offset(uri, params).ok_or("nothing to rename here")?;
        let Some(d) = a.target_at(off) else {
            let word = word_at(&a.sm.file(a.root_file).src, off);
            if crate::check::builtins::is_builtin(&word) {
                return Err(format!("`{}` is built into Burn and cannot be renamed", word));
            }
            return Err("nothing to rename here".into());
        };
        let path = a.file_path(d).ok_or("this name cannot be renamed")?;
        if sources::is_cached(&path) {
            return Err("names from the standard library and built-ins cannot be renamed".into());
        }
        let name = a.sm.file(d.file).text(d).to_string();
        if !valid_name(&name) {
            return Err("this is not a name that can be renamed".into());
        }
        let (u, _) = a
            .index
            .defs
            .iter()
            .filter(|(u, _)| u.file == a.root_file && (u.start as usize) <= off && off <= (u.end as usize))
            .min_by_key(|(u, _)| u.end - u.start)
            .copied()
            .unwrap_or((d, d));
        let span = if u.file == a.root_file { u } else { d };
        Ok(Json::obj(vec![("range", range_json(&a.sm, span)), ("placeholder", Json::str(name))]))
    }

    pub fn rename(&self, uri: &str, params: &Json) -> Result<Json, String> {
        self.prepare_rename(uri, params)?;
        let new_name = params.get("newName").as_str().unwrap_or("").to_string();
        if !valid_name(&new_name) {
            return Err(format!("`{}` is not a valid name", new_name));
        }
        let (refs, old) = self.references_of(uri, params, true, true).ok_or("nothing to rename here")?;
        let mut changes: HashMap<String, Vec<Json>> = HashMap::new();
        for (u, r, _, _) in refs {
            changes
                .entry(u)
                .or_default()
                .push(Json::obj(vec![("range", r), ("newText", Json::str(new_name.clone()))]));
        }
        let _ = old;
        Ok(Json::obj(vec![(
            "changes",
            Json::Obj(changes.into_iter().map(|(u, e)| (u, Json::Arr(e))).collect()),
        )]))
    }

    pub fn workspace_symbols(&self, params: &Json) -> Json {
        let query = params.get("query").as_str().unwrap_or("").to_lowercase();
        let mut files: Vec<PathBuf> = Vec::new();
        for r in &self.roots {
            collect(r, 0, &mut files);
        }
        for u in self.docs.keys() {
            let p = uri_to_path(u);
            for f in self.project_files(&p) {
                files.push(f);
            }
        }
        files.sort();
        files.dedup();
        let mut out = Vec::new();
        for f in files.into_iter().take(500) {
            let text = match self.docs.iter().find(|(u, _)| canon(&uri_to_path(u)) == canon(&f)) {
                Some((_, t)) => t.clone(),
                None => match std::fs::read_to_string(&f) {
                    Ok(t) => t,
                    Err(_) => continue,
                },
            };
            let uri = self.uri_for(&f);
            let file = crate::source::SourceFile::new(String::new(), None, text.clone());
            for (name, kind, start, end) in top_level(&text) {
                if !query.is_empty() && !name.to_lowercase().contains(&query) {
                    continue;
                }
                let pos = |o: usize| {
                    let (l, c) = file.line_utf16_col(o);
                    Json::obj(vec![("line", Json::num(l as f64)), ("character", Json::num(c as f64))])
                };
                out.push(Json::obj(vec![
                    ("name", Json::str(name)),
                    ("kind", Json::num(kind)),
                    ("location", location(&uri, Json::obj(vec![("start", pos(start)), ("end", pos(end))]))),
                ]));
            }
        }
        Json::Arr(out)
    }
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 8 || out.len() >= 500 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.starts_with('.') || matches!(name.as_str(), "build" | "target" | "node_modules" | "out" | "dist") {
            continue;
        }
        if p.is_dir() {
            collect(&p, depth + 1, out);
        } else if name.ends_with(".bn") {
            out.push(canon(&p));
        }
    }
}

fn top_level(text: &str) -> Vec<(String, i32, usize, usize)> {
    use crate::lexer::Tok;
    let (tokens, _) = crate::lexer::lex(text, 0);
    let ident = |i: usize| match tokens.get(i).map(|t| &t.kind) {
        Some(Tok::Ident(n)) => Some(n.as_str()),
        _ => None,
    };
    let mut out = Vec::new();
    let mut depth = 0i32;
    for i in 0..tokens.len() {
        match &tokens[i].kind {
            Tok::LBrace => depth += 1,
            Tok::RBrace => depth -= 1,
            Tok::Fun if depth <= 1 => {
                if let Some(name) = ident(i + 1) {
                    let t = &tokens[i + 1];
                    out.push((name.to_string(), if depth == 0 { 12 } else { 6 }, t.span.start as usize, t.span.end as usize));
                }
            }
            Tok::Def if depth == 0 => {
                let mut k = i + 1;
                while matches!(ident(k), Some("abstract" | "static" | "pub" | "priv")) {
                    k += 1;
                }
                let kind = match ident(k) {
                    Some("struct") => 5,
                    Some("interface") => 11,
                    Some("enum") => 10,
                    Some("type" | "annotation") => 23,
                    _ => continue,
                };
                if let Some(name) = ident(k + 1) {
                    let t = &tokens[k + 1];
                    out.push((name.to_string(), kind, t.span.start as usize, t.span.end as usize));
                }
            }
            _ => {}
        }
    }
    out
}

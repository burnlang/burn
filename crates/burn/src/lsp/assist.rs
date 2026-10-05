use super::json::Json;
use super::{Analysis, Server};
use crate::types::{Ty, T_ERROR};

fn skip_string(b: &[u8], mut i: usize) -> usize {
    let quote = b[i];
    i += 1;
    while i < b.len() && b[i] != quote {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == b'$' && b.get(i + 1) == Some(&b'{') {
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                match b[i] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    b'"' | b'\'' => {
                        i = skip_string(b, i);
                        continue;
                    }
                    _ => {}
                }
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    i + 1
}

fn open_call(text: &str) -> Option<(usize, usize)> {
    let b = text.as_bytes();
    let mut stack: Vec<(Option<usize>, usize)> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if c == b'"' || c == b'\'' {
            i = skip_string(b, i);
            continue;
        }
        match c {
            b'(' => stack.push((Some(i), 0)),
            b'[' | b'{' => stack.push((None, 0)),
            b')' | b']' | b'}' => {
                stack.pop();
            }
            b',' => {
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    stack.into_iter().rev().find_map(|(p, n)| p.map(|p| (p, n)))
}

fn callee(text: &str, paren: usize) -> (Vec<String>, bool) {
    let before = text[..paren].trim_end();
    let bytes = before.as_bytes();
    let mut s = bytes.len();
    while s > 0 && (bytes[s - 1].is_ascii_alphanumeric() || bytes[s - 1] == b'_' || bytes[s - 1] == b'.') {
        s -= 1;
    }
    let chain: Vec<String> = before[s..].split('.').filter(|p| !p.is_empty()).map(|p| p.to_string()).collect();
    let is_new = before[..s].trim_end().ends_with("new");
    (chain, is_new)
}

fn split_params(sig: &str) -> Option<(usize, Vec<(usize, usize)>)> {
    let open = sig.find('(')?;
    let b = sig.as_bytes();
    let mut depth = 0;
    let mut start = open + 1;
    let mut out = Vec::new();
    for (i, c) in b.iter().enumerate().skip(open) {
        match c {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => {
                depth -= 1;
                if depth == 0 {
                    let part = sig[start..i].trim();
                    if !part.is_empty() {
                        let lead = sig[start..i].len() - sig[start..i].trim_start().len();
                        out.push((start + lead, start + lead + part.len()));
                    }
                    return Some((open, out));
                }
            }
            b',' if depth == 1 => {
                let part = sig[start..i].trim();
                let lead = sig[start..i].len() - sig[start..i].trim_start().len();
                out.push((start + lead, start + lead + part.len()));
                start = i + 1;
            }
            _ => {}
        }
    }
    None
}

impl Server {
    fn signature_of(&self, a: &Analysis, chain: &[String], is_new: bool, off: usize) -> Option<(String, String, usize)> {
        let name = chain.last()?;
        if chain.len() == 1 {
            let fs: Vec<&(String, String, crate::source::Span, usize)> = a.funcs.iter().filter(|f| f.0 == *name).collect();
            if let Some(f) = fs.iter().find(|f| f.3 == a.root_module).or(fs.first()) {
                return Some((f.1.clone(), String::new(), 0));
            }
            let ctor = format!("{}.<init>", name);
            let new_name = format!("new {}", name);
            if let Some(f) = a.funcs.iter().find(|f| f.0 == ctor || f.0 == new_name || (is_new && f.0 == *name)) {
                return Some((f.1.replacen("<init>", name, 1), String::new(), 0));
            }
            if let Some(b) = crate::doc::builtins::find(name) {
                return Some((b.sig.clone(), crate::doc::comment::to_markdown(&b.doc), 0));
            }
            return None;
        }
        let (t, _) = self.resolve_chain(a, &chain[..chain.len() - 1], off)?;
        match a.types.get(t) {
            Ty::Record(r) => {
                let rec = &a.types.records[*r as usize];
                let mut cur = Some(*r);
                while let Some(ri) = cur {
                    let owner = &a.types.records[ri as usize];
                    let full = format!("{}.{}", owner.name, name);
                    if let Some(f) = a.funcs.iter().find(|f| f.0 == full) {
                        return Some((f.1.clone(), String::new(), 0));
                    }
                    cur = owner.parent;
                }
                let _ = rec;
                None
            }
            Ty::Interface(i) => {
                let m = a.types.ifaces[*i as usize].methods.iter().find(|m| m.name == *name)?;
                let ps: Vec<String> = m.params.iter().map(|p| a.types.display(*p)).collect();
                Some((format!("fun {}({}): {}", name, ps.join(", "), a.types.display(m.ret)), String::new(), 0))
            }
            _ => {
                let b = crate::doc::builtins::find(name)?;
                Some((b.sig.clone(), crate::doc::comment::to_markdown(&b.doc), 1))
            }
        }
    }

    pub fn signature_help(&self, uri: &str, params: &Json) -> Json {
        let (Some(text), Some(a)) = (self.docs.get(uri), self.analyses.get(uri)) else {
            return Json::Null;
        };
        let line = params.at(&["position", "line"]).as_f64().unwrap_or(0.0) as usize;
        let ch = params.at(&["position", "character"]).as_f64().unwrap_or(0.0) as usize;
        let doc_off = crate::source::SourceFile::new(String::new(), None, text.clone())
            .offset_of_utf16(line, ch)
            .min(text.len());
        let off = super::repair::shift(&a.inserts, doc_off);
        let Some((paren, commas)) = open_call(&text[..doc_off]) else {
            return Json::Null;
        };
        let (chain, is_new) = callee(text, paren);
        if chain.is_empty() {
            return Json::Null;
        }
        let Some((sig, doc, skip)) = self.signature_of(a, &chain, is_new, off) else {
            return Json::Null;
        };
        let Some((_, ranges)) = split_params(&sig) else {
            return Json::Null;
        };
        let utf16 = |i: usize| sig[..i].encode_utf16().count() as f64;
        let params: Vec<Json> = ranges
            .iter()
            .skip(skip)
            .map(|(s, e)| Json::obj(vec![("label", Json::Arr(vec![Json::num(utf16(*s)), Json::num(utf16(*e))]))]))
            .collect();
        let n = params.len();
        let variadic = sig.contains("...");
        let active = if n == 0 {
            0
        } else if variadic {
            commas.min(n - 1)
        } else {
            commas
        };
        let mut info = vec![("label", Json::str(sig.clone())), ("parameters", Json::Arr(params))];
        if !doc.is_empty() {
            info.push(("documentation", Json::obj(vec![("kind", Json::str("markdown")), ("value", Json::str(doc))])));
        }
        Json::obj(vec![
            ("signatures", Json::Arr(vec![Json::obj(info)])),
            ("activeSignature", Json::num(0)),
            ("activeParameter", Json::num(active as f64)),
        ])
    }

    pub fn inlay_hints(&self, uri: &str, params: &Json) -> Json {
        let Some(a) = self.analyses.get(uri) else {
            return Json::Arr(vec![]);
        };
        let f = a.sm.file(a.root_file);
        let line_of = |k: &str| params.at(&["range", k, "line"]).as_f64().unwrap_or(0.0) as usize;
        let (from, to) = (line_of("start"), line_of("end"));
        let src = &f.src;
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for l in &a.index.locals {
            if l.decl.file != a.root_file || l.ty == T_ERROR || l.name.starts_with('_') || l.name.starts_with('<') {
                continue;
            }
            let (start, end) = (l.decl.start as usize, l.decl.end as usize);
            if end > src.len() || src[start..end] != l.name || !seen.insert(start) {
                continue;
            }
            let after = src[end..].trim_start();
            if after.starts_with(':') {
                continue;
            }
            let before = src[..start].trim_end();
            let word: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let inferred = matches!(word.as_str(), "var" | "const" | "for")
                || (before.ends_with(',') && before.rsplit('\n').next().unwrap_or("").trim_start().starts_with("for"));
            if !inferred {
                continue;
            }
            let (line, col) = f.line_utf16_col(end);
            if line < from || line > to {
                continue;
            }
            out.push(Json::obj(vec![
                (
                    "position",
                    Json::obj(vec![("line", Json::num(line as f64)), ("character", Json::num(col as f64))]),
                ),
                ("label", Json::str(format!(": {}", a.types.display(l.ty)))),
                ("kind", Json::num(1)),
                ("paddingLeft", Json::Bool(false)),
            ]));
        }
        Json::Arr(out)
    }
}

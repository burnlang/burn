use super::ide::Target;
use super::json::Json;
use super::{Analysis, Server};
use crate::lexer::{self, Tok};
use crate::loader::STDLIB;
use crate::types::{Ty, T_ERROR};

#[derive(Debug, PartialEq)]
pub enum Ctx {
    General,
    Type,
    Annotation,
    AnnotationArgs(String),
    DefKind,
}

pub const BUILTIN_TYPES: &[(&str, &str)] = &[
    ("int", "64-bit signed integer"),
    ("float", "64-bit floating point number"),
    ("string", "UTF-8 text"),
    ("bool", "`true` or `false`"),
    ("any", "a value of any type, narrowed with `is` or `as`"),
    ("void", "no value, for functions that return nothing"),
    ("int8", "8-bit signed integer"),
    ("int16", "16-bit signed integer"),
    ("int32", "32-bit signed integer"),
    ("int64", "64-bit signed integer, the same as `int`"),
    ("uint8", "8-bit unsigned integer, also called `byte`"),
    ("uint16", "16-bit unsigned integer"),
    ("uint32", "32-bit unsigned integer"),
    ("uint64", "64-bit unsigned integer"),
    ("float32", "32-bit floating point number"),
    ("byte", "8-bit unsigned integer, the same as `uint8`"),
];

type Fields = &'static [(&'static str, &'static str)];

const BUILTIN_ANNOTATIONS: &[(&str, &str, Fields)] = &[
    ("Getter", "Generates `getName()`, or `isName()` for `bool` fields.", &[]),
    ("Setter", "Generates `setName(value)`.", &[]),
    ("Deprecated", "Every use produces a warning with the message.", &[("message", "string")]),
    ("Export", "Lets bytecode libraries call this top-level function.", &[("name", "string")]),
    (
        "Native",
        "Declares a function without a body that the host program provides.",
        &[("name", "string")],
    ),
    (
        "Inject",
        "A mixin that runs this function when `target` starts (`at: \"head\"`) or returns (`at: \"return\"`).",
        &[("target", "string"), ("at", "string"), ("cancellable", "bool"), ("priority", "int")],
    ),
    (
        "Overwrite",
        "A mixin that replaces the body of `target`.",
        &[("target", "string"), ("priority", "int")],
    ),
    (
        "Redirect",
        "A mixin that replaces every call to `call` inside `target` with a call to this function.",
        &[
            ("target", "string"),
            ("call", "string"),
            ("rt", "string"),
            ("host", "string"),
            ("priority", "int"),
        ],
    ),
];

const KEYWORD_DOCS: &[(&str, &str)] = &[
    ("fun", "declares a function"),
    ("var", "declares a variable"),
    ("const", "declares a constant"),
    (
        "def",
        "starts a definition: `def struct`, `def interface`, `def enum`, `def type`, `def annotation`",
    ),
    ("if", "runs a block when a condition holds"),
    ("else", "the branch taken when the `if` condition does not hold"),
    ("while", "repeats a block while a condition holds"),
    ("for", "loops over a range, array, map or string"),
    ("in", "separates the loop variables from what `for` loops over"),
    ("match", "chooses a branch by comparing a value with patterns"),
    ("return", "leaves the function, optionally with a value"),
    ("break", "leaves the innermost loop"),
    ("continue", "skips to the next iteration of the innermost loop"),
    ("import", "imports a module: `import \"std/date\"`"),
    ("pub", "makes a declaration visible to importing modules"),
    ("priv", "keeps a declaration private to its module or struct"),
    ("async", "declares a function that runs as a task"),
    ("await", "waits for a task and gives its result"),
    ("is", "tests the type of a value and narrows it"),
    ("as", "converts or casts a value to a type; `as?` gives null on failure"),
    ("new", "creates an instance of a struct"),
    ("destroy", "destroys a struct instance and runs its destructor"),
    ("static", "a member that belongs to the struct, not to an instance"),
    ("abstract", "a struct that cannot be created, only extended"),
    ("self", "the current instance inside a method"),
    ("true", "the boolean true"),
    ("false", "the boolean false"),
    ("null", "no value, for optional types"),
    ("struct", "after `def`: a struct with fields and methods"),
    ("interface", "after `def`: a set of methods that structs implement"),
    ("enum", "after `def`: a type with a fixed set of values"),
    ("type", "after `def`: a plain data type"),
    ("annotation", "after `def`: an annotation used as `@Name(...)`"),
];

pub fn keyword_doc(k: &str) -> &'static str {
    KEYWORD_DOCS.iter().find(|(n, _)| *n == k).map(|(_, d)| *d).unwrap_or("keyword")
}

fn open_bracket(text: &str) -> Option<(usize, u8)> {
    let b = text.as_bytes();
    let mut stack: Vec<(usize, u8)> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'"' | b'\'' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'(' | b'[' | b'{' | b'<' => stack.push((i, b[i])),
            b')' | b']' | b'}' => {
                while let Some((_, c)) = stack.pop() {
                    if c != b'<' {
                        break;
                    }
                }
            }
            b'>' => {
                if matches!(stack.last(), Some((_, b'<'))) {
                    stack.pop();
                }
            }
            b'\n' | b';' | b'=' => {
                while matches!(stack.last(), Some((_, b'<'))) {
                    stack.pop();
                }
            }
            _ => {}
        }
        i += 1;
    }
    stack.last().copied()
}

fn last_word(text: &str) -> &str {
    let t = text.trim_end();
    let start = t.rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map(|i| i + 1).unwrap_or(0);
    &t[start..]
}

fn is_declaration_paren(before: &str) -> bool {
    let t = before.trim_end();
    let line = t.rsplit('\n').next().unwrap_or("").trim_start();
    let word = last_word(t);
    word == "fun"
        || line.starts_with("fun ")
        || line.contains(" fun ")
        || line.starts_with("async fun ")
        || line.starts_with("def ")
        || line.starts_with("pub def ")
        || line.starts_with("priv def ")
        || line.starts_with("pub fun ")
        || line.starts_with("priv fun ")
        || line.starts_with("static fun ")
        || line.starts_with('@') && line.contains(" def ")
}

fn is_literal_brace(before: &str) -> bool {
    let t = before.trim_end();
    matches!(t.chars().last(), Some('=' | '(' | ',' | ':' | '[') | None) || last_word(t) == "return"
}

fn type_context(head: &str) -> bool {
    let t = head.trim_end();
    if t.ends_with("as?") {
        return true;
    }
    match last_word(t) {
        "is" | "as" if t.len() > 2 => return true,
        _ => {}
    }
    if let Some(rest) = t.strip_suffix(':') {
        let rest = rest.strip_suffix(':').unwrap_or(rest);
        if rest.trim_end().ends_with(')') {
            return true;
        }
        return match open_bracket(rest) {
            Some((i, b'(')) => is_declaration_paren(&rest[..i]),
            Some((i, b'{')) => !is_literal_brace(&rest[..i]),
            Some((_, b'<')) => true,
            Some(_) => false,
            None => true,
        };
    }
    if let Some(rest) = t.strip_suffix('[').or_else(|| t.strip_suffix('?')) {
        return type_context(rest);
    }
    if let Some(rest) = t.strip_suffix('<') {
        return !rest.trim_end().is_empty() && last_word(rest).chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
    }
    if let Some(rest) = t.strip_suffix(',') {
        if let Some((i, b'<')) = open_bracket(rest) {
            return type_context(&rest[..=i]);
        }
    }
    false
}

pub fn context(head: &str) -> Ctx {
    if head.ends_with('@') {
        return Ctx::Annotation;
    }
    if let Some((i, b'(')) = open_bracket(head) {
        let before = &head[..i];
        let name = last_word(before);
        if !name.is_empty() && before[..before.len() - name.len()].ends_with('@') {
            let t = head.trim_end();
            if t.ends_with('(') || t.ends_with(',') {
                return Ctx::AnnotationArgs(name.to_string());
            }
        }
    }
    let line = head.rsplit('\n').next().unwrap_or("").trim_start();
    let mut words = line.split_whitespace().filter(|w| !w.starts_with('@'));
    let mut first = words.next();
    while matches!(first, Some("pub" | "priv")) {
        first = words.next();
    }
    if first == Some("def") && words.all(|w| matches!(w, "abstract" | "static")) && head.ends_with(char::is_whitespace) {
        return Ctx::DefKind;
    }
    if type_context(head) {
        return Ctx::Type;
    }
    Ctx::General
}

fn item(label: &str, kind: i32, detail: &str, doc: Option<&str>, sort: &str) -> Json {
    let mut f = vec![
        ("label", Json::str(label)),
        ("kind", Json::num(kind as f64)),
        ("detail", Json::str(detail)),
        ("sortText", Json::str(format!("{}{}", sort, label))),
    ];
    if let Some(d) = doc {
        f.push(("documentation", Json::obj(vec![("kind", Json::str("markdown")), ("value", Json::str(d))])));
    }
    Json::obj(f)
}

fn declared_annotations(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let (tokens, _) = lexer::lex(text, 0);
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if t.kind != Tok::Def {
            continue;
        }
        let is_annotation = matches!(tokens.get(i + 1).map(|t| &t.kind), Some(Tok::Ident(w)) if w == "annotation");
        let Some(Tok::Ident(name)) = tokens.get(i + 2).map(|t| &t.kind) else {
            continue;
        };
        if !is_annotation {
            continue;
        }
        let mut fields = Vec::new();
        if let Some(open) = tokens.get(i + 3).filter(|t| t.kind == Tok::LBrace) {
            let start = open.span.end as usize;
            let end = text[start..].find('}').map(|e| start + e).unwrap_or(text.len());
            for line in text[start..end].lines() {
                let decl = line.split('=').next().unwrap_or("").split("//").next().unwrap_or("").trim();
                if let Some((ty, field)) = decl.rsplit_once(char::is_whitespace) {
                    fields.push((field.trim().to_string(), ty.trim().to_string()));
                }
            }
        }
        out.push((name.clone(), fields));
    }
    out
}

impl Server {
    fn annotation_sources(&self, uri: &str, a: &Analysis) -> Vec<String> {
        let mut texts: Vec<String> = self.docs.get(uri).cloned().into_iter().collect();
        for (_, target) in &a.links {
            match target {
                Target::Std(name) => texts.extend(STDLIB.iter().find(|s| s.name == *name).map(|s| s.src.to_string())),
                Target::File(p) => texts.extend(std::fs::read_to_string(p).ok()),
                Target::Lib(_) => {}
            }
        }
        texts
    }

    pub fn annotation_items(&self, uri: &str, a: &Analysis) -> Vec<Json> {
        let mut out: Vec<Json> = BUILTIN_ANNOTATIONS
            .iter()
            .map(|(n, doc, fields)| {
                let params: Vec<String> = fields.iter().map(|(f, t)| format!("{}: {}", f, t)).collect();
                let detail = if params.is_empty() {
                    format!("@{}", n)
                } else {
                    format!("@{}({})", n, params.join(", "))
                };
                item(n, 7, &detail, Some(doc), "1")
            })
            .collect();
        for text in self.annotation_sources(uri, a) {
            for (name, fields) in declared_annotations(&text) {
                let params: Vec<String> = fields.iter().map(|(f, t)| format!("{} {}", t, f)).collect();
                out.push(item(&name, 7, &format!("def annotation {} {{ {} }}", name, params.join(", ")), None, "0"));
            }
        }
        out
    }

    pub fn annotation_arg_items(&self, uri: &str, a: &Analysis, name: &str) -> Vec<Json> {
        let mut fields: Vec<(String, String)> = BUILTIN_ANNOTATIONS
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, _, f)| f.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect())
            .unwrap_or_default();
        if fields.is_empty() {
            for text in self.annotation_sources(uri, a) {
                if let Some((_, f)) = declared_annotations(&text).into_iter().find(|(n, _)| n == name) {
                    fields = f;
                    break;
                }
            }
        }
        fields
            .iter()
            .map(|(f, t)| {
                let mut j = item(f, 5, t, None, "0");
                if let Json::Obj(fs) = &mut j {
                    fs.push(("insertText".into(), Json::str(format!("{}: ", f))));
                }
                j
            })
            .collect()
    }

    pub fn def_kind_items() -> Vec<Json> {
        ["struct", "interface", "enum", "type", "annotation", "abstract", "static"]
            .iter()
            .map(|k| item(k, 14, keyword_doc(k), None, "0"))
            .collect()
    }

    pub fn type_items(&self, uri: &str, a: &Analysis) -> Vec<Json> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for t in &a.type_names {
            if !seen.insert(t.0.clone()) {
                continue;
            }
            let (kind, detail) = match a.types.get(t.1) {
                Ty::Interface(i) if !a.types.ifaces[*i as usize].variants.is_empty() => (13, format!("enum {}", t.0)),
                Ty::Interface(_) => (8, format!("interface {}", t.0)),
                Ty::Enum(_) => (13, format!("enum {}", t.0)),
                Ty::Record(r) if a.types.records[*r as usize].is_class => (7, format!("struct {}", t.0)),
                _ if t.1 == T_ERROR => (25, "type parameter".to_string()),
                _ => (22, format!("type {}", t.0)),
            };
            out.push(item(&t.0, kind, &detail, None, "0"));
        }
        for (n, doc) in BUILTIN_TYPES {
            if seen.insert(n.to_string()) {
                out.push(item(n, 22, "built-in type", Some(doc), "1"));
            }
        }
        let visible: std::collections::HashSet<String> = seen;
        for j in self.auto_import_items(uri, a, &visible) {
            let kind = j.get("kind").as_f64().unwrap_or(0.0) as i32;
            if matches!(kind, 7 | 8 | 13 | 22) {
                out.push(j);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_type_positions() {
        assert_eq!(context("var n: "), Ctx::Type);
        assert_eq!(context("fun f(a: int, b: "), Ctx::Type);
        assert_eq!(context("fun f(a: int): "), Ctx::Type);
        assert_eq!(context("def struct P(x: "), Ctx::Type);
        assert_eq!(context("    if v is "), Ctx::Type);
        assert_eq!(context("    var s = v as "), Ctx::Type);
        assert_eq!(context("var xs: ["), Ctx::Type);
        assert_eq!(context("var b: Box<"), Ctx::Type);
        assert_eq!(context("def struct C(r: float) :: "), Ctx::Type);
        assert_eq!(context("fun main() {\n    var n: "), Ctx::Type);
    }

    #[test]
    fn values_are_not_type_positions() {
        assert_eq!(context("    route(path: "), Ctx::General);
        assert_eq!(context("var m = {\"a\": "), Ctx::General);
        assert_eq!(context("    var x = "), Ctx::General);
        assert_eq!(context("    if a < "), Ctx::General);
    }

    #[test]
    fn detects_annotations_and_definitions() {
        assert_eq!(context("@"), Ctx::Annotation);
        assert_eq!(context("    @"), Ctx::Annotation);
        assert_eq!(context("@Inject("), Ctx::AnnotationArgs("Inject".into()));
        assert_eq!(context("@Route(\"/x\", "), Ctx::AnnotationArgs("Route".into()));
        assert_eq!(context("def "), Ctx::DefKind);
        assert_eq!(context("pub def abstract "), Ctx::DefKind);
    }

    #[test]
    fn reads_declared_annotations() {
        let a = declared_annotations("def annotation Route {\n    string path\n    string method = \"GET\"\n}\ndef annotation Internal\n");
        assert_eq!(a[0].0, "Route");
        assert_eq!(
            a[0].1,
            vec![("path".to_string(), "string".to_string()), ("method".to_string(), "string".to_string())]
        );
        assert_eq!(a[1].0, "Internal");
    }
}

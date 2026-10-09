use super::builtins::Builtin;
use super::comment::{Block, DocComment};
use super::model::{DocModule, Item, Kind, Member, MemberKind, Sig};
use crate::ast::{StructKind, TypeExpr, TypeExprKind};
use std::collections::HashMap;
use std::fmt::Write;

pub struct Site {
    pub title: String,
    pub modules: Vec<DocModule>,
    pub builtins: Vec<Builtin>,
    pub private: bool,
}

pub struct Page {
    pub path: String,
    pub content: String,
}

type KindGroup = (&'static str, fn(&Kind) -> bool);
type MemberGroup = (&'static str, fn(&MemberKind) -> bool);

const PRIMITIVES: &[&str] = &["int", "float", "string", "bool", "void", "any", "null", "Future"];

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '.' { c } else { '-' }).collect()
}

pub fn module_url(m: &str) -> String {
    format!("m-{}.html", slug(m))
}

pub fn type_url(m: &str, t: &str) -> String {
    format!("t-{}.{}.html", slug(m), t)
}

fn kind_label(k: &Kind) -> &'static str {
    match k {
        Kind::Function => "function",
        Kind::Struct(StructKind::Normal) => "struct",
        Kind::Struct(StructKind::Abstract) => "abstract struct",
        Kind::Struct(StructKind::Static) => "static struct",
        Kind::Interface => "interface",
        Kind::Enum => "enum",
        Kind::Type => "type",
        Kind::Alias => "type alias",
        Kind::Annotation => "annotation",
        Kind::Value { is_const: true } => "constant",
        Kind::Value { is_const: false } => "variable",
    }
}

fn has_page(k: &Kind) -> bool {
    matches!(k, Kind::Struct(_) | Kind::Interface | Kind::Enum | Kind::Type | Kind::Annotation)
}

struct Linker {
    types: HashMap<String, Vec<(String, String)>>,
    funcs: HashMap<String, Vec<(String, String)>>,
    builtins: HashMap<String, String>,
}

impl Linker {
    fn new(site: &Site) -> Linker {
        let mut l = Linker {
            types: HashMap::new(),
            funcs: HashMap::new(),
            builtins: HashMap::new(),
        };
        for m in &site.modules {
            for it in visible(&m.items, site.private) {
                if has_page(&it.kind) {
                    l.types.entry(it.name.clone()).or_default().push((m.name.clone(), type_url(&m.name, &it.name)));
                } else {
                    l.funcs
                        .entry(it.name.clone())
                        .or_default()
                        .push((m.name.clone(), format!("{}#{}", module_url(&m.name), slug(&it.name))));
                }
            }
        }
        for b in &site.builtins {
            let u = format!("builtins.html#{}", b.name);
            l.builtins.insert(b.name.clone(), u.clone());
            for a in &b.doc.aliases {
                l.builtins.insert(a.clone(), u.clone());
            }
        }
        l
    }

    fn pick(list: Option<&Vec<(String, String)>>, module: &str) -> Option<String> {
        let list = list?;
        list.iter().find(|(m, _)| m == module).or_else(|| list.first()).map(|(_, u)| u.clone())
    }

    fn ty(&self, name: &str, module: &str) -> Option<String> {
        Self::pick(self.types.get(name), module)
    }

    fn target(&self, text: &str, module: &str) -> Option<String> {
        let t = text.trim().trim_end_matches("()");
        if let Some((a, b)) = t.split_once('.') {
            return self.ty(a, module).map(|u| format!("{}#{}", u, slug(b.trim_end_matches("()"))));
        }
        self.ty(t, module)
            .or_else(|| Self::pick(self.funcs.get(t), module))
            .or_else(|| self.builtins.get(t).cloned())
    }
}

fn visible(items: &[Item], private: bool) -> impl Iterator<Item = &Item> {
    items.iter().filter(move |i| private || !i.private)
}

struct Ctx<'a> {
    link: &'a Linker,
    module: String,
}

impl Ctx<'_> {
    fn ty(&self, t: &TypeExpr) -> String {
        match &t.kind {
            TypeExprKind::Named(n, args) => {
                let head = if PRIMITIVES.contains(&n.as_str()) {
                    format!("<span class=\"prim\">{}</span>", esc(n))
                } else if let Some(u) = self.link.ty(n, &self.module) {
                    format!("<a class=\"ty\" href=\"{}\">{}</a>", u, esc(n))
                } else {
                    format!("<span class=\"ty\">{}</span>", esc(n))
                };
                if args.is_empty() {
                    head
                } else {
                    format!("{}&lt;{}&gt;", head, args.iter().map(|a| self.ty(a)).collect::<Vec<_>>().join(", "))
                }
            }
            TypeExprKind::Array(e) => format!("[{}]", self.ty(e)),
            TypeExprKind::Optional(e) => match e.kind {
                TypeExprKind::Func(..) => format!("({})?", self.ty(e)),
                _ => format!("{}?", self.ty(e)),
            },
            TypeExprKind::Map(k, v) => format!("{{{}: {}}}", self.ty(k), self.ty(v)),
            TypeExprKind::Func(ps, r) => {
                let p: Vec<String> = ps.iter().map(|x| self.ty(x)).collect();
                match r {
                    Some(r) => format!("<span class=\"kw\">fun</span>({}): {}", p.join(", "), self.ty(r)),
                    None => format!("<span class=\"kw\">fun</span>({})", p.join(", ")),
                }
            }
        }
    }

    fn params(&self, ps: &[(String, TypeExpr)]) -> String {
        ps.iter()
            .map(|(n, t)| format!("<span class=\"pn\">{}</span>: {}", esc(n), self.ty(t)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn sig(&self, prefix: &str, name: &str, s: &Sig) -> String {
        let r = s.ret.as_ref().map(|r| format!(": {}", self.ty(r))).unwrap_or_default();
        format!(
            "{}{}<span class=\"kw\">fun</span> <span class=\"fn\">{}</span>({}){}",
            prefix,
            if s.is_async { "<span class=\"kw\">async</span> " } else { "" },
            esc(name),
            self.params(&s.params),
            r
        )
    }

    fn inline(&self, text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        loop {
            let tick = rest.find('`');
            let tag = rest.find("{@");
            let next = match (tick, tag) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => break,
            };
            out.push_str(&esc(&rest[..next]));
            let after = &rest[next..];
            if let Some(tail) = after.strip_prefix('`') {
                match tail.find('`') {
                    Some(e) => {
                        let code = &tail[..e];
                        match self
                            .link
                            .target(code, &self.module)
                            .filter(|_| code.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '(' || c == ')'))
                        {
                            Some(u) if code.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) => {
                                let _ = write!(out, "<a href=\"{}\"><code>{}</code></a>", u, esc(code));
                            }
                            _ => {
                                let _ = write!(out, "<code>{}</code>", esc(code));
                            }
                        }
                        rest = &after[2 + e..];
                    }
                    None => {
                        out.push_str(&esc(after));
                        rest = "";
                    }
                }
            } else {
                match after.find('}') {
                    Some(e) => {
                        let inner = &after[2..e];
                        let (tag, arg) = inner.split_once(char::is_whitespace).unwrap_or((inner, ""));
                        let arg = arg.trim();
                        if tag == "link" || tag == "linkplain" {
                            let (target, label) = arg.split_once(char::is_whitespace).unwrap_or((arg, arg));
                            match self.link.target(target, &self.module) {
                                Some(u) => {
                                    let _ = write!(out, "<a href=\"{}\"><code>{}</code></a>", u, esc(label.trim()));
                                }
                                None => {
                                    let _ = write!(out, "<code>{}</code>", esc(label.trim()));
                                }
                            }
                        } else {
                            let _ = write!(out, "<code>{}</code>", esc(arg));
                        }
                        rest = &after[e + 1..];
                    }
                    None => {
                        out.push_str(&esc(after));
                        rest = "";
                    }
                }
            }
        }
        out.push_str(&esc(rest));
        out
    }

    fn body(&self, d: &DocComment) -> String {
        let mut out = String::new();
        for b in &d.body {
            match b {
                Block::Para(p) => {
                    let _ = write!(out, "<p>{}</p>", self.inline(p));
                }
                Block::Code(c) => {
                    let _ = write!(out, "<pre class=\"code\"><code>{}</code></pre>", highlight(c));
                }
                Block::List(items) => {
                    out.push_str("<ul>");
                    for it in items {
                        let _ = write!(out, "<li>{}</li>", self.inline(it));
                    }
                    out.push_str("</ul>");
                }
            }
        }
        out
    }

    fn tags(&self, d: &DocComment, params: &[(String, TypeExpr)]) -> String {
        let mut out = String::new();
        let mut rows: Vec<(String, String)> = Vec::new();
        if !d.params.is_empty() || !params.is_empty() {
            let mut s = String::from("<dl class=\"params\">");
            for (n, _) in params {
                let desc = d.param(n).map(|t| self.inline(t)).unwrap_or_default();
                let _ = write!(s, "<dt><code>{}</code></dt><dd>{}</dd>", esc(n), desc);
            }
            for (n, desc) in &d.params {
                if !params.iter().any(|(p, _)| p == n) {
                    let _ = write!(s, "<dt><code>{}</code></dt><dd>{}</dd>", esc(n), self.inline(desc));
                }
            }
            s.push_str("</dl>");
            if !params.is_empty() || !d.params.is_empty() {
                rows.push(("Parameters".into(), s));
            }
        }
        if let Some(r) = &d.ret {
            rows.push(("Returns".into(), self.inline(r)));
        }
        if !d.throws.is_empty() {
            rows.push(("Errors".into(), d.throws.iter().map(|t| self.inline(t)).collect::<Vec<_>>().join("<br>")));
        }
        if !d.see.is_empty() {
            let links: Vec<String> = d
                .see
                .iter()
                .map(|s| match self.link.target(s, &self.module) {
                    Some(u) => format!("<a href=\"{}\"><code>{}</code></a>", u, esc(s)),
                    None => self.inline(s),
                })
                .collect();
            rows.push(("See also".into(), links.join(", ")));
        }
        if let Some(s) = &d.since {
            rows.push(("Since".into(), esc(s)));
        }
        if !d.authors.is_empty() {
            rows.push(("Author".into(), esc(&d.authors.join(", "))));
        }
        if !rows.is_empty() {
            out.push_str("<dl class=\"tags\">");
            for (k, v) in rows {
                let _ = write!(out, "<dt>{}</dt><dd>{}</dd>", k, v);
            }
            out.push_str("</dl>");
        }
        for ex in &d.examples {
            let _ = write!(
                out,
                "<div class=\"example\"><div class=\"label\">Example</div><pre class=\"code\"><code>{}</code></pre></div>",
                highlight(ex)
            );
        }
        out
    }

    fn summary(&self, d: &Option<DocComment>) -> String {
        match d {
            Some(d) => {
                let mut s = self.inline(&d.summary);
                if d.deprecated.is_some() {
                    s = format!("<span class=\"badge dep\">Deprecated</span> {}", s);
                }
                s
            }
            None => String::new(),
        }
    }
}

fn deprecated(doc: &Option<DocComment>, anns: &[String]) -> Option<String> {
    if let Some(d) = doc.as_ref().and_then(|d| d.deprecated.clone()) {
        return Some(d);
    }
    anns.iter().find(|a| a.starts_with("@Deprecated")).map(|a| {
        let inner = a
            .trim_start_matches("@Deprecated")
            .trim_start_matches('(')
            .trim_end_matches(')')
            .trim_matches('"');
        inner.to_string()
    })
}

fn highlight(code: &str) -> String {
    const KW: &[&str] = &[
        "fun",
        "var",
        "const",
        "def",
        "struct",
        "type",
        "interface",
        "enum",
        "annotation",
        "abstract",
        "static",
        "if",
        "else",
        "while",
        "for",
        "in",
        "return",
        "break",
        "continue",
        "import",
        "pub",
        "priv",
        "async",
        "await",
        "is",
        "as",
        "new",
        "destroy",
        "self",
        "true",
        "false",
        "null",
    ];
    let mut out = String::new();
    for line in code.split('\n') {
        let (src, comment) = match line.find("//") {
            Some(i) if line[..i].matches('"').count() % 2 == 0 => (&line[..i], Some(&line[i..])),
            _ => (line, None),
        };
        let chars: Vec<char> = src.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '"' {
                let start = i;
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i = (i + 1).min(chars.len());
                let s: String = chars[start..i].iter().collect();
                let _ = write!(out, "<span class=\"s\">{}</span>", esc(&s));
            } else if c.is_alphabetic() || c == '_' {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let w: String = chars[start..i].iter().collect();
                if KW.contains(&w.as_str()) {
                    let _ = write!(out, "<span class=\"k\">{}</span>", w);
                } else if PRIMITIVES.contains(&w.as_str()) || w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    let _ = write!(out, "<span class=\"t\">{}</span>", w);
                } else {
                    out.push_str(&esc(&w));
                }
            } else if c.is_ascii_digit() {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                    i += 1;
                }
                let n: String = chars[start..i].iter().collect();
                let _ = write!(out, "<span class=\"n\">{}</span>", esc(&n));
            } else {
                out.push_str(&esc(&c.to_string()));
                i += 1;
            }
        }
        if let Some(c) = comment {
            let _ = write!(out, "<span class=\"c\">{}</span>", esc(c));
        }
        out.push('\n');
    }
    out.pop();
    out
}

fn layout(site: &Site, title: &str, current: &str, nav: &str, main: &str) -> String {
    let mut side = String::new();
    let mut section = |label: &str, std: bool| {
        let ms: Vec<&DocModule> = site.modules.iter().filter(|m| m.is_std == std).collect();
        if ms.is_empty() {
            return;
        }
        let _ = write!(side, "<div class=\"nav-h\">{}</div><ul>", label);
        for m in ms {
            let u = module_url(&m.name);
            let _ = write!(
                side,
                "<li><a href=\"{}\"{}>{}</a></li>",
                u,
                if u == current { " class=\"on\"" } else { "" },
                esc(&m.name)
            );
        }
        side.push_str("</ul>");
    };
    section("Modules", false);
    section("Standard library", true);
    if !site.builtins.is_empty() {
        let _ = write!(
            side,
            "<div class=\"nav-h\">Language</div><ul><li><a href=\"builtins.html\"{}>Built-in functions</a></li></ul>",
            if current == "builtins.html" { " class=\"on\"" } else { "" }
        );
    }
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · {site}</title>
<link rel="stylesheet" href="style.css">
</head>
<body>
<header class="top">
<a class="brand" href="index.html"><span class="flame">&#x1F525;</span> {site}</a>
<div class="search"><input id="q" type="search" placeholder="Search  /" autocomplete="off" aria-label="Search"><div id="results" hidden></div></div>
</header>
<div class="wrap">
<nav class="side">{side}{nav}</nav>
<main>{main}
<footer>Generated by <code>burn doc</code></footer>
</main>
</div>
<script src="search-index.js"></script>
<script src="search.js"></script>
</body>
</html>
"#,
        title = esc(title),
        site = esc(&site.title),
        side = side,
        nav = nav,
        main = main
    )
}

fn item_nav(m: &DocModule, private: bool, current: &str) -> String {
    let mut out = format!("<div class=\"nav-h\">In {}</div><ul class=\"items\">", esc(&m.name));
    for it in visible(&m.items, private) {
        let u = if has_page(&it.kind) {
            type_url(&m.name, &it.name)
        } else {
            format!("{}#{}", module_url(&m.name), slug(&it.name))
        };
        let _ = write!(
            out,
            "<li><a href=\"{}\"{}><span class=\"ic {}\"></span>{}</a></li>",
            u,
            if u == current { " class=\"on\"" } else { "" },
            icon(&it.kind),
            esc(&it.name)
        );
    }
    out.push_str("</ul>");
    out
}

fn icon(k: &Kind) -> &'static str {
    match k {
        Kind::Function => "i-fn",
        Kind::Struct(_) => "i-st",
        Kind::Interface => "i-if",
        Kind::Enum => "i-en",
        Kind::Type | Kind::Alias | Kind::Annotation => "i-ty",
        Kind::Value { .. } => "i-va",
    }
}

fn item_head(ctx: &Ctx, it: &Item) -> String {
    match &it.kind {
        Kind::Function => ctx.sig(
            if it.public { "<span class=\"kw\">pub</span> " } else { "" },
            &it.name,
            it.sig.as_ref().unwrap(),
        ),
        Kind::Value { is_const } => format!(
            "{}<span class=\"kw\">{}</span> <span class=\"fn\">{}</span>{}{}",
            if it.public { "<span class=\"kw\">pub</span> " } else { "" },
            if *is_const { "const" } else { "var" },
            esc(&it.name),
            it.ty.as_ref().map(|t| format!(": {}", ctx.ty(t))).unwrap_or_default(),
            it.default.as_ref().map(|d| format!(" = {}", esc(d))).unwrap_or_default()
        ),
        Kind::Alias => format!(
            "<span class=\"kw\">def type</span> <span class=\"fn\">{}</span> = {}",
            esc(&it.name),
            it.ty.as_ref().map(|t| ctx.ty(t)).unwrap_or_default()
        ),
        _ => {
            let kw = match &it.kind {
                Kind::Struct(StructKind::Abstract) => "def abstract struct",
                Kind::Struct(StructKind::Static) => "def static struct",
                Kind::Struct(_) => "def struct",
                Kind::Interface => "def interface",
                Kind::Enum => "def enum",
                Kind::Annotation => "def annotation",
                _ => "def type",
            };
            let mut s = format!(
                "{}<span class=\"kw\">{}</span> <span class=\"fn\">{}</span>",
                if it.public { "<span class=\"kw\">pub</span> " } else { "" },
                kw,
                esc(&it.name)
            );
            if let Some(c) = &it.ctor {
                if !c.is_empty() {
                    let _ = write!(s, "({})", ctx.params(c));
                }
            }
            if let Some(e) = &it.extends {
                let _ = write!(s, " : {}", link_name(ctx, e));
            }
            if !it.implements.is_empty() {
                let _ = write!(s, " :: {}", it.implements.iter().map(|x| link_name(ctx, x)).collect::<Vec<_>>().join(", "));
            }
            s
        }
    }
}

fn link_name(ctx: &Ctx, n: &str) -> String {
    match ctx.link.ty(n, &ctx.module) {
        Some(u) => format!("<a class=\"ty\" href=\"{}\">{}</a>", u, esc(n)),
        None => format!("<span class=\"ty\">{}</span>", esc(n)),
    }
}

fn detail(ctx: &Ctx, id: &str, name: &str, head: &str, doc: &Option<DocComment>, anns: &[String], params: &[(String, TypeExpr)]) -> String {
    let mut out = format!(
        "<section class=\"member\" id=\"{}\"><h3><a href=\"#{}\">{}</a></h3>",
        slug(id),
        slug(id),
        esc(name)
    );
    for a in anns.iter().filter(|a| !a.starts_with("@Deprecated")) {
        let _ = write!(out, "<div class=\"ann\">{}</div>", esc(a));
    }
    let _ = write!(out, "<pre class=\"sig\">{}</pre>", head);
    if let Some(dep) = deprecated(doc, anns) {
        let _ = write!(out, "<div class=\"deprecated\"><b>Deprecated.</b> {}</div>", ctx.inline(&dep));
    }
    match doc {
        Some(d) => {
            out.push_str(&ctx.body(d));
            out.push_str(&ctx.tags(d, params));
        }
        None => out.push_str("<p class=\"undoc\">No description.</p>"),
    }
    out.push_str("</section>");
    out
}

fn summary_table(title: &str, rows: &[(String, String)]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut out = format!("<h2>{}</h2><table class=\"summary\"><tbody>", title);
    for (a, b) in rows {
        let _ = write!(out, "<tr><td class=\"name\">{}</td><td>{}</td></tr>", a, b);
    }
    out.push_str("</tbody></table>");
    out
}

fn module_page(site: &Site, link: &Linker, m: &DocModule) -> Page {
    let ctx = Ctx { link, module: m.name.clone() };
    let url = module_url(&m.name);
    let mut main = format!(
        "<div class=\"crumbs\"><a href=\"index.html\">Overview</a> / {}</div><h1><span class=\"kind\">{}</span>{}</h1>",
        esc(&m.name),
        if m.is_std { "standard library module" } else { "module" },
        esc(&m.name)
    );
    if let Some(d) = &m.doc {
        main.push_str(&ctx.body(d));
        main.push_str(&ctx.tags(d, &[]));
    }
    let items: Vec<&Item> = visible(&m.items, site.private).collect();
    let groups: [KindGroup; 6] = [
        ("Structs", |k| matches!(k, Kind::Struct(_))),
        ("Interfaces", |k| matches!(k, Kind::Interface)),
        ("Enums", |k| matches!(k, Kind::Enum)),
        ("Types and annotations", |k| matches!(k, Kind::Type | Kind::Alias | Kind::Annotation)),
        ("Functions", |k| matches!(k, Kind::Function)),
        ("Constants and variables", |k| matches!(k, Kind::Value { .. })),
    ];
    for (title, f) in groups {
        let rows: Vec<(String, String)> = items
            .iter()
            .filter(|i| f(&i.kind))
            .map(|i| {
                let u = if has_page(&i.kind) {
                    type_url(&m.name, &i.name)
                } else {
                    format!("#{}", slug(&i.name))
                };
                let name = match &i.sig {
                    Some(s) => format!(
                        "<a href=\"{}\"><code>{}</code></a><div class=\"mini\">{}</div>",
                        u,
                        esc(&i.name),
                        ctx.sig("", &i.name, s).replace("<span class=\"kw\">fun</span> ", "")
                    ),
                    None => format!("<a href=\"{}\"><code>{}</code></a>", u, esc(&i.name)),
                };
                let label = match &i.kind {
                    Kind::Struct(StructKind::Abstract) => "<span class=\"badge\">abstract</span> ",
                    Kind::Struct(StructKind::Static) => "<span class=\"badge\">static</span> ",
                    _ => "",
                };
                let dep = if i.doc.as_ref().map(|d| d.deprecated.is_none()).unwrap_or(true) && deprecated(&None, &i.annotations).is_some() {
                    "<span class=\"badge dep\">Deprecated</span> "
                } else {
                    ""
                };
                (name, format!("{}{}{}", label, dep, ctx.summary(&i.doc)))
            })
            .collect();
        main.push_str(&summary_table(title, &rows));
    }
    let details: Vec<&&Item> = items.iter().filter(|i| !has_page(&i.kind)).collect();
    if !details.is_empty() {
        main.push_str("<h2>Details</h2>");
        for i in details {
            let params = i.sig.as_ref().map(|s| s.params.clone()).unwrap_or_default();
            main.push_str(&detail(&ctx, &i.name, &i.name, &item_head(&ctx, i), &i.doc, &i.annotations, &params));
        }
    }
    let nav = item_nav(m, site.private, &url);
    Page {
        content: layout(site, &m.name, &url, &nav, &main),
        path: url,
    }
}

fn find_item<'a>(site: &'a Site, name: &str, module: &str) -> Option<(&'a DocModule, &'a Item)> {
    let mut found = None;
    for m in &site.modules {
        if let Some(i) = m.items.iter().find(|i| i.name == name && has_page(&i.kind)) {
            if m.name == module {
                return Some((m, i));
            }
            found.get_or_insert((m, i));
        }
    }
    found
}

fn type_page(site: &Site, link: &Linker, m: &DocModule, it: &Item) -> Page {
    let ctx = Ctx { link, module: m.name.clone() };
    let url = type_url(&m.name, &it.name);
    let mut main = format!(
        "<div class=\"crumbs\"><a href=\"index.html\">Overview</a> / <a href=\"{}\">{}</a> / {}</div><h1><span class=\"kind\">{}</span>{}</h1>",
        module_url(&m.name),
        esc(&m.name),
        esc(&it.name),
        kind_label(&it.kind),
        esc(&it.name)
    );
    let mut chain = Vec::new();
    let mut cur = it.extends.clone();
    let mut guard = 0;
    while let Some(p) = cur {
        guard += 1;
        if guard > 32 {
            break;
        }
        chain.push(p.clone());
        cur = find_item(site, &p, &m.name).and_then(|(_, i)| i.extends.clone());
    }
    if !chain.is_empty() {
        let _ = write!(
            main,
            "<div class=\"hier\">{} ← {}</div>",
            chain.iter().rev().map(|c| link_name(&ctx, c)).collect::<Vec<_>>().join(" ← "),
            esc(&it.name)
        );
    }
    for a in it.annotations.iter().filter(|a| !a.starts_with("@Deprecated")) {
        let _ = write!(main, "<div class=\"ann\">{}</div>", esc(a));
    }
    let _ = write!(main, "<pre class=\"sig\">{}</pre>", item_head(&ctx, it));
    let mut subs: Vec<String> = Vec::new();
    for om in &site.modules {
        for oi in &om.items {
            if oi.extends.as_deref() == Some(&it.name) || oi.implements.iter().any(|x| x == &it.name) {
                subs.push(format!("<a class=\"ty\" href=\"{}\">{}</a>", type_url(&om.name, &oi.name), esc(&oi.name)));
            }
        }
    }
    if let Some(dep) = deprecated(&it.doc, &it.annotations) {
        let _ = write!(main, "<div class=\"deprecated\"><b>Deprecated.</b> {}</div>", ctx.inline(&dep));
    }
    match &it.doc {
        Some(d) => {
            main.push_str(&ctx.body(d));
            let mut d2 = d.clone();
            d2.params.clear();
            main.push_str(&ctx.tags(&d2, &[]));
        }
        None => main.push_str("<p class=\"undoc\">No description.</p>"),
    }
    if !subs.is_empty() {
        let _ = write!(
            main,
            "<dl class=\"tags\"><dt>{}</dt><dd>{}</dd></dl>",
            if matches!(it.kind, Kind::Interface) {
                "Implemented by"
            } else {
                "Extended by"
            },
            subs.join(", ")
        );
    }
    let members: Vec<&Member> = it.members.iter().filter(|x| site.private || !x.private).collect();
    let row = |x: &Member| -> (String, String) {
        let name = match &x.sig {
            Some(s) => format!(
                "<a href=\"#{}\"><code>{}</code></a><div class=\"mini\">{}</div>",
                slug(&x.name),
                esc(&x.name),
                ctx.sig("", &x.name, s).replace("<span class=\"kw\">fun</span> ", "")
            ),
            None => format!(
                "<a href=\"#{}\"><code>{}</code></a>{}",
                slug(&x.name),
                esc(&x.name),
                x.ty.as_ref().map(|t| format!("<div class=\"mini\">{}</div>", ctx.ty(t))).unwrap_or_default()
            ),
        };
        let badge = match x.kind {
            MemberKind::AbstractMethod if !matches!(it.kind, Kind::Interface) => "<span class=\"badge\">abstract</span> ",
            MemberKind::StaticMethod | MemberKind::StaticValue => "<span class=\"badge\">static</span> ",
            _ => "",
        };
        let dep = if x.doc.as_ref().map(|d| d.deprecated.is_none()).unwrap_or(true) && deprecated(&None, &x.annotations).is_some() {
            "<span class=\"badge dep\">Deprecated</span> "
        } else {
            ""
        };
        (name, format!("{}{}{}", badge, dep, ctx.summary(&x.doc)))
    };
    if let Some(c) = &it.ctor {
        let _ = write!(
            main,
            "<h2>Constructor</h2><pre class=\"sig\"><span class=\"kw\">new</span> <span class=\"fn\">{}</span>({})</pre>",
            esc(&it.name),
            ctx.params(c)
        );
        if let Some(d) = &it.doc {
            if !c.is_empty() {
                let only = DocComment {
                    params: d.params.clone(),
                    ..Default::default()
                };
                main.push_str(&ctx.tags(&only, c));
            }
        }
    }
    let sets: [MemberGroup; 5] = [
        ("Variants", |k| *k == MemberKind::Variant),
        ("Fields", |k| *k == MemberKind::Field),
        ("Static values", |k| *k == MemberKind::StaticValue),
        ("Methods", |k| {
            matches!(k, MemberKind::Method | MemberKind::AbstractMethod | MemberKind::Init | MemberKind::Destructor)
        }),
        ("Static functions", |k| *k == MemberKind::StaticMethod),
    ];
    for (title, f) in sets {
        let rows: Vec<(String, String)> = members.iter().filter(|x| f(&x.kind)).map(|x| row(x)).collect();
        main.push_str(&summary_table(title, &rows));
    }
    let own: Vec<&str> = it.members.iter().map(|x| x.name.as_str()).collect();
    let mut cur = it.extends.clone();
    let mut guard = 0;
    while let Some(p) = cur {
        guard += 1;
        if guard > 32 {
            break;
        }
        let Some((pm, pi)) = find_item(site, &p, &m.name) else { break };
        let inherited: Vec<String> = pi
            .members
            .iter()
            .filter(|x| matches!(x.kind, MemberKind::Method | MemberKind::Field | MemberKind::AbstractMethod) && !own.contains(&x.name.as_str()) && !x.private)
            .map(|x| {
                format!(
                    "<a href=\"{}#{}\"><code>{}</code></a>",
                    type_url(&pm.name, &pi.name),
                    slug(&x.name),
                    esc(&x.name)
                )
            })
            .collect();
        if !inherited.is_empty() {
            let _ = write!(
                main,
                "<div class=\"inherited\"><b>Inherited from {}:</b> {}</div>",
                link_name(&ctx, &p),
                inherited.join(", ")
            );
        }
        cur = pi.extends.clone();
    }
    if !members.is_empty() {
        main.push_str("<h2>Details</h2>");
        for x in members {
            let head = match (&x.kind, &x.sig) {
                (MemberKind::Variant, Some(s)) => format!("{}.<span class=\"fn\">{}</span>({})", esc(&it.name), esc(&x.name), ctx.params(&s.params)),
                (_, Some(s)) => {
                    let prefix = match x.kind {
                        MemberKind::StaticMethod => "<span class=\"kw\">static</span> ",
                        MemberKind::AbstractMethod if !matches!(it.kind, Kind::Interface) => "<span class=\"kw\">abstract</span> ",
                        _ => "",
                    };
                    ctx.sig(if x.private { "<span class=\"kw\">priv</span> " } else { "" }, &x.name, s).replacen(
                        "<span class=\"kw\">fun</span>",
                        &format!("{}<span class=\"kw\">fun</span>", prefix),
                        1,
                    )
                }
                (MemberKind::Variant, _) => format!("{}.<span class=\"fn\">{}</span>", esc(&it.name), esc(&x.name)),
                _ => format!(
                    "{}{}<span class=\"fn\">{}</span>{}{}",
                    if x.private { "<span class=\"kw\">priv</span> " } else { "" },
                    match x.kind {
                        MemberKind::StaticValue => {
                            if x.is_const {
                                "<span class=\"kw\">static const</span> "
                            } else {
                                "<span class=\"kw\">static var</span> "
                            }
                        }
                        _ => "",
                    },
                    esc(&x.name),
                    x.ty.as_ref().map(|t| format!(": {}", ctx.ty(t))).unwrap_or_default(),
                    x.default.as_ref().map(|d| format!(" = {}", esc(d))).unwrap_or_default()
                ),
            };
            let params = x.sig.as_ref().map(|s| s.params.clone()).unwrap_or_default();
            main.push_str(&detail(&ctx, &x.name, &x.name, &head, &x.doc, &x.annotations, &params));
        }
    }
    let nav = item_nav(m, site.private, &url);
    Page {
        content: layout(site, &format!("{} {}", kind_label(&it.kind), it.name), &url, &nav, &main),
        path: url,
    }
}

const GROUPS: &[&str] = &[
    "Output",
    "Input",
    "Conversion",
    "Strings",
    "Arrays",
    "Maps",
    "Math",
    "Random",
    "Time",
    "Files",
    "JSON",
    "HTTP",
    "Async",
    "Types",
    "Program",
];

fn builtins_page(site: &Site, link: &Linker) -> Page {
    let ctx = Ctx { link, module: String::new() };
    let mut main = String::from(
        "<div class=\"crumbs\"><a href=\"index.html\">Overview</a> / Built-in functions</div><h1><span class=\"kind\">language</span>Built-in functions</h1><p>These functions are always available, without an import. Functions that take a value first can also be called as methods on it: <code>text.upper()</code> is <code>upper(text)</code>.</p>",
    );
    let mut groups: Vec<String> = GROUPS.iter().map(|s| s.to_string()).collect();
    for b in &site.builtins {
        let g = b.doc.group.clone().unwrap_or_else(|| "Other".into());
        if !groups.contains(&g) {
            groups.push(g);
        }
    }
    let mut nav = String::from("<div class=\"nav-h\">Groups</div><ul class=\"items\">");
    for g in &groups {
        let list: Vec<&Builtin> = site.builtins.iter().filter(|b| b.doc.group.as_deref().unwrap_or("Other") == g).collect();
        if list.is_empty() {
            continue;
        }
        let _ = write!(nav, "<li><a href=\"#g-{}\">{}</a></li>", slug(g), esc(g));
        let rows: Vec<(String, String)> = list
            .iter()
            .map(|b| {
                let aliases = if b.doc.aliases.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<div class=\"mini\">also {}</div>",
                        b.doc.aliases.iter().map(|a| format!("<code>{}</code>", esc(a))).collect::<Vec<_>>().join(", ")
                    )
                };
                (
                    format!("<a href=\"#{}\"><code>{}</code></a>{}", b.name, esc(&b.name), aliases),
                    ctx.summary(&Some(b.doc.clone())),
                )
            })
            .collect();
        let _ = write!(main, "<div id=\"g-{}\"></div>", slug(g));
        main.push_str(&summary_table(g, &rows));
    }
    nav.push_str("</ul>");
    main.push_str("<h2>Details</h2>");
    for b in &site.builtins {
        let head = highlight(&b.sig);
        let mut d = b.doc.clone();
        let mut extra = String::new();
        if !d.aliases.is_empty() {
            extra = format!(
                "<dl class=\"tags\"><dt>Also called</dt><dd>{}</dd></dl>",
                d.aliases.iter().map(|a| format!("<code>{}</code>", esc(a))).collect::<Vec<_>>().join(", ")
            );
            d.aliases.clear();
        }
        if let Some(m) = b.module() {
            extra.push_str(&format!("<dl class=\"tags\"><dt>Import</dt><dd><code>import \"std/{}\"</code></dd></dl>", m));
        }
        let mut s = detail(&ctx, &b.name, &b.name, &head, &Some(d), &[], &[]);
        s.insert_str(s.len() - "</section>".len(), &extra);
        main.push_str(&s);
    }
    Page {
        content: layout(site, "Built-in functions", "builtins.html", &nav, &main),
        path: "builtins.html".into(),
    }
}

fn index_page(site: &Site, link: &Linker) -> Page {
    let ctx = Ctx { link, module: String::new() };
    let mut main = format!("<h1><span class=\"kind\">documentation</span>{}</h1>", esc(&site.title));
    if let Some(d) = site.modules.iter().find(|m| !m.is_std).and_then(|m| m.doc.as_ref()) {
        main.push_str(&ctx.body(d));
    }
    for (title, std) in [("Modules", false), ("Standard library", true)] {
        let rows: Vec<(String, String)> = site
            .modules
            .iter()
            .filter(|m| m.is_std == std)
            .map(|m| {
                let summary = m.doc.as_ref().map(|d| ctx.inline(&d.summary)).unwrap_or_else(|| {
                    let n = visible(&m.items, site.private).count();
                    format!("<span class=\"undoc\">{} documented item{}</span>", n, if n == 1 { "" } else { "s" })
                });
                (format!("<a href=\"{}\"><code>{}</code></a>", module_url(&m.name), esc(&m.name)), summary)
            })
            .collect();
        main.push_str(&summary_table(title, &rows));
    }
    if !site.builtins.is_empty() {
        main.push_str(&summary_table(
            "Language",
            &[(
                "<a href=\"builtins.html\"><code>builtins</code></a>".into(),
                format!(
                    "{} built-in functions for output, strings, arrays, maps, math, files, JSON, HTTP and more.",
                    site.builtins.len()
                ),
            )],
        ));
    }
    Page {
        content: layout(site, "Overview", "index.html", "", &main),
        path: "index.html".into(),
    }
}

fn search_index(site: &Site, link: &Linker) -> String {
    let mut entries = Vec::new();
    let ctx = Ctx { link, module: String::new() };
    let strip = |s: String| -> String {
        let mut out = String::new();
        let mut tag = false;
        for c in s.chars() {
            match c {
                '<' => tag = true,
                '>' => tag = false,
                _ if !tag => out.push(c),
                _ => {}
            }
        }
        out.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
    };
    let js = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ");
    for m in &site.modules {
        entries.push(format!("[\"{}\",\"module\",\"{}\",\"\"]", js(&m.name), module_url(&m.name)));
        for it in visible(&m.items, site.private) {
            let u = if has_page(&it.kind) {
                type_url(&m.name, &it.name)
            } else {
                format!("{}#{}", module_url(&m.name), slug(&it.name))
            };
            let s = strip(ctx.summary(&it.doc));
            entries.push(format!("[\"{}\",\"{}\",\"{}\",\"{}\"]", js(&it.name), kind_label(&it.kind), u, js(&s)));
            if has_page(&it.kind) {
                for x in it.members.iter().filter(|x| site.private || !x.private) {
                    let s = strip(ctx.summary(&x.doc));
                    entries.push(format!(
                        "[\"{}.{}\",\"member\",\"{}#{}\",\"{}\"]",
                        js(&it.name),
                        js(&x.name),
                        u,
                        slug(&x.name),
                        js(&s)
                    ));
                }
            }
        }
    }
    for b in &site.builtins {
        let s = strip(ctx.summary(&Some(b.doc.clone())));
        entries.push(format!("[\"{}\",\"builtin\",\"builtins.html#{}\",\"{}\"]", js(&b.name), b.name, js(&s)));
        for a in &b.doc.aliases {
            entries.push(format!("[\"{}\",\"builtin\",\"builtins.html#{}\",\"{}\"]", js(a), b.name, js(&s)));
        }
    }
    format!("window.BURNDOC=[\n{}\n];\n", entries.join(",\n"))
}

pub fn render(site: &Site) -> Vec<Page> {
    let link = Linker::new(site);
    let mut pages = vec![index_page(site, &link)];
    for m in &site.modules {
        pages.push(module_page(site, &link, m));
        for it in visible(&m.items, site.private) {
            if has_page(&it.kind) {
                pages.push(type_page(site, &link, m, it));
            }
        }
    }
    if !site.builtins.is_empty() {
        pages.push(builtins_page(site, &link));
    }
    pages.push(Page {
        path: "search-index.js".into(),
        content: search_index(site, &link),
    });
    pages.push(Page {
        path: "search.js".into(),
        content: include_str!("../../../../lib/doc/search.js").into(),
    });
    pages.push(Page {
        path: "style.css".into(),
        content: include_str!("../../../../lib/doc/style.css").into(),
    });
    pages
}

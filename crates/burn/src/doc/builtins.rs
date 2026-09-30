use super::comment::{self, DocComment};

#[derive(Clone, Debug)]
pub struct Builtin {
    pub name: String,
    pub sig: String,
    pub doc: DocComment,
}

pub const SOURCE: &str = include_str!("builtins.bn");

pub fn parse(src: &str) -> Vec<Builtin> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(start) = rest.find("/**") {
        let Some(end) = rest[start..].find("*/") else { break };
        let raw = &rest[start..start + end + 2];
        let after = &rest[start + end + 2..];
        let sig = after.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string();
        rest = after;
        let Some(head) = sig.strip_prefix("fun ") else { continue };
        let name: String = head.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if name.is_empty() {
            continue;
        }
        out.push(Builtin {
            name,
            sig,
            doc: comment::parse(raw),
        });
    }
    out
}

pub fn all() -> &'static [Builtin] {
    static ALL: std::sync::OnceLock<Vec<Builtin>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| parse(SOURCE))
}

pub fn find(name: &str) -> Option<&'static Builtin> {
    all().iter().find(|b| b.name == name || b.doc.aliases.iter().any(|a| a == name))
}

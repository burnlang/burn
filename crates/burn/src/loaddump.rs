use crate::project::{self, Value};
use std::fmt::Write;
use std::path::Path;

fn opt(s: &Option<String>) -> &str {
    s.as_deref().unwrap_or("-")
}

fn value(out: &mut String, depth: usize, label: &str, v: &Value) {
    let pad = "  ".repeat(depth);
    match v {
        Value::Str(s) => {
            let _ = writeln!(out, "{}{} Str {}", pad, label, crate::lexer::escape(s));
        }
        Value::Int(n) => {
            let _ = writeln!(out, "{}{} Int {}", pad, label, n);
        }
        Value::Float(f) => {
            let _ = writeln!(out, "{}{} Float {}", pad, label, burn_runtime::fmt::float_str(*f));
        }
        Value::Bool(b) => {
            let _ = writeln!(out, "{}{} Bool {}", pad, label, b);
        }
        Value::Array(items) => {
            let _ = writeln!(out, "{}{} Array", pad, label);
            for it in items {
                value(out, depth + 1, "-", it);
            }
        }
        Value::Table(t) => {
            let _ = writeln!(out, "{}{} Table", pad, label);
            table(out, depth + 1, t);
        }
    }
}

fn table(out: &mut String, depth: usize, t: &project::Table) {
    for (k, v) in t {
        value(out, depth, &crate::lexer::escape(k), v);
    }
}

pub fn toml(src: &str) -> String {
    let mut out = String::new();
    match project::parse(src) {
        Ok(t) => table(&mut out, 0, &t),
        Err(e) => {
            let _ = writeln!(out, "error {}", e);
        }
    }
    out
}

pub fn project(start: &Path) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "workspace_only {}", project::is_workspace_only(start));
    if let Some(ws) = project::find_workspace(start) {
        let _ = writeln!(out, "workspace {} {}", ws.root.display(), opt(&ws.name));
        for m in &ws.members {
            let _ = writeln!(out, "  member {} {} {}", m.path, m.dir.display(), opt(&m.name));
        }
    }
    let Some(root) = project::find_root(start) else {
        out.push_str("no project\n");
        return out;
    };
    let p = match project::load(&root) {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(out, "error {}", e);
            return out;
        }
    };
    let m = &p.manifest;
    let _ = writeln!(out, "root {}", p.root.display());
    let _ = writeln!(out, "name {}", m.name);
    let _ = writeln!(out, "version {}", m.version);
    let _ = writeln!(out, "kind {}", if m.kind == project::Kind::Lib { "lib" } else { "app" });
    let _ = writeln!(out, "target {}", m.target);
    let _ = writeln!(out, "main {}", m.main);
    let _ = writeln!(out, "output {}", opt(&m.output));
    let _ = writeln!(out, "std {}", m.std);
    let _ = writeln!(out, "main_path {}", p.main_path().display());
    let _ = writeln!(out, "source_root {}", p.source_root().display());
    let _ = writeln!(out, "lock_root {}", p.lock_root.display());
    for l in &p.lock {
        let _ = writeln!(out, "locked {} {} {}", l.name, l.rev, l.source);
    }
    let mut names: Vec<String> = vec![m.name.clone()];
    for (d, path) in &m.dependencies {
        let _ = writeln!(out, "dependency {} {}", d, opt(path));
        names.push(d.clone());
    }
    if let Some(ws) = &p.workspace {
        let _ = writeln!(out, "in_workspace {}", ws.root.display());
        names.extend(ws.members.iter().filter_map(|m| m.name.clone()));
    }
    names.extend(p.lock.iter().map(|l| l.name.clone()));
    for n in names {
        match p.resolve(&n) {
            Ok(dir) => {
                let _ = writeln!(out, "resolve {} {}", n, dir.display());
            }
            Err(e) => {
                let _ = writeln!(out, "resolve {} error {}", n, e);
            }
        }
    }
    out
}

pub fn modules(file: &Path) -> String {
    let mut out = String::new();
    let mut loader = crate::loader::Loader::new();
    let root = match loader.load_file(file) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(out, "error {}", e);
            return out;
        }
    };
    let loaded = loader.finish(root);
    for (i, m) in loaded.modules.iter().enumerate() {
        let _ = writeln!(out, "module {} {} {}", i, loaded.sm.file(m.file).name, m.key);
        for (d, s) in &m.imports {
            let _ = writeln!(out, "  import {} {} {}", d, s.start, s.end);
        }
        for (l, s) in &m.libs {
            let _ = writeln!(out, "  lib {} {} {}", l, s.start, s.end);
        }
    }
    for (i, l) in loaded.libs.iter().enumerate() {
        let _ = writeln!(out, "lib {} {} {}", i, l.name, l.path.display());
    }
    let order: Vec<String> = loaded.order.iter().map(|i| i.to_string()).collect();
    let _ = writeln!(out, "order {}", order.join(" "));
    let _ = writeln!(out, "no_std {}", loaded.no_std);
    for d in &loaded.diags {
        out.push_str(&crate::diag::render(&loaded.sm, d, false));
    }
    out
}

pub fn declarations(file: &Path) -> String {
    let mut loader = crate::loader::Loader::new();
    let root = match loader.load_file(file) {
        Ok(r) => r,
        Err(e) => return format!("error {}\n", e),
    };
    let loaded = loader.finish(root);
    crate::check::declarations(&loaded)
}

pub mod builtins;
pub mod comment;
pub mod html;
pub mod model;

use crate::lexer;
use crate::loader::{Loader, STDLIB};
use crate::parser;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn std_modules() -> Vec<model::DocModule> {
    STDLIB
        .iter()
        .map(|s| {
            let (toks, _) = lexer::lex(s.src, 0);
            let (m, _) = parser::parse_module(toks, 0);
            model::build(&format!("std/{}", s.name), true, s.src, &m)
        })
        .collect()
}

fn module_name(path: &Path, base: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    rel.with_extension("").to_string_lossy().replace('\\', "/")
}

pub fn cmd(args: &[String]) -> ExitCode {
    let mut out = PathBuf::from("burndoc");
    let mut private = false;
    let mut with_std = true;
    let mut title: Option<String> = None;
    let mut files = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(o) => out = PathBuf::from(o),
                    None => {
                        eprintln!("error: {} needs a directory", args[i - 1]);
                        return ExitCode::from(2);
                    }
                }
            }
            "--title" => {
                i += 1;
                title = args.get(i).cloned();
            }
            "--private" => private = true,
            "--no-std" => with_std = false,
            a if a.starts_with('-') => {
                eprintln!("error: unknown option `{}`", a);
                return ExitCode::from(2);
            }
            f => files.push(f.to_string()),
        }
        i += 1;
    }
    let mut modules = Vec::new();
    let base = files
        .first()
        .and_then(|f| std::fs::canonicalize(f).ok())
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    let mut seen: Vec<PathBuf> = Vec::new();
    for f in &files {
        let mut loader = Loader::new();
        let root = match loader.load_file(Path::new(f)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {}", e);
                return ExitCode::from(1);
            }
        };
        let loaded = loader.finish(root);
        for d in loaded.diags.iter().filter(|d| d.severity == crate::diag::Severity::Error) {
            eprint!("{}", crate::diag::render(&loaded.sm, d, crate::driver::use_color()));
        }
        for m in &loaded.modules {
            let file = loaded.sm.file(m.file);
            let Some(p) = file.path.clone() else { continue };
            if seen.contains(&p) {
                continue;
            }
            seen.push(p.clone());
            modules.push(model::build(&module_name(&p, &base), false, &file.src, &m.ast));
        }
    }
    if with_std {
        modules.extend(std_modules());
    }
    let title = title.unwrap_or_else(|| match files.first() {
        Some(f) => Path::new(f)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Burn".into()),
        None => "Burn standard library".into(),
    });
    let site = html::Site {
        title,
        modules,
        builtins: if with_std { builtins::all().to_vec() } else { Vec::new() },
        private,
    };
    let pages = html::render(&site);
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("error: cannot create {}: {}", out.display(), e);
        return ExitCode::from(1);
    }
    for p in &pages {
        if let Err(e) = std::fs::write(out.join(&p.path), &p.content) {
            eprintln!("error: cannot write {}: {}", out.join(&p.path).display(), e);
            return ExitCode::from(1);
        }
    }
    let html_pages = pages.iter().filter(|p| p.path.ends_with(".html")).count();
    eprintln!("wrote {} pages to {}", html_pages, out.join("index.html").display());
    ExitCode::SUCCESS
}

use crate::check::{self, CheckOptions, CheckResult};
use crate::diag::{render, Diagnostic, Severity};
use crate::hir::Program;
use crate::loader::{Loaded, Loader};
use crate::source::SourceMap;
use std::io::IsTerminal;
use std::path::Path;

pub struct Compiled {
    pub program: Program,
    pub sm: SourceMap,
    pub warnings: Vec<Diagnostic>,
}

pub struct Failed {
    pub sm: SourceMap,
    pub diags: Vec<Diagnostic>,
}

pub fn use_color() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal()
}

pub fn report(sm: &SourceMap, diags: &[Diagnostic]) {
    let color = use_color();
    let mut out = String::new();
    for d in diags {
        out.push_str(&render(sm, d, color));
        out.push('\n');
    }
    let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
    if errors > 0 {
        out.push_str(&format!("{} error{} found\n", errors, if errors == 1 { "" } else { "s" }));
    }
    eprint!("{}", out);
}

pub fn load_path(path: &Path) -> Result<Loaded, String> {
    let mut loader = Loader::new();
    let root = loader.load_file(path)?;
    Ok(loader.finish(root))
}

pub fn load_source(name: &str, src: &str) -> Loaded {
    let mut loader = Loader::new();
    let root = loader.load_source(name, src.to_string(), std::env::current_dir().ok());
    loader.finish(root)
}

pub fn check_loaded(loaded: Loaded, opts: CheckOptions) -> Result<Compiled, Failed> {
    let CheckResult { program, diags, .. } = check::check(&loaded, opts);
    let mut all = loaded.diags.clone();
    all.extend(diags);
    all.sort_by_key(|d| (d.severity != Severity::Error, d.span.file, d.span.start));
    match program {
        Some(p) if !all.iter().any(|d| d.severity == Severity::Error) => Ok(Compiled {
            program: p,
            sm: loaded.sm,
            warnings: all,
        }),
        _ => Failed { sm: loaded.sm, diags: all }.into(),
    }
}

impl From<Failed> for Result<Compiled, Failed> {
    fn from(f: Failed) -> Self {
        Err(f)
    }
}

pub fn compile_path(path: &Path) -> Result<Compiled, Failed> {
    match load_path(path) {
        Ok(l) => check_loaded(l, CheckOptions::default()),
        Err(e) => Err(Failed {
            sm: SourceMap::default(),
            diags: vec![Diagnostic::error(Default::default(), e)],
        }),
    }
}

pub fn compile_source(name: &str, src: &str) -> Result<Compiled, Failed> {
    check_loaded(load_source(name, src), CheckOptions::default())
}

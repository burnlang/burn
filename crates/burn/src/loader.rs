use crate::ast::{ItemKind, Module};
use crate::diag::Diagnostic;
use crate::lexer;
use crate::parser;
use crate::source::{FileId, SourceMap, Span};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Stdlib {
    pub name: &'static str,
    pub src: &'static str,
}

pub const STDLIB: &[Stdlib] = &[
    Stdlib {
        name: "date",
        src: include_str!("../../../lib/std/date.bn"),
    },
    Stdlib {
        name: "time",
        src: include_str!("../../../lib/std/time.bn"),
    },
    Stdlib {
        name: "http",
        src: include_str!("../../../lib/std/http.bn"),
    },
    Stdlib {
        name: "math",
        src: include_str!("../../../lib/std/math.bn"),
    },
    Stdlib {
        name: "strings",
        src: include_str!("../../../lib/std/strings.bn"),
    },
    Stdlib {
        name: "json",
        src: include_str!("../../../lib/std/json.bn"),
    },
    Stdlib {
        name: "process",
        src: include_str!("../../../lib/std/process.bn"),
    },
    Stdlib {
        name: "fs",
        src: include_str!("../../../lib/std/fs.bn"),
    },
    Stdlib {
        name: "list",
        src: include_str!("../../../lib/std/list.bn"),
    },
    Stdlib {
        name: "collections",
        src: include_str!("../../../lib/std/collections.bn"),
    },
    Stdlib {
        name: "path",
        src: include_str!("../../../lib/std/path.bn"),
    },
    Stdlib {
        name: "random",
        src: include_str!("../../../lib/std/random.bn"),
    },
    Stdlib {
        name: "testing",
        src: include_str!("../../../lib/std/testing.bn"),
    },
];

pub fn stdlib_name(path: &str) -> Option<&'static Stdlib> {
    let p = path.trim_end_matches(".bn");
    let p = p
        .strip_prefix("std/")
        .or_else(|| p.strip_prefix("std:"))
        .or_else(|| p.strip_prefix("src/lib/std/"))
        .unwrap_or(p);
    STDLIB.iter().find(|s| s.name == p)
}

pub struct LoadedModule {
    pub file: FileId,
    pub ast: Module,
    pub imports: Vec<(usize, Span)>,
    pub libs: Vec<(usize, Span)>,
    pub key: String,
}

pub struct LoadedLib {
    pub name: String,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

pub struct Loaded {
    pub sm: SourceMap,
    pub modules: Vec<LoadedModule>,
    pub libs: Vec<LoadedLib>,
    pub order: Vec<usize>,
    pub diags: Vec<Diagnostic>,
    pub root: usize,
}

pub struct Loader {
    pub sm: SourceMap,
    pub modules: Vec<LoadedModule>,
    pub libs: Vec<LoadedLib>,
    pub diags: Vec<Diagnostic>,
    by_key: HashMap<String, usize>,
    pub overrides: HashMap<PathBuf, String>,
    project: Option<Result<crate::project::Project, String>>,
}

pub fn is_library_path(p: &str) -> bool {
    p.ends_with(".bvmc") || p.ends_with(".bar") || p.ends_with(".bvm")
}

thread_local! {
    static BUILDING: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn newest_source(dir: &Path, newest: &mut std::time::SystemTime) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "build" || name == "target" {
            continue;
        }
        if path.is_dir() {
            newest_source(&path, newest);
        } else if name.ends_with(".bn") || name == "burn.toml" || name.ends_with(".bvmc") {
            if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
                if t > *newest {
                    *newest = t;
                }
            }
        }
    }
}

fn stale(dir: &Path, out: &Path) -> bool {
    let Ok(built) = std::fs::metadata(out).and_then(|m| m.modified()) else {
        return true;
    };
    let mut newest = std::time::SystemTime::UNIX_EPOCH;
    newest_source(dir, &mut newest);
    if newest > built {
        return true;
    }
    match std::fs::read(out) {
        Ok(bytes) => !crate::check::libs::library_modules(&bytes)
            .map(|ms| ms.iter().all(crate::check::libs::ref_counted))
            .unwrap_or(false),
        Err(_) => true,
    }
}

fn build_package(name: &str, dir: &Path, project: crate::project::Project, out: &Path) -> Result<(), String> {
    let main = crate::project::package_main(dir)?;
    let mut loader = Loader::new();
    loader.project = Some(Ok(project));
    let root = loader.load_file(&main)?;
    let loaded = loader.finish(root);
    let compiled = crate::driver::check_loaded(loaded, crate::check::CheckOptions::default()).map_err(|f| {
        let first = f
            .diags
            .iter()
            .find(|d| d.severity == crate::diag::Severity::Error)
            .map(|d| d.message.clone())
            .unwrap_or_default();
        format!("the package `{}` does not compile, so it has no bytecode: {}", name, first)
    })?;
    let (module, _) = crate::vm::linked(&compiled.program)?;
    bvm::verify(&module).map_err(|e| format!("the bytecode of `{}` is invalid: {}", name, e))?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
    }
    std::fs::write(out, bvm::binary::encode(&module)).map_err(|e| format!("cannot write {}: {}", out.display(), e))
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn display_name(p: &Path) -> String {
    if let Ok(cwd) = std::env::current_dir() {
        if let Ok(rel) = p.strip_prefix(&cwd) {
            return rel.display().to_string();
        }
    }
    p.display().to_string()
}

impl Loader {
    pub fn new() -> Loader {
        Loader {
            sm: SourceMap::default(),
            modules: Vec::new(),
            libs: Vec::new(),
            diags: Vec::new(),
            by_key: HashMap::new(),
            overrides: HashMap::new(),
            project: None,
        }
    }

    fn project(&mut self, near: Option<&Path>) -> Result<&crate::project::Project, String> {
        if self.project.is_none() {
            let start = near.map(|p| p.to_path_buf()).or_else(|| std::env::current_dir().ok()).unwrap_or_default();
            self.project = Some(match crate::project::find_root(&start) {
                Some(root) => crate::project::load(&root),
                None => Err("package imports need a project; create one with `burn init <name>`, which writes burn.toml".into()),
            });
        }
        self.project.as_ref().unwrap().as_ref().map_err(|e| e.clone())
    }

    fn resolve_package(&mut self, name: &str, sub: &str, base: Option<&Path>) -> Result<usize, String> {
        let (dir, is_self) = {
            let project = self.project(base)?;
            (project.resolve(name)?, project.manifest.name == name)
        };
        let file = if sub.is_empty() {
            if is_self {
                let p = self.project(base)?;
                p.main_path()
            } else {
                crate::project::package_main(&dir)?
            }
        } else {
            let direct = dir.join(sub);
            if direct.is_file() {
                direct
            } else {
                dir.join(format!("{}.bn", sub))
            }
        };
        if !file.is_file() && !self.overrides.contains_key(&canonical(&file)) {
            return Err(format!(
                "cannot find `{}` in the package `{}` ({})",
                if sub.is_empty() { "its main file" } else { sub },
                name,
                file.display()
            ));
        }
        self.load_file(&file)
    }

    fn package_bytecode(&mut self, name: &str, base: Option<&Path>) -> Result<PathBuf, String> {
        let (dir, own, project) = {
            let p = self.project(base)?;
            (p.resolve(name)?, p.manifest.name == name, p.clone())
        };
        if own {
            return Err(format!(
                "a project cannot import its own bytecode; import its source with `import \"{}\"`",
                name
            ));
        }
        let out = dir.join("build").join(format!("{}.bvmc", crate::project::short_name(name)));
        if !stale(&dir, &out) {
            return Ok(out);
        }
        if BUILDING.with(|b| b.borrow().iter().any(|n| n == name)) {
            return Err(format!("the bytecode of `{}` imports itself through its packages", name));
        }
        BUILDING.with(|b| b.borrow_mut().push(name.to_string()));
        let built = build_package(name, &dir, project, &out);
        BUILDING.with(|b| {
            b.borrow_mut().pop();
        });
        built.map(|_| out)
    }

    fn load_library(&mut self, p: &str, base: Option<&Path>) -> Result<usize, String> {
        let package = p
            .strip_suffix(".bvmc")
            .and_then(crate::project::split_package_path)
            .filter(|(_, sub)| sub.is_empty());
        let path = if let Some((name, _)) = package {
            self.package_bytecode(&name, base)?
        } else {
            let mut candidates = Vec::new();
            if let Some(b) = base {
                candidates.push(b.join(p));
            }
            if let Ok(cwd) = std::env::current_dir() {
                candidates.push(cwd.join(p));
            }
            candidates
                .into_iter()
                .find(|c| c.is_file())
                .ok_or_else(|| format!("cannot find bytecode library `{}`", p))?
        };
        let c = canonical(&path);
        if let Some(i) = self.libs.iter().position(|l| l.path == c) {
            return Ok(i);
        }
        let bytes = std::fs::read(&c).map_err(|e| format!("cannot read `{}`: {}", p, e))?;
        let name = c.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        self.libs.push(LoadedLib { name, path: c, bytes });
        Ok(self.libs.len() - 1)
    }

    fn read(&self, p: &Path) -> Option<String> {
        let c = canonical(p);
        if let Some(s) = self.overrides.get(&c) {
            return Some(s.clone());
        }
        std::fs::read_to_string(p).ok()
    }

    pub fn load_file(&mut self, path: &Path) -> Result<usize, String> {
        let c = canonical(path);
        if self.modules.is_empty() && self.project.is_none() {
            if let Some(root) = crate::project::find_root(&c) {
                self.project = Some(crate::project::load(&root));
            }
        }
        let key = c.display().to_string();
        if let Some(i) = self.by_key.get(&key) {
            return Ok(*i);
        }
        let src = self.read(path).ok_or_else(|| format!("cannot read file `{}`", path.display()))?;
        Ok(self.add_module(key, display_name(&c), Some(c), src, false))
    }

    pub fn load_source(&mut self, name: &str, src: String, base: Option<PathBuf>) -> usize {
        let key = format!("<{}>#{}", name, self.modules.len());
        let path = base.map(|b| b.join(name));
        self.add_module(key, name.to_string(), path, src, false)
    }

    fn add_module(&mut self, key: String, name: String, path: Option<PathBuf>, src: String, is_std: bool) -> usize {
        let file = self.sm.add(name, path.clone(), src);
        let (toks, ld) = lexer::lex(&self.sm.file(file).src, file);
        self.diags.extend(ld);
        let (ast, pd) = parser::parse_module(toks, file);
        self.diags.extend(pd);
        let idx = self.modules.len();
        self.by_key.insert(key.clone(), idx);
        self.modules.push(LoadedModule {
            file,
            ast,
            imports: Vec::new(),
            libs: Vec::new(),
            key,
        });
        let base = path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| std::env::current_dir().ok());
        let mut imports = Vec::new();
        let mut libs = Vec::new();
        let items: Vec<(String, Span)> = self.modules[idx]
            .ast
            .items
            .iter()
            .filter_map(|it| match &it.kind {
                ItemKind::Import(list) => Some(list.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        for (p, span) in items {
            if is_library_path(&p) {
                match self.load_library(&p, base.as_deref()) {
                    Ok(li) => {
                        if !libs.iter().any(|(x, _)| *x == li) {
                            libs.push((li, span));
                        }
                    }
                    Err(e) => self.diags.push(Diagnostic::error(span, e)),
                }
                continue;
            }
            match self.resolve_import(&p, base.as_deref(), is_std) {
                Ok(m) => {
                    if m == idx {
                        self.diags.push(Diagnostic::error(span, "a module cannot import itself"));
                    } else if !imports.iter().any(|(x, _)| *x == m) {
                        imports.push((m, span));
                    }
                }
                Err(e) => self.diags.push(Diagnostic::error(span, e)),
            }
        }
        self.modules[idx].imports = imports;
        self.modules[idx].libs = libs;
        idx
    }

    fn resolve_import(&mut self, p: &str, base: Option<&Path>, from_std: bool) -> Result<usize, String> {
        if let Some((name, sub)) = crate::project::split_package_path(p.trim_end_matches(".bn")) {
            let sub = if p.ends_with(".bn") && !sub.is_empty() { format!("{}.bn", sub) } else { sub };
            return self.resolve_package(&name, &sub, base);
        }
        let mut candidates = Vec::new();
        if let Some(b) = base {
            candidates.push(b.join(p));
            candidates.push(b.join(format!("{}.bn", p)));
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join(p));
            candidates.push(cwd.join(format!("{}.bn", p)));
        }
        let std = stdlib_name(p);
        let explicit_std = p.starts_with("std/") || p.starts_with("std:") || !p.contains('/') && !p.ends_with(".bn");
        if let Some(s) = std {
            if explicit_std || from_std {
                return Ok(self.load_std(s));
            }
        }
        for c in candidates {
            if c.is_file() || self.overrides.contains_key(&canonical(&c)) {
                return self.load_file(&c);
            }
        }
        if let Some(s) = std {
            return Ok(self.load_std(s));
        }
        let known: Vec<&str> = STDLIB.iter().map(|s| s.name).collect();
        Err(format!("cannot find module `{}` (standard modules: {})", p, known.join(", ")))
    }

    fn load_std(&mut self, s: &'static Stdlib) -> usize {
        let key = format!("std:{}", s.name);
        if let Some(i) = self.by_key.get(&key) {
            return *i;
        }
        self.add_module(key, format!("std/{}.bn", s.name), None, s.src.to_string(), true)
    }

    pub fn finish(self, root: usize) -> Loaded {
        let mut order = Vec::new();
        let mut state = vec![0u8; self.modules.len()];
        fn visit(i: usize, mods: &[LoadedModule], state: &mut [u8], order: &mut Vec<usize>) {
            if state[i] != 0 {
                return;
            }
            state[i] = 1;
            for (d, _) in &mods[i].imports {
                visit(*d, mods, state, order);
            }
            state[i] = 2;
            order.push(i);
        }
        for i in 0..self.modules.len() {
            if i != root {
                continue;
            }
            visit(i, &self.modules, &mut state, &mut order);
        }
        for i in 0..self.modules.len() {
            visit(i, &self.modules, &mut state, &mut order);
        }
        Loaded {
            sm: self.sm,
            modules: self.modules,
            libs: self.libs,
            order,
            diags: self.diags,
            root,
        }
    }
}

impl Default for Loader {
    fn default() -> Self {
        Loader::new()
    }
}

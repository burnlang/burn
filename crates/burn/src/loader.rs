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
}

pub fn is_library_path(p: &str) -> bool {
    p.ends_with(".bvmc") || p.ends_with(".bar") || p.ends_with(".bvm")
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
        }
    }

    fn load_library(&mut self, p: &str, base: Option<&Path>) -> Result<usize, String> {
        let mut candidates = Vec::new();
        if let Some(b) = base {
            candidates.push(b.join(p));
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join(p));
        }
        let path = candidates
            .into_iter()
            .find(|c| c.is_file())
            .ok_or_else(|| format!("cannot find bytecode library `{}`", p))?;
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

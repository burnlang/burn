use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "burn.toml";
pub const LOCKFILE: &str = "burn.lock";

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<Value>),
    Table(Table),
}

pub type Table = Vec<(String, Value)>;

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&Table> {
        match self {
            Value::Table(t) => Some(t),
            _ => None,
        }
    }
}

pub fn get<'a>(t: &'a Table, key: &str) -> Option<&'a Value> {
    t.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn get_mut<'a>(t: &'a mut Table, key: &str) -> Option<&'a mut Value> {
    t.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    line: usize,
}

pub fn parse(src: &str) -> Result<Table, String> {
    let mut p = Parser {
        s: src.as_bytes(),
        i: 0,
        line: 1,
    };
    let mut root: Table = Vec::new();
    let mut current: Vec<String> = Vec::new();
    loop {
        p.skip_blank();
        if p.i >= p.s.len() {
            break;
        }
        if p.peek() == b'[' {
            let array = p.s.get(p.i + 1) == Some(&b'[');
            p.i += if array { 2 } else { 1 };
            p.ws();
            let path = p.key_path()?;
            p.ws();
            p.expect(b']')?;
            if array {
                p.expect(b']')?;
            }
            p.end_of_line()?;
            if array {
                let (last, parents) = path.split_last().unwrap();
                let parent = table_at(&mut root, parents, p.line)?;
                match get_mut(parent, last) {
                    Some(Value::Array(items)) => items.push(Value::Table(Vec::new())),
                    Some(_) => return Err(p.err(&format!("`{}` is not an array of tables", last))),
                    None => parent.push((last.clone(), Value::Array(vec![Value::Table(Vec::new())]))),
                }
            } else {
                table_at(&mut root, &path, p.line)?;
            }
            current = path;
            continue;
        }
        let path = p.key_path()?;
        p.ws();
        p.expect(b'=')?;
        p.ws();
        let v = p.value()?;
        p.end_of_line()?;
        let (last, parents) = path.split_last().unwrap();
        let mut full = current.clone();
        full.extend(parents.iter().cloned());
        let t = table_at(&mut root, &full, p.line)?;
        if get(t, last).is_some() {
            return Err(p.err(&format!("`{}` is defined twice", last)));
        }
        t.push((last.clone(), v));
    }
    Ok(root)
}

fn table_at<'a>(root: &'a mut Table, path: &[String], line: usize) -> Result<&'a mut Table, String> {
    let mut t = root;
    for k in path {
        if get(t, k).is_none() {
            t.push((k.clone(), Value::Table(Vec::new())));
        }
        let v = get_mut(t, k).unwrap();
        t = match v {
            Value::Table(x) => x,
            Value::Array(items) => match items.last_mut() {
                Some(Value::Table(x)) => x,
                _ => return Err(format!("line {}: `{}` is not a table", line, k)),
            },
            _ => return Err(format!("line {}: `{}` is not a table", line, k)),
        };
    }
    Ok(t)
}

impl<'a> Parser<'a> {
    fn peek(&self) -> u8 {
        self.s.get(self.i).copied().unwrap_or(0)
    }

    fn err(&self, msg: &str) -> String {
        format!("line {}: {}", self.line, msg)
    }

    fn ws(&mut self) {
        while matches!(self.peek(), b' ' | b'\t') {
            self.i += 1;
        }
    }

    fn comment(&mut self) {
        if self.peek() == b'#' {
            while self.i < self.s.len() && self.peek() != b'\n' {
                self.i += 1;
            }
        }
    }

    fn skip_blank(&mut self) {
        loop {
            self.ws();
            self.comment();
            match self.peek() {
                b'\n' => {
                    self.line += 1;
                    self.i += 1;
                }
                b'\r' => self.i += 1,
                _ => return,
            }
        }
    }

    fn end_of_line(&mut self) -> Result<(), String> {
        self.ws();
        self.comment();
        match self.peek() {
            0 => Ok(()),
            b'\r' | b'\n' => Ok(()),
            c => Err(self.err(&format!("unexpected `{}` after the value", c as char))),
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.peek() == c {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(&format!("expected `{}`", c as char)))
        }
    }

    fn key_path(&mut self) -> Result<Vec<String>, String> {
        let mut path = vec![self.key()?];
        loop {
            self.ws();
            if self.peek() != b'.' {
                return Ok(path);
            }
            self.i += 1;
            self.ws();
            path.push(self.key()?);
        }
    }

    fn key(&mut self) -> Result<String, String> {
        match self.peek() {
            b'"' => self.basic_string(),
            b'\'' => self.literal_string(),
            _ => {
                let start = self.i;
                while matches!(self.peek(), b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-') {
                    self.i += 1;
                }
                if start == self.i {
                    return Err(self.err("expected a key"));
                }
                Ok(String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
            }
        }
    }

    fn basic_string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.peek();
            self.i += 1;
            match c {
                0 | b'\n' => return Err(self.err("unterminated string")),
                b'"' => return Ok(String::from_utf8_lossy(&out).into_owned()),
                b'\\' => {
                    let e = self.peek();
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'u' => {
                            let hex = std::str::from_utf8(self.s.get(self.i..self.i + 4).unwrap_or(b"")).unwrap_or("");
                            let ch = u32::from_str_radix(hex, 16)
                                .ok()
                                .and_then(char::from_u32)
                                .ok_or_else(|| self.err("bad \\u escape"))?;
                            self.i += 4;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(self.err("unknown escape in string")),
                    }
                }
                c => out.push(c),
            }
        }
    }

    fn literal_string(&mut self) -> Result<String, String> {
        self.i += 1;
        let start = self.i;
        while !matches!(self.peek(), b'\'' | b'\n' | 0) {
            self.i += 1;
        }
        if self.peek() != b'\'' {
            return Err(self.err("unterminated string"));
        }
        let s = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
        self.i += 1;
        Ok(s)
    }

    fn value(&mut self) -> Result<Value, String> {
        match self.peek() {
            b'"' => Ok(Value::Str(self.basic_string()?)),
            b'\'' => Ok(Value::Str(self.literal_string()?)),
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_blank();
                    if self.peek() == b']' {
                        self.i += 1;
                        return Ok(Value::Array(items));
                    }
                    items.push(self.value()?);
                    self.skip_blank();
                    match self.peek() {
                        b',' => self.i += 1,
                        b']' => {}
                        _ => return Err(self.err("expected `,` or `]` in the array")),
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut t: Table = Vec::new();
                loop {
                    self.ws();
                    if self.peek() == b'}' {
                        self.i += 1;
                        return Ok(Value::Table(t));
                    }
                    let k = self.key()?;
                    self.ws();
                    self.expect(b'=')?;
                    self.ws();
                    let v = self.value()?;
                    t.push((k, v));
                    self.ws();
                    match self.peek() {
                        b',' => self.i += 1,
                        b'}' => {}
                        _ => return Err(self.err("expected `,` or `}` in the inline table")),
                    }
                }
            }
            _ => {
                let start = self.i;
                while matches!(self.peek(), b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'+' | b'.') {
                    self.i += 1;
                }
                let word = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
                match word {
                    "true" => Ok(Value::Bool(true)),
                    "false" => Ok(Value::Bool(false)),
                    "" => Err(self.err("expected a value")),
                    _ => {
                        let clean = word.replace('_', "");
                        if let Ok(n) = clean.parse::<i64>() {
                            Ok(Value::Int(n))
                        } else if let Ok(f) = clean.parse::<f64>() {
                            Ok(Value::Float(f))
                        } else {
                            Err(self.err(&format!("`{}` is not a value; put text in quotes", word)))
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    App,
    Lib,
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub kind: Kind,
    pub target: String,
    pub main: String,
    pub output: Option<String>,
    pub std: bool,
    pub dependencies: Vec<(String, Option<String>)>,
}

#[derive(Clone, Debug)]
pub struct Locked {
    pub name: String,
    pub rev: String,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub lock: Vec<Locked>,
    pub lock_root: PathBuf,
    pub workspace: Option<Workspace>,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub dir: PathBuf,
    pub path: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub root: PathBuf,
    pub name: Option<String>,
    pub members: Vec<Member>,
}

impl Workspace {
    pub fn member_named(&self, name: &str) -> Option<&Member> {
        self.members.iter().find(|m| m.name.as_deref().map(|n| same_package(n, name)).unwrap_or(false))
    }

    pub fn contains(&self, dir: &Path) -> bool {
        let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        self.members.iter().any(|m| m.dir == dir)
    }

    pub fn find(&self, sel: &str) -> Result<&Member, String> {
        let sel = sel.trim_end_matches('/');
        let hit = self.members.iter().find(|m| {
            m.path == sel
                || m.dir.file_name().map(|f| f == sel).unwrap_or(false)
                || m.name.as_deref().map(|n| same_package(n, sel) || short_name(n) == sel).unwrap_or(false)
        });
        hit.ok_or_else(|| {
            let names: Vec<&str> = self.members.iter().map(|m| m.path.as_str()).collect();
            format!("the workspace has no member `{}` (its members: {})", sel, names.join(", "))
        })
    }

    pub fn projects(&self) -> Result<Vec<Project>, String> {
        self.members.iter().map(|m| load(&m.dir)).collect()
    }
}

pub type WorkspaceManifest = (Option<String>, Vec<String>);

pub fn parse_workspace(src: &str) -> Result<Option<WorkspaceManifest>, String> {
    let root = parse(src)?;
    let Some(ws) = get(&root, "workspace") else {
        return Ok(None);
    };
    let ws = ws.as_table().ok_or("`workspace` must be a table")?;
    let name = str_field(ws, "name", "workspace")?;
    if let Some(n) = &name {
        if !valid_name(n) {
            return Err(format!("`{}` is not a valid package name; names look like `github.com/owner/project`", n));
        }
    }
    let mut members = Vec::new();
    match get(ws, "members") {
        None => {}
        Some(Value::Array(items)) => {
            for it in items {
                members.push(it.as_str().ok_or("`workspace.members` must be a list of folders in quotes")?.to_string());
            }
        }
        Some(_) => return Err("`workspace.members` must be a list of folders, like [\"common\", \"app\"]".into()),
    }
    Ok(Some((name, members)))
}

pub fn load_workspace(root: &Path) -> Result<Option<Workspace>, String> {
    let path = root.join(MANIFEST);
    let src = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let Some((name, paths)) = parse_workspace(&src).map_err(|e| format!("{}: {}", path.display(), e))? else {
        return Ok(None);
    };
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut members = Vec::new();
    for p in paths {
        let dir = root.join(&p);
        let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        let name = std::fs::read_to_string(dir.join(MANIFEST))
            .ok()
            .and_then(|s| parse_manifest(&s).ok())
            .map(|m| m.name);
        members.push(Member { dir, path: p, name });
    }
    Ok(Some(Workspace { root, name, members }))
}

pub fn find_workspace(start: &Path) -> Option<Workspace> {
    let dir = if start.is_dir() { start.to_path_buf() } else { start.parent()?.to_path_buf() };
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    let mut cur: Option<&Path> = Some(&dir);
    while let Some(d) = cur {
        if d.join(MANIFEST).is_file() {
            if let Ok(Some(ws)) = load_workspace(d) {
                return Some(ws);
            }
        }
        cur = d.parent();
    }
    None
}

pub fn is_workspace_only(root: &Path) -> bool {
    let Ok(src) = std::fs::read_to_string(root.join(MANIFEST)) else {
        return false;
    };
    let Ok(t) = parse(&src) else {
        return false;
    };
    get(&t, "workspace").is_some() && get(&t, "package").is_none()
}

pub fn valid_name(name: &str) -> bool {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.len() < 3 {
        return false;
    }
    let domain = parts[0];
    let labels: Vec<&str> = domain.split('.').collect();
    let label_ok = |l: &&str| !l.is_empty() && !l.starts_with('-') && l.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-');
    let seg_ok = |s: &str| !s.is_empty() && !s.starts_with('.') && s.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'));
    labels.len() >= 2
        && labels.iter().all(label_ok)
        && labels.last().map(|t| t.bytes().all(|c| c.is_ascii_lowercase())).unwrap_or(false)
        && parts[1..].iter().all(|p| seg_ok(p))
}

pub fn split_package_path(p: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = p.splitn(4, '/').collect();
    if parts.len() < 3 {
        return None;
    }
    let name = parts[..3].join("/");
    if !valid_name(&name) {
        return None;
    }
    Some((name, parts.get(3).map(|s| s.to_string()).unwrap_or_default()))
}

pub fn split_known(p: &str, knows: &dyn Fn(&str) -> bool) -> Option<(String, String)> {
    let parts: Vec<&str> = p.split('/').collect();
    for k in (4..=parts.len()).rev() {
        let name = parts[..k].join("/");
        if valid_name(&name) && knows(&name) {
            return Some((name, parts[k..].join("/")));
        }
    }
    split_package_path(p)
}

pub fn same_package(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

pub fn short_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

fn str_field(t: &Table, key: &str, section: &str) -> Result<Option<String>, String> {
    match get(t, key) {
        None => Ok(None),
        Some(Value::Str(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("`{}.{}` must be text in quotes", section, key)),
    }
}

pub fn parse_manifest(src: &str) -> Result<Manifest, String> {
    let root = parse(src)?;
    let pkg = get(&root, "package").and_then(|v| v.as_table()).ok_or("missing the `[package]` section")?;
    let name = str_field(pkg, "name", "package")?.ok_or("missing `name` in `[package]`")?;
    if !valid_name(&name) {
        return Err(format!("`{}` is not a valid package name; names look like `github.com/owner/project`", name));
    }
    let kind = match str_field(pkg, "kind", "package")?.as_deref() {
        None | Some("app") => Kind::App,
        Some("lib") => Kind::Lib,
        Some(k) => return Err(format!("unknown kind `{}`; use \"app\" or \"lib\"", k)),
    };
    let target = str_field(pkg, "target", "package")?.unwrap_or_else(|| "native".into());
    if !matches!(target.as_str(), "native" | "js" | "bvm" | "bar") {
        return Err(format!("unknown target `{}`; use \"bvm\", \"bar\", \"native\" or \"js\"", target));
    }
    let main = str_field(pkg, "main", "package")?.unwrap_or_else(|| if kind == Kind::Lib { "src/lib.bn".into() } else { "src/main.bn".into() });
    let std = match get(pkg, "std") {
        None => true,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err("`package.std` must be true or false".into()),
    };
    let mut dependencies = Vec::new();
    if let Some(deps) = get(&root, "dependencies") {
        let deps = deps.as_table().ok_or("`dependencies` must be a table")?;
        for (dep, v) in deps {
            let mut path = None;
            if let Value::Table(t) = v {
                for key in ["version", "git"] {
                    str_field(t, key, dep)?;
                }
                path = str_field(t, "path", dep)?;
            } else if v.as_str().is_none() {
                return Err(format!("the dependency `{}` must be a version in quotes or a table", dep));
            }
            if !valid_name(dep) {
                return Err(format!("`{}` is not a valid package name; names look like `github.com/owner/project`", dep));
            }
            dependencies.push((dep.clone(), path));
        }
    }
    if let Some(s) = get(&root, "scripts") {
        for (k, v) in s.as_table().ok_or("`scripts` must be a table")? {
            v.as_str().ok_or_else(|| format!("the script `{}` must be a command in quotes", k))?;
        }
    }
    Ok(Manifest {
        name,
        version: str_field(pkg, "version", "package")?.unwrap_or_else(|| "0.1.0".into()),
        kind,
        target,
        main,
        output: str_field(pkg, "output", "package")?,
        std,
        dependencies,
    })
}

pub fn parse_lock(src: &str) -> Result<Vec<Locked>, String> {
    let root = parse(src)?;
    let mut out = Vec::new();
    if let Some(Value::Array(items)) = get(&root, "package") {
        for it in items {
            let t = it.as_table().ok_or("every `[[package]]` must be a table")?;
            let field = |k: &str| get(t, k).and_then(|v| v.as_str()).unwrap_or("").to_string();
            out.push(Locked {
                name: field("name"),
                rev: field("rev"),
                source: field("source"),
            });
        }
    }
    Ok(out)
}

pub fn find_root(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_dir() { start.to_path_buf() } else { start.parent()?.to_path_buf() };
    if dir.as_os_str().is_empty() {
        dir = std::env::current_dir().ok()?;
    }
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    let mut cur: Option<&Path> = Some(&dir);
    while let Some(d) = cur {
        if d.join(MANIFEST).is_file() {
            return Some(d.to_path_buf());
        }
        cur = d.parent();
    }
    None
}

pub fn load(root: &Path) -> Result<Project, String> {
    let path = root.join(MANIFEST);
    let src = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    if parse_workspace(&src).ok().flatten().is_some() && parse(&src).map(|t| get(&t, "package").is_none()).unwrap_or(false) {
        return Err(format!(
            "{} is the root of a workspace, not a package\n  = help: run the command inside one of its members, or pick one with `-p <member>`",
            root.display()
        ));
    }
    let manifest = parse_manifest(&src).map_err(|e| format!("{}: {}", path.display(), e))?;
    let workspace = root
        .parent()
        .and_then(find_workspace)
        .filter(|w| w.contains(root))
        .or_else(|| load_workspace(root).ok().flatten());
    let lock_root = workspace.as_ref().map(|w| w.root.clone()).unwrap_or_else(|| root.to_path_buf());
    let lock_path = lock_root.join(LOCKFILE);
    let lock = match std::fs::read_to_string(&lock_path) {
        Ok(s) => parse_lock(&s).map_err(|e| format!("{}: {}", lock_path.display(), e))?,
        Err(_) => Vec::new(),
    };
    Ok(Project {
        root: root.to_path_buf(),
        manifest,
        lock,
        lock_root,
        workspace,
    })
}

pub fn burn_home() -> PathBuf {
    if let Some(h) = std::env::var_os("BURN_HOME").filter(|h| !h.is_empty()) {
        return PathBuf::from(h);
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default();
    PathBuf::from(home).join(".burn")
}

pub fn package_dir(project_root: &Path, l: &Locked) -> PathBuf {
    if let Some(p) = l.source.strip_prefix("path+") {
        return project_root.join(p);
    }
    let rev: String = l.rev.chars().take(12).collect();
    let mut dir = burn_home().join("packages");
    let parts: Vec<&str> = l.name.split('/').collect();
    let (repo, sub) = parts.split_at(parts.len().min(3));
    for part in repo {
        dir.push(part);
    }
    let file = dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    dir.set_file_name(format!("{}@{}", file, rev));
    for part in sub {
        dir.push(part);
    }
    dir
}

impl Project {
    pub fn locked(&self, name: &str) -> Option<&Locked> {
        self.lock.iter().find(|l| same_package(&l.name, name))
    }

    pub fn is_self(&self, name: &str) -> bool {
        same_package(&self.manifest.name, name)
    }

    pub fn knows(&self, name: &str) -> bool {
        self.is_self(name)
            || self.manifest.dependencies.iter().any(|(d, _)| same_package(d, name))
            || self.locked(name).is_some()
            || self.member(name).is_some()
    }

    pub fn member(&self, name: &str) -> Option<&Member> {
        self.workspace.as_ref().and_then(|w| w.member_named(name))
    }

    pub fn split_import(&self, p: &str) -> Option<(String, String)> {
        split_known(p, &|n| self.knows(n))
    }

    pub fn main_path(&self) -> PathBuf {
        self.root.join(&self.manifest.main)
    }

    pub fn source_root(&self) -> PathBuf {
        source_root(&self.root, &self.manifest)
    }

    pub fn resolve(&self, name: &str) -> Result<PathBuf, String> {
        if self.is_self(name) {
            return Ok(self.root.clone());
        }
        if let Some(m) = self.member(name) {
            return Ok(m.dir.clone());
        }
        let declared = self.manifest.dependencies.iter().find(|(d, _)| same_package(d, name));
        if let (None, Some((_, Some(path)))) = (self.locked(name), declared) {
            let dir = self.root.join(path);
            return if dir.join(MANIFEST).is_file() {
                Ok(dir)
            } else {
                Err(format!(
                    "the package `{}` should be in {}, but there is no burn.toml there",
                    name,
                    dir.display()
                ))
            };
        }
        let declared = declared.is_some();
        match self.locked(name) {
            Some(l) => {
                let dir = package_dir(&self.lock_root, l);
                if dir.join(MANIFEST).is_file() {
                    Ok(dir)
                } else {
                    Err(format!("the package `{}` is in burn.lock but not downloaded; run `ash install`", name))
                }
            }
            None if declared => Err(format!("the package `{}` is not installed yet; run `ash install`", name)),
            None => Err(format!(
                "the package `{}` is not a dependency; add it with `ash install {}` (this project is `{}` in {})",
                name,
                name,
                self.manifest.name,
                self.root.join(MANIFEST).display()
            )),
        }
    }
}

pub fn source_root(dir: &Path, m: &Manifest) -> PathBuf {
    match Path::new(&m.main).parent() {
        Some(p) if !p.as_os_str().is_empty() => dir.join(p),
        _ => dir.to_path_buf(),
    }
}

pub fn package_source_root(dir: &Path) -> Result<PathBuf, String> {
    let src = std::fs::read_to_string(dir.join(MANIFEST)).map_err(|e| format!("cannot read {}: {}", dir.join(MANIFEST).display(), e))?;
    let m = parse_manifest(&src).map_err(|e| format!("{}: {}", dir.join(MANIFEST).display(), e))?;
    Ok(source_root(dir, &m))
}

pub fn module_file(base: &Path, path: &str, exists: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    let path = path.trim_end_matches('/');
    let mut candidates = Vec::new();
    if path.ends_with(".bn") {
        candidates.push(base.join(path));
    } else {
        candidates.push(base.join(format!("{}.bn", path)));
        candidates.push(base.join(path).join("mod.bn"));
    }
    candidates.into_iter().find(|c| exists(c))
}

pub fn package_main(dir: &Path) -> Result<PathBuf, String> {
    let src = std::fs::read_to_string(dir.join(MANIFEST)).map_err(|e| format!("cannot read {}: {}", dir.join(MANIFEST).display(), e))?;
    let m = parse_manifest(&src).map_err(|e| format!("{}: {}", dir.join(MANIFEST).display(), e))?;
    Ok(dir.join(m.main))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_manifest() {
        let src = r#"
[package]
name = "github.com/ada/hello"
version = "1.2.0"
kind = "lib"
target = "js"

[dependencies]
"github.com/burnlang/colors" = "^1.2"
"example.org/me/local" = { path = "../local" }

[scripts]
build = "burnc src/lib.bn --target js -o build/hello.js"
"#;
        let m = parse_manifest(src).unwrap();
        assert_eq!(m.name, "github.com/ada/hello");
        assert_eq!(m.kind, Kind::Lib);
        assert_eq!(m.main, "src/lib.bn");
        assert_eq!(m.target, "js");
        assert_eq!(m.dependencies[0], ("github.com/burnlang/colors".to_string(), None));
        assert_eq!(m.dependencies[1], ("example.org/me/local".to_string(), Some("../local".to_string())));
        assert!(parse_manifest("[package]\nname = \"github.com/a/b\"\n[scripts]\nx = 1\n").is_err());
    }

    #[test]
    fn parses_a_lock() {
        let src = "version = 1\n\n[[package]]\nname = \"github.com/a/b\"\nversion = \"v1.0.0\"\nrev = \"0123456789abcdef\"\nsource = \"git+https://github.com/a/b\"\n\n[[package]]\nname = \"github.com/c/d\"\nrev = \"ff\"\nsource = \"path+../d\"\n";
        let l = parse_lock(src).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].rev, "0123456789abcdef");
        assert_eq!(l[1].source, "path+../d");
    }

    #[test]
    fn checks_names() {
        assert!(valid_name("github.com/S42yt/burn-app"));
        assert!(valid_name("gitlab.example.org/team/lib_2"));
        assert!(!valid_name("hello"));
        assert!(!valid_name("github.com/owner"));
        assert!(!valid_name("github/owner/project"));
        assert!(!valid_name("../owner/project"));
        assert_eq!(split_package_path("github.com/a/b/src/x"), Some(("github.com/a/b".into(), "src/x".into())));
        assert_eq!(split_package_path("utils/math.bn"), None);
    }

    #[test]
    fn keeps_sub_packages_inside_their_repository() {
        let l = Locked {
            name: "github.com/ada/game/common".into(),
            rev: "0123456789abcdef".into(),
            source: "git+https://github.com/ada/game".into(),
        };
        let dir = package_dir(Path::new("/p"), &l);
        assert!(dir.ends_with("packages/github.com/ada/game@0123456789ab/common"), "{}", dir.display());
    }

    #[test]
    fn knows_sub_packages_and_workspaces() {
        assert!(valid_name("github.com/ada/game/common"));
        assert!(!valid_name("github.com/ada/game/"));
        let known = |n: &str| n == "github.com/ada/game/common";
        assert_eq!(
            split_known("github.com/ada/game/common/net/http", &known),
            Some(("github.com/ada/game/common".into(), "net/http".into()))
        );
        assert_eq!(split_known("github.com/ada/other/x", &known), Some(("github.com/ada/other".into(), "x".into())));
        let ws = parse_workspace("[workspace]\nname = \"github.com/ada/game\"\nmembers = [\"common\", \"app\"]\n")
            .unwrap()
            .unwrap();
        assert_eq!(ws, (Some("github.com/ada/game".to_string()), vec!["common".to_string(), "app".to_string()]));
        assert!(parse_workspace("[workspace]\nmembers = \"common\"\n").is_err());
        assert_eq!(parse_workspace("[package]\nname = \"github.com/a/b\"\n").unwrap(), None);
    }

    #[test]
    fn reports_bad_toml() {
        assert!(parse("[package]\nname = hello\n").unwrap_err().contains("line 2"));
        assert!(parse("a = 1\na = 2\n").unwrap_err().contains("twice"));
    }
}

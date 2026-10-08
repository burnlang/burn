use crate::project::{Kind, Project};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Main,
    Bin,
    Test,
    Example,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub role: Role,
    pub name: String,
    pub path: PathBuf,
}

fn sources(dir: &Path, role: Role, nested_main: bool) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Entry> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.is_file() && path.extension().map(|x| x == "bn").unwrap_or(false) {
                let name = path.file_stem()?.to_string_lossy().into_owned();
                return Some(Entry { role, name, path });
            }
            if nested_main && path.is_dir() && path.join("main.bn").is_file() {
                let name = path.file_name()?.to_string_lossy().into_owned();
                return Some(Entry {
                    role,
                    name,
                    path: path.join("main.bn"),
                });
            }
            None
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn bins(p: &Project) -> Vec<Entry> {
    sources(&p.source_root().join("bin"), Role::Bin, true)
}

pub fn tests(p: &Project) -> Vec<Entry> {
    sources(&p.root.join("tests"), Role::Test, false)
}

pub fn examples(p: &Project) -> Vec<Entry> {
    sources(&p.root.join("examples"), Role::Example, true)
}

pub fn main_entry(p: &Project) -> Option<Entry> {
    let path = p.main_path();
    path.is_file().then(|| Entry {
        role: Role::Main,
        name: crate::project::short_name(&p.manifest.name).to_string(),
        path,
    })
}

pub fn all(p: &Project) -> Vec<Entry> {
    let mut out: Vec<Entry> = main_entry(p).into_iter().collect();
    out.extend(bins(p));
    out.extend(tests(p));
    out.extend(examples(p));
    out
}

pub fn programs(p: &Project) -> Vec<Entry> {
    let mut out: Vec<Entry> = main_entry(p).into_iter().filter(|_| p.manifest.kind == Kind::App).collect();
    out.extend(bins(p));
    out
}

pub fn find(list: Vec<Entry>, what: &str, name: &str, p: &Project) -> Result<Entry, String> {
    let names: Vec<String> = list.iter().map(|e| e.name.clone()).collect();
    match list.into_iter().find(|e| e.name == name) {
        Some(e) => Ok(e),
        None if names.is_empty() => Err(format!("{} has no {}s", p.manifest.name, what)),
        None => Err(format!("{} has no {} named `{}` (it has: {})", p.manifest.name, what, name, names.join(", "))),
    }
}

pub fn output_for(p: &Project, entry: &Entry, target: &str) -> PathBuf {
    let ext = match target {
        "js" | "javascript" | "node" => ".js",
        "bvm" | "bytecode" => ".bvmc",
        "bar" => ".bar",
        _ => "",
    };
    match entry.role {
        Role::Main => p.root.join(p.manifest.output.clone().unwrap_or_else(|| format!("build/{}{}", entry.name, ext))),
        Role::Example => p.root.join("build").join("examples").join(format!("{}{}", entry.name, ext)),
        _ => p.root.join("build").join(format!("{}{}", entry.name, ext)),
    }
}

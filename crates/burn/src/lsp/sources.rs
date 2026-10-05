use std::path::{Path, PathBuf};

pub fn dir() -> PathBuf {
    crate::project::burn_home().join("cache").join("sources").join(env!("CARGO_PKG_VERSION"))
}

pub fn is_cached(path: &Path) -> bool {
    let d = dir();
    let d = std::fs::canonicalize(&d).unwrap_or(d);
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    p.starts_with(&d)
}

pub fn is_stub(path: &Path) -> bool {
    is_cached(path) && !path.parent().map(|p| p.ends_with("std")).unwrap_or(false)
}

fn write_read_only(path: &Path, content: &str) -> Option<PathBuf> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(content) {
        return Some(path.to_path_buf());
    }
    std::fs::create_dir_all(path.parent()?).ok()?;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perm = meta.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        let _ = std::fs::set_permissions(path, perm);
    }
    std::fs::write(path, content).ok()?;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perm = meta.permissions();
        perm.set_readonly(true);
        let _ = std::fs::set_permissions(path, perm);
    }
    Some(path.to_path_buf())
}

pub fn std_file(name: &str) -> Option<PathBuf> {
    let s = crate::loader::STDLIB.iter().find(|s| s.name == name)?;
    write_read_only(&dir().join("std").join(format!("{}.bn", s.name)), s.src)
}

pub fn std_display(name: &str) -> Option<&str> {
    name.strip_prefix("std/")?.strip_suffix(".bn")
}

pub fn builtins_file() -> Option<PathBuf> {
    write_read_only(&dir().join("builtins.bn"), crate::doc::builtins::SOURCE)
}

pub fn builtin_location(name: &str) -> Option<(PathBuf, usize, usize)> {
    let b = crate::doc::builtins::find(name)?;
    let src = crate::doc::builtins::SOURCE;
    let needle = format!("fun {}(", b.name);
    let at = src.lines().position(|l| l.starts_with(&needle))?;
    Some((builtins_file()?, at, 4))
}

pub fn library_stub(lib: &str, funcs: &[(String, String)], target: &str) -> Option<(PathBuf, usize)> {
    let mut text = format!(
        "// `{}` is a bytecode library. These are the functions it exports, with the types Burn sees.\n\n",
        lib
    );
    let mut line = 0;
    for (i, (name, sig)) in funcs.iter().enumerate() {
        if name == target {
            line = i + 2;
        }
        text.push_str(sig);
        text.push('\n');
    }
    let file: String = lib.chars().map(|c| if c.is_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
    Some((write_read_only(&dir().join("libs").join(format!("{}.bn", file)), &text)?, line))
}

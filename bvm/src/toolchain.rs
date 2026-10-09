use crate::exec::Host;
use bvm_runtime::obj::{array_at, array_len, str_ref, string};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub const TOOLS: &[&str] = &["burn", "burni", "burnc", "burn-lsp"];

pub fn tool_name() -> Option<String> {
    let first = std::env::args_os().next()?;
    let stem = Path::new(&first).file_stem()?.to_string_lossy().to_lowercase();
    TOOLS.contains(&stem.as_str()).then_some(stem)
}

pub fn cli_module() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("BURN_CLI_BVM") {
        return Ok(PathBuf::from(p));
    }
    let mut exes = Vec::new();
    if let Some(first) = std::env::args_os().next().map(PathBuf::from) {
        if first.components().count() > 1 {
            exes.push(first);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        exes.push(exe.canonicalize().unwrap_or(exe));
    }
    for exe in exes {
        if let Some(p) = exe.parent().and_then(|bin| bin.parent()).map(|p| p.join("share/burn/burn.bvm")) {
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err("the Burn command line is not installed (share/burn/burn.bvm)\n  = help: install a toolchain that ships it, or set BURN_CLI_BVM to a build of compiler/src/bin/burn.bn".into())
}

fn launcher() -> String {
    let Some(first) = std::env::args_os().next().map(PathBuf::from) else {
        return "burn".into();
    };
    if first.components().count() > 1 && first.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join(&first).display().to_string();
        }
    }
    first.display().to_string()
}

fn strings(p: u64) -> Vec<String> {
    (0..array_len(p)).map(|i| str_ref(array_at(p, i)).to_string()).collect()
}

fn finish(code: i32) -> ! {
    bvm_runtime::io::flush();
    std::process::exit(code)
}

fn run_text(text: &str, args: Vec<String>) -> i32 {
    let m = match crate::asm::assemble(text) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("internal error: the compiler produced an invalid bvm module: {}", e);
            return 70;
        }
    };
    match crate::exec::run(&m, &Host::new(), args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {}", e);
            1
        }
    }
}

fn checked(text: &str) -> Result<crate::Module, String> {
    let m = crate::asm::assemble(text).map_err(|e| e.to_string())?;
    crate::verify(&m).map_err(|e| e.to_string())?;
    Ok(m)
}

pub fn host(tool: &str) -> Host {
    let mut host = Host::new();
    let name = tool.to_string();
    host.register("burn.tool", 0, move |_| string(&name));
    host.register("burn.version", 0, |_| string(crate::VERSION));
    host.register("burn.launcher", 0, |_| string(&launcher()));
    host.register("burn.colorErrors", 0, |_| {
        (std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal()) as u64
    });
    host.register("burn.runModule", 2, |a| {
        let code = run_text(str_ref(a[0]), strings(a[1]));
        finish(code)
    });
    host.register("burn.runFile", 2, |a| {
        let code = match crate::run_file(Path::new(str_ref(a[0])), strings(a[1])) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("error: {}", e);
                1
            }
        };
        finish(code)
    });
    host.register("burn.checkModule", 1, |a| match checked(str_ref(a[0])) {
        Ok(_) => string(""),
        Err(e) => string(&e),
    });
    host.register("burn.writeBytecode", 2, |a| {
        let out = Path::new(str_ref(a[1]));
        match checked(str_ref(a[0])) {
            Ok(m) => match std::fs::write(out, crate::binary::encode(&m)) {
                Ok(()) => string(""),
                Err(e) => string(&format!("cannot write {}: {}", out.display(), e)),
            },
            Err(e) => string(&e),
        }
    });
    host.register("burn.writeArchive", 3, |a| {
        let out = Path::new(str_ref(a[2]));
        string(&match write_archive(str_ref(a[0]), str_ref(a[1]), out) {
            Ok(()) => String::new(),
            Err(e) => e,
        })
    });
    host
}

fn write_archive(text: &str, name: &str, out: &Path) -> Result<(), String> {
    let m = crate::asm::assemble(text).map_err(|e| e.to_string())?;
    let mut a = crate::archive::Archive::new(name);
    a.add_module(name, m);
    a.link()?;
    std::fs::write(out, a.encode(true)).map_err(|e| format!("cannot write {}: {}", out.display(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(out) {
            let mut perm = meta.permissions();
            perm.set_mode(perm.mode() | 0o111);
            let _ = std::fs::set_permissions(out, perm);
        }
    }
    Ok(())
}

pub fn main(tool: &str, args: Vec<String>) -> ExitCode {
    let path = match cli_module() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::from(2);
        }
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: could not read {}: {}", path.display(), e);
            return ExitCode::from(2);
        }
    };
    let m = match crate::parse(&bytes) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {}: {}", path.display(), e);
            return ExitCode::from(2);
        }
    };
    match crate::exec::run(&m, &host(tool), args) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("error: {}: {}", path.display(), e);
            ExitCode::from(2)
        }
    }
}

use crate::exec::Host;
use bvm_runtime::obj::{array_at, array_len, str_ref, string};
use std::collections::{HashMap, HashSet};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

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

enum Failure {
    Error(String),
    Internal(String),
}

impl Failure {
    fn report(self) -> String {
        match self {
            Failure::Error(e) => format!("1\terror: {}", e),
            Failure::Internal(e) => format!("70\tinternal error: the compiler produced an invalid bvm module: {}", e),
        }
    }
}

fn library_bytes(libs: &[String]) -> Result<Vec<(String, Vec<u8>)>, Failure> {
    libs.iter()
        .map(|l| {
            std::fs::read(l)
                .map(|b| (l.clone(), b))
                .map_err(|e| Failure::Error(format!("cannot read {}: {}", l, e)))
        })
        .collect()
}

fn linked(text: &str, libs: &[String]) -> Result<(crate::Module, Host), Failure> {
    let m = crate::asm::assemble(text).map_err(|e| Failure::Internal(e.to_string()))?;
    if libs.is_empty() {
        return Ok((m, Host::new()));
    }
    let mut mods = vec![m];
    let mut host = Host::new();
    for (path, bytes) in library_bytes(libs)? {
        mods.extend(library_modules(&bytes).map_err(|e| Failure::Error(format!("{}: {}", path, e)))?);
        if crate::archive::is_archive(&bytes) {
            let a = crate::archive::Archive::decode(&bytes).map_err(|e| Failure::Error(format!("{}: {}", path, e)))?;
            host.extend(&a.host());
        }
    }
    Ok((crate::link(&mods).map_err(Failure::Error)?, host))
}

fn run_text(text: &str, libs: &[String], args: Vec<String>) -> Result<i32, Failure> {
    let (m, host) = linked(text, libs)?;
    bvm_runtime::io::set_args(args);
    let prog = crate::load(&m, &host).map_err(|e| match e {
        crate::LoadError::Link(_) | crate::LoadError::MissingImport(_) | crate::LoadError::ImportArity { .. } | crate::LoadError::Unlinked(_) => {
            Failure::Error(e.to_string())
        }
        e => Failure::Internal(e.to_string()),
    })?;
    let mut r = crate::Runner::new(prog.clone(), Vec::new());
    if let Some(e) = prog.entry {
        r.call(e);
    }
    r.finish();
    Ok(0)
}

fn write_bytecode(text: &str, libs: &[String], out: &Path, asm: &str) -> Result<(), Failure> {
    let (m, _) = linked(text, libs)?;
    crate::verify(&m).map_err(|e| Failure::Internal(e.to_string()))?;
    if !asm.is_empty() {
        std::fs::write(asm, crate::asm::disassemble(&m)).map_err(|e| Failure::Error(format!("cannot write {}: {}", asm, e)))?;
    }
    std::fs::write(out, crate::binary::encode(&m)).map_err(|e| Failure::Error(format!("cannot write {}: {}", out.display(), e)))
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

pub fn stale(dir: &Path, out: &Path) -> bool {
    let Ok(built) = std::fs::metadata(out).and_then(|m| m.modified()) else {
        return true;
    };
    let mut newest = std::time::SystemTime::UNIX_EPOCH;
    newest_source(dir, &mut newest);
    if newest > built {
        return true;
    }
    match std::fs::read(out) {
        Ok(bytes) => !library_modules(&bytes).map(|ms| ms.iter().all(ref_counted)).unwrap_or(false),
        Err(_) => true,
    }
}

#[derive(Default)]
struct ReplSession {
    saved: HashMap<String, u64>,
    inited: HashSet<String>,
}

static REPL: Mutex<Option<ReplSession>> = Mutex::new(None);

fn repl_start() {
    bvm_runtime::io::set_panic_mode(true);
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let p = info.payload();
        if p.downcast_ref::<bvm_runtime::io::BurnError>().is_some() || p.downcast_ref::<bvm_runtime::io::BurnExit>().is_some() {
            return;
        }
        default(info);
    }));
    *REPL.lock().unwrap() = Some(ReplSession::default());
}

fn repl_eval(text: &str, libs: &[String], globals: &[String], inits: &[String], funcs: &[u64]) -> String {
    let cli = bvm_runtime::meta::current_meta();
    let (m, host) = match linked(text, libs) {
        Ok(x) => x,
        Err(f) => {
            let r = f.report();
            let (code, message) = r.split_once('\t').unwrap_or(("1", &r));
            eprintln!("{}", message);
            finish(code.parse().unwrap_or(1))
        }
    };
    let prog = match crate::load(&m, &host) {
        Ok(p) => p,
        Err(e @ (crate::LoadError::Link(_) | crate::LoadError::MissingImport(_) | crate::LoadError::ImportArity { .. } | crate::LoadError::Unlinked(_))) => {
            eprintln!("error: {}", e);
            finish(1)
        }
        Err(e) => {
            eprintln!("internal error: the compiler produced an invalid bvm module: {}", e);
            finish(70)
        }
    };
    let mut guard = REPL.lock().unwrap();
    let session = guard.get_or_insert_with(ReplSession::default);
    let values: Vec<u64> = globals.iter().map(|g| *session.saved.get(g).unwrap_or(&0)).collect();
    let mut runner = crate::Runner::new(prog, values);
    let root = inits.last().cloned().unwrap_or_default();
    let mut result = String::new();
    for (key, &init) in inits.iter().zip(funcs) {
        if *key != root && session.inited.contains(key) {
            continue;
        }
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runner.call(init as u32)));
        bvm_runtime::io::flush();
        if let Err(e) = r {
            result = if let Some(err) = e.downcast_ref::<bvm_runtime::io::BurnError>() {
                format!("error\t{}", err.0)
            } else if let Some(ex) = e.downcast_ref::<bvm_runtime::io::BurnExit>() {
                format!("exit\t{}", ex.0)
            } else {
                "error\tinternal error".into()
            };
            break;
        }
        if *key != root {
            session.inited.insert(key.clone());
        }
    }
    let values = runner.finish();
    for (i, g) in globals.iter().enumerate() {
        if let Some(v) = values.get(i) {
            session.saved.insert(g.clone(), *v);
        }
    }
    bvm_runtime::meta::restore_meta(cli);
    result
}

fn ints(p: u64) -> Vec<u64> {
    (0..array_len(p)).map(|i| array_at(p, i)).collect()
}

fn write_archive(text: &str, name: &str, libs: &[String], out: &Path) -> Result<(), Failure> {
    let m = crate::asm::assemble(text).map_err(|e| Failure::Internal(e.to_string()))?;
    let mut a = crate::archive::Archive::new(name);
    a.add_module(name, m);
    for (path, bytes) in library_bytes(libs)? {
        let stem = Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let parts = library_modules(&bytes).map_err(|e| Failure::Error(format!("{}: {}", path, e)))?;
        for (k, m) in parts.into_iter().enumerate() {
            let part = if !m.name.is_empty() {
                m.name.clone()
            } else if k == 0 {
                stem.clone()
            } else {
                format!("{}.{}", stem, k)
            };
            a.add_module(&part, m);
        }
        if let Ok(lib) = crate::archive::Archive::decode(&bytes) {
            for (n, d) in lib.resources {
                a.add_resource(&n, d);
            }
        }
    }
    a.link().map_err(Failure::Error)?;
    std::fs::write(out, a.encode(true)).map_err(|e| Failure::Error(format!("cannot write {}: {}", out.display(), e)))?;
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

fn outcome(r: Result<(), Failure>) -> u64 {
    string(&match r {
        Ok(()) => String::new(),
        Err(f) => f.report(),
    })
}

pub fn host(tool: &str) -> Host {
    let mut host = Host::new();
    let name = tool.to_string();
    host.register("burn.tool", 0, move |_| string(&name));
    host.register("burn.version", 0, |_| string(crate::VERSION));
    host.register("burn.launcher", 0, |_| string(&launcher()));
    host.register("burn.replStart", 0, |_| {
        repl_start();
        0
    });
    host.register("burn.replReset", 0, |_| {
        *REPL.lock().unwrap() = Some(ReplSession::default());
        0
    });
    host.register("burn.replEval", 5, |a| {
        string(&repl_eval(str_ref(a[0]), &strings(a[1]), &strings(a[2]), &strings(a[3]), &ints(a[4])))
    });
    host.register("burn.readLine", 1, |a| {
        use std::io::{BufRead, Write};
        bvm_runtime::io::flush();
        print!("{}", str_ref(a[0]));
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(n) if n > 0 => {
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                string(&line)
            }
            _ => 0,
        }
    });
    host.register("burn.stale", 2, |a| stale(Path::new(str_ref(a[0])), Path::new(str_ref(a[1]))) as u64);
    host.register("burn.library", 1, |a| string(&describe_library(Path::new(str_ref(a[0])))));
    host.register("burn.colorErrors", 0, |_| {
        (std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal()) as u64
    });
    host.register("burn.runModule", 3, |a| {
        let code = match run_text(str_ref(a[0]), &strings(a[1]), strings(a[2])) {
            Ok(code) => code,
            Err(f) => {
                let r = f.report();
                let (code, message) = r.split_once('\t').unwrap_or(("1", &r));
                eprintln!("{}", message);
                code.parse().unwrap_or(1)
            }
        };
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
    host.register("burn.writeBytecode", 4, |a| {
        outcome(write_bytecode(str_ref(a[0]), &strings(a[1]), Path::new(str_ref(a[2])), str_ref(a[3])))
    });
    host.register("burn.writeArchive", 4, |a| {
        outcome(write_archive(str_ref(a[0]), str_ref(a[1]), &strings(a[2]), Path::new(str_ref(a[3]))))
    });
    host
}

fn library_module(bytes: &[u8]) -> Result<crate::Module, String> {
    if crate::archive::is_archive(bytes) {
        let a = crate::archive::Archive::decode(bytes)?;
        return a.link_with(&crate::LinkOptions {
            allow_unresolved: true,
            skip_mixins: false,
        });
    }
    crate::parse(bytes)
}

fn library_modules(bytes: &[u8]) -> Result<Vec<crate::Module>, String> {
    if crate::archive::is_archive(bytes) {
        return Ok(crate::archive::Archive::decode(bytes)?.ordered());
    }
    Ok(vec![crate::parse(bytes)?])
}

fn ref_counted(m: &crate::Module) -> bool {
    m.annotations.iter().any(|a| a.target == crate::Target::Module && a.name == "RefCounted")
}

fn describe_type(d: &crate::Desc) -> String {
    use crate::Desc;
    match d {
        Desc::Error => "error".into(),
        Desc::Void => "void".into(),
        Desc::Null => "null".into(),
        Desc::Int => "int".into(),
        Desc::Float => "float".into(),
        Desc::Bool => "bool".into(),
        Desc::Str => "str".into(),
        Desc::Any => "any".into(),
        Desc::Num(n) => format!("num\t{}", *n as u8),
        Desc::Func => "func".into(),
        Desc::Array(e) => format!("array\t{}", e),
        Desc::Map(k, v) => format!("map\t{}\t{}", k, v),
        Desc::Optional(x) => format!("optional\t{}", x),
        Desc::Future(x) => format!("future\t{}", x),
        Desc::Record { name, fields, class, .. } => {
            let mut s = format!("record\t{}\t{}\t{}", name, *class as u8, fields.len());
            for (f, t) in fields {
                s.push_str(&format!("\t{}\t{}", f, t));
            }
            s
        }
        Desc::Interface { name } => format!("interface\t{}", name),
        Desc::Enum { name, variants } => format!("enum\t{}\t{}\t{}", name, variants.len(), variants.join("\t")),
    }
}

pub fn describe_library(path: &Path) -> String {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return format!("error\t{}", e),
    };
    if let Ok(mods) = library_modules(&bytes) {
        if !mods.iter().all(ref_counted) {
            return "old".into();
        }
    }
    let m = match library_module(&bytes) {
        Ok(m) => m,
        Err(e) => return format!("error\t{}", e),
    };
    let mut out = String::new();
    for d in &m.types {
        out.push_str(&format!("type\t{}\n", describe_type(d)));
    }
    for (i, f) in m.funcs.iter().enumerate() {
        if f.external {
            continue;
        }
        let Some(name) = crate::link::export_name(&m, i as u32) else { continue };
        match &f.sig {
            None => out.push_str(&format!("func\t{}\tnone\n", name)),
            Some(sig) => {
                out.push_str(&format!("func\t{}\t{}\t{}", name, sig.ret, sig.params.len()));
                for (k, t) in sig.params.iter().enumerate() {
                    let p = f.names.get(k).cloned().unwrap_or_else(|| format!("arg{}", k));
                    out.push_str(&format!("\t{}\t{}", t, p));
                }
                out.push('\n');
            }
        }
    }
    out
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
    let host = host(tool);
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::exec::run(&m, &host, args))) {
        Ok(Ok(code)) => ExitCode::from(code as u8),
        Ok(Err(e)) => {
            eprintln!("error: {}: {}", path.display(), e);
            ExitCode::from(2)
        }
        Err(e) => match e.downcast_ref::<bvm_runtime::io::BurnExit>() {
            Some(exit) => finish(exit.0),
            None => std::panic::resume_unwind(e),
        },
    }
}

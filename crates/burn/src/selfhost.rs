use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

const HELP: &str = "build it with `sh scripts/bootstrap.sh` and set BURN_COMPILER_BVM to compiler/build/stage2.bvm, or install a toolchain that ships it";

pub fn compiler() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("BURN_COMPILER_BVM") {
        return Ok(PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.canonicalize().unwrap_or(exe);
    match exe.parent().and_then(|bin| bin.parent()).map(|p| p.join("share/burn/compiler.bvm")) {
        Some(p) if p.is_file() => Ok(p),
        _ => Err(format!(
            "the compiler written in Burn is not installed (share/burn/compiler.bvm)\n  = help: {}",
            HELP
        )),
    }
}

pub fn wanted(name: Option<&str>) -> Result<bool, String> {
    match name {
        None | Some("rust") => Ok(false),
        Some("burn") => Ok(true),
        Some(other) => Err(format!("unknown compiler `{}`; use `rust` or `burn`", other)),
    }
}

fn temp_module(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("burn-{}-{}.bvm", tag, std::process::id()))
}

pub fn build(file: &Path, out: &Path) -> Result<(), ExitCode> {
    let compiler = compiler().map_err(|e| {
        eprintln!("error: {}", e);
        ExitCode::from(2)
    })?;
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("burn"));
    let status = Command::new(exe)
        .arg(&compiler)
        .arg("build")
        .arg(file)
        .arg("-o")
        .arg(out)
        .stdout(Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(ExitCode::from(s.code().unwrap_or(1) as u8)),
        Err(e) => {
            eprintln!("error: could not run the compiler written in Burn: {}", e);
            Err(ExitCode::from(1))
        }
    }
}

pub fn run(file: &Path, args: Vec<String>) -> ExitCode {
    let tmp = temp_module("run");
    if let Err(c) = build(file, &tmp) {
        return c;
    }
    let result = bvm::run_file(&tmp, args);
    let _ = std::fs::remove_file(&tmp);
    match result {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::from(1)
        }
    }
}

pub fn check(files: &[String]) -> ExitCode {
    let tmp = temp_module("check");
    let mut code = ExitCode::SUCCESS;
    for f in files {
        if let Err(c) = build(Path::new(f), &tmp) {
            code = c;
        }
    }
    let _ = std::fs::remove_file(&tmp);
    code
}

pub fn build_module(file: &Path, out: PathBuf) -> ExitCode {
    let text = out.extension().and_then(|e| e.to_str()) == Some("bvm");
    let tmp = if text { out.clone() } else { temp_module("build") };
    if let Err(c) = build(file, &tmp) {
        return c;
    }
    if !text {
        let m = bvm::read(&tmp);
        let _ = std::fs::remove_file(&tmp);
        let m = match m {
            Ok(m) => m,
            Err(e) => {
                eprintln!("internal error: the compiler written in Burn produced an invalid bvm module: {}", e);
                return ExitCode::from(70);
            }
        };
        if let Err(e) = bvm::verify(&m) {
            eprintln!("internal error: the compiler written in Burn produced an invalid bvm module: {}", e);
            return ExitCode::from(70);
        }
        if let Err(e) = std::fs::write(&out, bvm::binary::encode(&m)) {
            eprintln!("error: cannot write {}: {}", out.display(), e);
            return ExitCode::from(1);
        }
    }
    println!("wrote {}", out.display());
    ExitCode::SUCCESS
}

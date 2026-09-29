pub mod x86;

use crate::hir::Program;
use std::path::{Path, PathBuf};
use std::process::Command;

static RUNTIME: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libburn_runtime.a"));
static NATIVE_LIBS: &str = include_str!(concat!(env!("OUT_DIR"), "/native_libs.txt"));

pub struct BuildOptions {
    pub output: PathBuf,
    pub emit_asm: Option<PathBuf>,
    pub strip: bool,
}

pub fn supported() -> Result<(), String> {
    if !cfg!(target_arch = "x86_64") || !(cfg!(target_os = "linux") || cfg!(target_os = "macos")) {
        return Err("native compilation currently supports x86_64 Linux and macOS; use `burn run` or `--target js` on this platform".into());
    }
    if RUNTIME.is_empty() {
        return Err("this burn binary was built without the native runtime library".into());
    }
    Ok(())
}

pub fn assembly(p: &Program) -> String {
    let meta = burn_runtime::meta::encode(&p.meta());
    x86::generate(p, &meta, &x86::Target::host())
}

fn temp_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("burn-build-{}-{}", std::process::id(), nanos))
}

pub fn build(p: &Program, opts: &BuildOptions) -> Result<(), String> {
    let asm = assembly(p);
    if let Some(path) = &opts.emit_asm {
        std::fs::write(path, &asm).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    }
    supported()?;
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let result = link(&dir, &asm, &opts.output, opts.strip);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn link(dir: &Path, asm: &str, output: &Path, strip: bool) -> Result<(), String> {
    let asm_path = dir.join("program.s");
    let lib_path = dir.join("libburn_runtime.a");
    std::fs::write(&asm_path, asm).map_err(|e| e.to_string())?;
    std::fs::write(&lib_path, RUNTIME).map_err(|e| e.to_string())?;
    let cc = std::env::var("BURN_CC").or_else(|_| std::env::var("CC")).unwrap_or_else(|_| "cc".into());
    let mut cmd = Command::new(&cc);
    cmd.arg(&asm_path).arg(&lib_path).arg("-o").arg(output);
    for l in NATIVE_LIBS.split_whitespace() {
        cmd.arg(l);
    }
    if cfg!(target_os = "macos") {
        cmd.arg("-Wl,-dead_strip");
    } else {
        cmd.arg("-Wl,--gc-sections");
    }
    if strip {
        cmd.arg("-s");
    }
    let out = cmd
        .output()
        .map_err(|e| format!("could not run the linker `{}`: {} (install a C toolchain or set BURN_CC)", cc, e))?;
    if !out.status.success() {
        return Err(format!("linking failed:\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(())
}

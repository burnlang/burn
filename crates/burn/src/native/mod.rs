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
    let p = crate::own::lower(p);
    x86::generate(&p, &meta, &x86::Target::host())
}

fn temp_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("burn-build-{}-{}", std::process::id(), nanos))
}

pub fn validate(p: &Program) -> Result<(), String> {
    use crate::hir::External;
    if let Some(f) = p.funcs.iter().find(|f| matches!(f.external, Some(External::Native { .. }))) {
        return Err(format!(
            "`{}` is marked @Native: functions provided by a host program only exist on bvm, so build this with `--target bvm` or `--target bar`, or run it with burni",
            f.name
        ));
    }
    let mut lib_funcs: Vec<String> = Vec::new();
    for l in &p.libs {
        let ms = crate::check::libs::library_modules(&l.bytes).map_err(|e| format!("{}: {}", l.path.display(), e))?;
        for m in ms {
            for (i, f) in m.funcs.iter().enumerate() {
                if f.external {
                    continue;
                }
                lib_funcs.push(f.name.clone());
                if let Some(e) = bvm::link::export_name(&m, i as u32) {
                    lib_funcs.push(e);
                }
                if !m.name.is_empty() {
                    lib_funcs.push(format!("{}::{}", m.name, f.name));
                }
            }
        }
    }
    for f in &p.funcs {
        for a in &f.annotations {
            if !x86::MIXINS.contains(&a.name.as_str()) {
                continue;
            }
            let target = a.str_arg("target").unwrap_or("");
            if lib_funcs.iter().any(|n| n == target) {
                continue;
            }
            if p.funcs.iter().any(|g| g.name == target && g.external.is_none()) {
                return Err(format!(
                    "@{} on `{}` targets `{}`, which is compiled to native code; mixins can only change bvm bytecode (use burni or `--target bvm`, or target a function in an imported bytecode library)",
                    a.name, f.name, target
                ));
            }
            return Err(format!(
                "@{} on `{}` targets `{}`, but no imported bytecode library has a function with that name",
                a.name, f.name, target
            ));
        }
    }
    Ok(())
}

pub fn build(p: &Program, opts: &BuildOptions) -> Result<(), String> {
    validate(p)?;
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

pub fn launcher_assembly(bytes: &[u8]) -> String {
    let t = x86::Target::host();
    let p = t.prefix;
    let plt = if t.macos { "" } else { "@PLT" };
    let mut s = String::new();
    s.push_str(".intel_syntax noprefix\n.text\n");
    s.push_str(&format!(".globl {p}main\n{p}main:\n"));
    s.push_str("    push rbp\n    mov rbp, rsp\n");
    s.push_str("    mov rdx, rdi\n    mov rcx, rsi\n");
    s.push_str("    lea rdi, [rip + burn_bundle]\n");
    s.push_str("    mov rsi, qword ptr [rip + burn_bundle_len]\n");
    s.push_str(&format!("    call {p}burn_bvm_main{plt}\n"));
    s.push_str("    pop rbp\n    ret\n");
    s.push_str(if t.macos { ".section __TEXT,__const\n" } else { ".section .rodata\n" });
    s.push_str(".p2align 4\n");
    s.push_str(&format!("burn_bundle_len:\n    .quad {}\n", bytes.len()));
    s.push_str("burn_bundle:\n");
    for chunk in bytes.chunks(32) {
        let parts: Vec<String> = chunk.iter().map(|x| x.to_string()).collect();
        s.push_str("    .byte ");
        s.push_str(&parts.join(","));
        s.push('\n');
    }
    if !t.macos {
        s.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    }
    s
}

pub fn build_launcher(bytes: &[u8], opts: &BuildOptions) -> Result<(), String> {
    let asm = launcher_assembly(bytes);
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

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let runtime_src = manifest.join("../burn-runtime/src");
    let bvm_src = manifest.join("../bvm/src");
    println!("cargo:rerun-if-changed={}", runtime_src.display());
    println!("cargo:rerun-if-changed={}", bvm_src.display());
    println!("cargo:rerun-if-changed=build.rs");
    let lib = out.join("libburn_runtime.a");
    let rlib = out.join("libburn_runtime_native.rlib");
    let libs_file = out.join("native_libs.txt");
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let target = env::var("TARGET").unwrap();
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let flags = [
        "--edition",
        "2021",
        "-C",
        "opt-level=3",
        "-C",
        "panic=abort",
        "-C",
        "debuginfo=0",
        "-C",
        "codegen-units=1",
        "-C",
        "strip=debuginfo",
    ];
    let runtime = Command::new(&rustc)
        .args(["--crate-name", "burn_runtime", "--crate-type", "rlib"])
        .args(flags)
        .args(["--target", &target])
        .arg("-o")
        .arg(&rlib)
        .arg(runtime_src.join("lib.rs"))
        .output();
    let result = match runtime {
        Ok(o) if o.status.success() => Command::new(&rustc)
            .args(["--crate-name", "bvm", "--crate-type", "staticlib"])
            .args(flags)
            .args(["--target", &target])
            .arg("--extern")
            .arg(format!("burn_runtime={}", rlib.display()))
            .arg("-L")
            .arg(&out)
            .args(["--print", "native-static-libs"])
            .env("CARGO_PKG_VERSION", &version)
            .arg("-o")
            .arg(&lib)
            .arg(bvm_src.join("lib.rs"))
            .output(),
        other => other,
    };
    let mut libs = String::new();
    let ok = match result {
        Ok(o) if o.status.success() => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            for line in stderr.lines() {
                if let Some(idx) = line.find("native-static-libs:") {
                    libs = line[idx + "native-static-libs:".len()..].trim().to_string();
                }
            }
            true
        }
        Ok(o) => {
            println!(
                "cargo:warning=could not build the native runtime: {}",
                String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("")
            );
            false
        }
        Err(e) => {
            println!("cargo:warning=could not run rustc for the native runtime: {}", e);
            false
        }
    };
    if !ok {
        let _ = fs::write(&lib, b"");
    } else {
        let flag = if target.contains("apple") { "-S" } else { "--strip-debug" };
        let _ = Command::new("strip").arg(flag).arg(&lib).output();
    }
    fs::write(&libs_file, libs).unwrap();
    let core = out.join("libburn_core.a");
    let built = Command::new(&rustc)
        .args(["--crate-name", "burn_runtime", "--crate-type", "staticlib", "--cfg", "burn_core"])
        .args(flags)
        .args(["--target", &target])
        .arg("-o")
        .arg(&core)
        .arg(runtime_src.join("lib.rs"))
        .output();
    match built {
        Ok(o) if o.status.success() => {
            let flag = if target.contains("apple") { "-S" } else { "--strip-debug" };
            let _ = Command::new("strip").arg(flag).arg(&core).output();
        }
        _ => {
            println!("cargo:warning=could not build the no-std runtime");
            let _ = fs::write(&core, b"");
        }
    }
}

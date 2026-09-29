use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let runtime_src = manifest.join("../burn-runtime/src");
    println!("cargo:rerun-if-changed={}", runtime_src.display());
    println!("cargo:rerun-if-changed=build.rs");
    let lib = out.join("libburn_runtime.a");
    let libs_file = out.join("native_libs.txt");
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let target = env::var("TARGET").unwrap();
    let result = Command::new(&rustc)
        .args(["--crate-name", "burn_runtime", "--crate-type", "staticlib", "--edition", "2021"])
        .args([
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
        ])
        .args(["--target", &target])
        .args(["--print", "native-static-libs"])
        .arg("-o")
        .arg(&lib)
        .arg(runtime_src.join("lib.rs"))
        .output();
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
}

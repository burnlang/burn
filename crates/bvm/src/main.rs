use bvm::{asm, binary, exec, verify, Host, Module};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "bvm - the Burn Virtual Machine

usage:
  bvm <file> [args...]              run a module (.bvm assembly or .bvmc bytecode)
  bvm run <file> [args...]          same as above
  bvm asm <file.bvm> [-o out.bvmc]  assemble text into bytecode
  bvm dis <file> [-o out.bvm]       disassemble a module into text
  bvm check <file>...               verify modules without running them
  bvm runtime                       list the runtime functions available to `rt`
  bvm version                       print the version
  bvm help                          show this help";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        None | Some("help") | Some("-h") | Some("--help") => {
            println!("{}", USAGE);
            ExitCode::SUCCESS
        }
        Some("version") | Some("-v") | Some("--version") => {
            println!("bvm {} (bytecode format {})", bvm::VERSION, bvm::FORMAT_VERSION);
            ExitCode::SUCCESS
        }
        Some("run") => match args.get(1) {
            Some(f) => run(f, args[2..].to_vec()),
            None => usage_error("bvm run needs a file"),
        },
        Some("asm") => convert(&args[1..], "bvmc", |m| Ok(binary::encode(m))),
        Some("dis") => convert(&args[1..], "bvm", |m| Ok(asm::disassemble(m).into_bytes())),
        Some("check") => check(&args[1..]),
        Some("link") => link(&args[1..]),
        Some("runtime") => {
            for f in burn_runtime::RtFn::all() {
                if !verify::RESERVED_RT.contains(f) {
                    println!("{:<18} {}", bvm::op::rt_name(*f), f.argc());
                }
            }
            ExitCode::SUCCESS
        }
        Some(f) if !f.starts_with('-') => run(f, args[1..].to_vec()),
        Some(f) => usage_error(&format!("unknown option {}", f)),
    }
}

fn link(args: &[String]) -> ExitCode {
    let mut files = Vec::new();
    let mut out: Option<PathBuf> = None;
    let mut opts = bvm::LinkOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                match args.get(i) {
                    Some(o) => out = Some(PathBuf::from(o)),
                    None => return usage_error("-o needs a file name"),
                }
            }
            "--lib" => opts.allow_unresolved = true,
            f => files.push(f.to_string()),
        }
        i += 1;
    }
    if files.is_empty() {
        return usage_error("bvm link needs modules to link");
    }
    let mut modules = Vec::new();
    for f in &files {
        match bvm::read(Path::new(f)) {
            Ok(m) => modules.push(m),
            Err(e) => {
                eprintln!("error: {}", e);
                return ExitCode::from(1);
            }
        }
    }
    let m = match bvm::link_with(&modules, &opts) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::from(1);
        }
    };
    let out = out.unwrap_or_else(|| Path::new(&files[0]).with_extension("linked.bvmc"));
    let bytes = if out.extension().and_then(|e| e.to_str()) == Some("bvm") {
        asm::disassemble(&m).into_bytes()
    } else {
        binary::encode(&m)
    };
    if let Err(e) = std::fs::write(&out, bytes) {
        eprintln!("error: could not write {}: {}", out.display(), e);
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("error: {}\n\n{}", msg, USAGE);
    ExitCode::from(2)
}

fn run(file: &str, args: Vec<String>) -> ExitCode {
    let m = match bvm::read(Path::new(file)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::from(1);
        }
    };
    if m.entry.is_none() {
        eprintln!("error: {} has no entry function (add `entry <name>` or a function called main)", file);
        return ExitCode::from(1);
    }
    match exec::run(&m, &Host::new(), args) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("error: {}: {}", file, e);
            ExitCode::from(1)
        }
    }
}

fn convert(args: &[String], ext: &str, f: impl Fn(&Module) -> Result<Vec<u8>, String>) -> ExitCode {
    let mut input: Option<&String> = None;
    let mut output: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                match args.get(i) {
                    Some(o) => output = Some(PathBuf::from(o)),
                    None => return usage_error("-o needs a file name"),
                }
            }
            _ if input.is_none() => input = Some(&args[i]),
            a => return usage_error(&format!("unexpected argument {}", a)),
        }
        i += 1;
    }
    let Some(input) = input else {
        return usage_error("missing input file");
    };
    let m = match bvm::read(Path::new(input)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::from(1);
        }
    };
    if let Err(e) = verify(&m) {
        eprintln!("error: {}: {}", input, e);
        return ExitCode::from(1);
    }
    let bytes = match f(&m) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::from(1);
        }
    };
    let out = output.unwrap_or_else(|| Path::new(input).with_extension(ext));
    if out == Path::new("-") {
        use std::io::Write;
        let _ = std::io::stdout().write_all(&bytes);
        return ExitCode::SUCCESS;
    }
    if let Err(e) = std::fs::write(&out, bytes) {
        eprintln!("error: could not write {}: {}", out.display(), e);
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn check(files: &[String]) -> ExitCode {
    if files.is_empty() {
        return usage_error("bvm check needs at least one file");
    }
    let mut failed = false;
    for f in files {
        match bvm::read(Path::new(f)).and_then(|m| verify(&m).map_err(|e| format!("{}: {}", f, e))) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("error: {}", e);
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

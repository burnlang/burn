mod ast;
mod check;
mod diag;
mod driver;
mod fmt;
mod hir;
mod js;
mod lexer;
mod loader;
mod lsp;
mod native;
mod parser;
mod repl;
mod source;
mod types;
mod vm;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!(
        "Burn {VERSION}

Usage:
  burn <file.bn> [args...]            run a program instantly (bytecode VM)
  burn run <file.bn> [--native] [args...]
                                      run a program; --native compiles it to machine code first
  burn build <file.bn> [options]      compile to a standalone executable
      -o, --output <path>             output file (default: file name without .bn)
      --target <native|js>            native executable (default) or JavaScript
      --emit-asm <path>               also write the generated x86-64 assembly
      --no-strip                      keep symbols in the executable
  burn check <file.bn>                type-check without running
  burn fmt [-w] [--check] <files...>  format source files
  burn repl                           start the interactive REPL
  burn eval '<code>'                  evaluate code from the command line
  burn lsp                            start the language server (stdio)
  burn version                        print the version

Legacy flags: -r (repl), -e <code> (eval), -exe <file> [name] (build), -d <file> (show IR)"
    );
}

fn default_output(file: &Path, js: bool) -> PathBuf {
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    let mut name = stem;
    if js {
        name.push_str(".js");
    } else if cfg!(windows) {
        name.push_str(".exe");
    }
    file.parent().map(|p| p.join(&name)).unwrap_or_else(|| PathBuf::from(name))
}

fn compile(path: &Path) -> Option<driver::Compiled> {
    match driver::compile_path(path) {
        Ok(c) => {
            if !c.warnings.is_empty() {
                driver::report(&c.sm, &c.warnings);
            }
            Some(c)
        }
        Err(f) => {
            driver::report(&f.sm, &f.diags);
            None
        }
    }
}

fn cmd_run(file: &Path, args: Vec<String>, native_mode: bool) -> ExitCode {
    let c = match compile(file) {
        Some(c) => c,
        None => return ExitCode::from(1),
    };
    if native_mode {
        let dir = std::env::temp_dir().join(format!("burn-run-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let exe = dir.join("program");
        let opts = native::BuildOptions {
            output: exe.clone(),
            emit_asm: None,
            strip: true,
        };
        if let Err(e) = native::build(&c.program, &opts) {
            eprintln!("error: {}", e);
            return ExitCode::from(1);
        }
        let status = std::process::Command::new(&exe).args(&args).status();
        let _ = std::fs::remove_dir_all(&dir);
        return match status {
            Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
            Err(e) => {
                eprintln!("error: could not run the compiled program: {}", e);
                ExitCode::from(1)
            }
        };
    }
    let code = vm::exec::run_program(&c.program, args);
    ExitCode::from(code as u8)
}

fn cmd_build(args: &[String]) -> ExitCode {
    let mut file: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut target = "native".to_string();
    let mut emit_asm: Option<PathBuf> = None;
    let mut strip = true;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "-o" | "--output" => {
                i += 1;
                output = args.get(i).map(PathBuf::from);
            }
            "--target" | "-t" => {
                i += 1;
                target = args.get(i).cloned().unwrap_or_default();
            }
            "--emit-asm" | "-S" => {
                i += 1;
                emit_asm = args.get(i).map(PathBuf::from);
            }
            "--no-strip" => strip = false,
            _ if a.starts_with("--target=") => target = a["--target=".len()..].to_string(),
            _ if file.is_none() => file = Some(PathBuf::from(a)),
            _ if output.is_none() => output = Some(PathBuf::from(a)),
            _ => {
                eprintln!("error: unexpected argument `{}`", a);
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    let file = match file {
        Some(f) => f,
        None => {
            eprintln!("error: no source file given");
            return ExitCode::from(2);
        }
    };
    let c = match compile(&file) {
        Some(c) => c,
        None => return ExitCode::from(1),
    };
    match target.as_str() {
        "js" | "javascript" | "node" => {
            let out = output.unwrap_or_else(|| default_output(&file, true));
            let js = js::generate(&c.program);
            if let Err(e) = std::fs::write(&out, js) {
                eprintln!("error: cannot write {}: {}", out.display(), e);
                return ExitCode::from(1);
            }
            println!("wrote {}", out.display());
            ExitCode::SUCCESS
        }
        "native" | "exe" | "asm" => {
            let out = output.unwrap_or_else(|| default_output(&file, false));
            let opts = native::BuildOptions {
                output: out.clone(),
                emit_asm,
                strip,
            };
            match native::build(&c.program, &opts) {
                Ok(()) => {
                    println!("built {}", out.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    ExitCode::from(1)
                }
            }
        }
        other => {
            eprintln!("error: unknown target `{}` (expected `native` or `js`)", other);
            ExitCode::from(2)
        }
    }
}

fn cmd_eval(code: &str) -> ExitCode {
    match driver::compile_source("<eval>", code) {
        Ok(c) => {
            if !c.warnings.is_empty() {
                driver::report(&c.sm, &c.warnings);
            }
            ExitCode::from(vm::exec::run_program(&c.program, vec![]) as u8)
        }
        Err(f) => {
            driver::report(&f.sm, &f.diags);
            ExitCode::from(1)
        }
    }
}

fn cmd_check(files: &[String]) -> ExitCode {
    let mut failed = false;
    for f in files {
        match driver::compile_path(Path::new(f)) {
            Ok(c) => {
                driver::report(&c.sm, &c.warnings);
            }
            Err(e) => {
                driver::report(&e.sm, &e.diags);
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

fn cmd_dump(file: &Path, what: &str) -> ExitCode {
    let c = match compile(file) {
        Some(c) => c,
        None => return ExitCode::from(1),
    };
    match what {
        "asm" => print!("{}", native::assembly(&c.program)),
        "js" => print!("{}", js::generate(&c.program)),
        "bytecode" => {
            let code = vm::exec::prepare(&c.program);
            for f in &code.funcs {
                println!("{} (params {}, locals {}):", f.name, f.params, f.locals);
                let end = code
                    .funcs
                    .iter()
                    .map(|x| x.entry)
                    .filter(|e| *e > f.entry)
                    .min()
                    .unwrap_or(code.ops.len() as u32);
                for i in f.entry..end {
                    println!("  {:5} {:?}", i, code.ops[i as usize]);
                }
            }
        }
        _ => {
            for (i, f) in c.program.funcs.iter().enumerate() {
                println!(
                    "fn #{} {} params={} locals={:?}",
                    i,
                    f.name,
                    f.params,
                    f.locals.iter().map(|t| c.program.types.display(*t)).collect::<Vec<_>>()
                );
                for s in &f.body {
                    println!("  {:?}", s);
                }
            }
        }
    }
    ExitCode::SUCCESS
}

fn tool_name() -> String {
    std::env::args_os()
        .next()
        .and_then(|a| Path::new(&a).file_stem().map(|s| s.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

fn burni_usage() {
    println!(
        "burni {VERSION} - the Burn interpreter

Usage:
  burni                       start the interactive REPL
  burni <file.bn> [args...]   type-check and run a program on the bytecode VM
  burni -e '<code>'           evaluate code from the command line
  burni -v | --version        print the version"
    );
}

fn burnc_usage() {
    println!(
        "burnc {VERSION} - the Burn compiler

Usage:
  burnc <file.bn> [options]   compile to a standalone native executable
      -o, --output <path>     output file (default: file name without .bn)
      --target <native|js>    native executable (default) or JavaScript
      --emit-asm <path>       also write the generated x86-64 assembly
      --no-strip              keep symbols in the executable
  burnc --check <files...>    type-check without producing output
  burnc -v | --version        print the version"
    );
}

fn burni(args: &[String]) -> ExitCode {
    match args.first().map(|s| s.as_str()) {
        None => repl::run(),
        Some("-h") | Some("--help") => {
            burni_usage();
            ExitCode::SUCCESS
        }
        Some("-v") | Some("--version") => {
            println!("burni {}", VERSION);
            ExitCode::SUCCESS
        }
        Some("-e") | Some("--eval") => match args.get(1) {
            Some(code) => cmd_eval(code),
            None => {
                eprintln!("error: no code given");
                ExitCode::from(2)
            }
        },
        Some(f) if f.starts_with('-') => {
            eprintln!("error: unknown option `{}`", f);
            burni_usage();
            ExitCode::from(2)
        }
        Some(f) => cmd_run(Path::new(f), args[1..].to_vec(), false),
    }
}

fn burnc(args: &[String]) -> ExitCode {
    match args.first().map(|s| s.as_str()) {
        None | Some("-h") | Some("--help") => {
            burnc_usage();
            if args.is_empty() {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            }
        }
        Some("-v") | Some("--version") => {
            println!("burnc {}", VERSION);
            ExitCode::SUCCESS
        }
        Some("--check") => cmd_check(&args[1..]),
        _ => cmd_build(args),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match tool_name().as_str() {
        "burni" => return burni(&args),
        "burnc" => return burnc(&args),
        "burn-lsp" => return lsp::run(),
        _ => {}
    }
    if args.is_empty() {
        usage();
        return ExitCode::from(1);
    }
    let first = args[0].as_str();
    let rest = &args[1..];
    match first {
        "-h" | "--help" | "help" => {
            usage();
            ExitCode::SUCCESS
        }
        "-v" | "--version" | "version" => {
            println!("Burn {}", VERSION);
            ExitCode::SUCCESS
        }
        "-r" | "--repl" | "repl" => repl::run(),
        "-e" | "--eval" | "eval" => match rest.first() {
            Some(code) => cmd_eval(code),
            None => {
                eprintln!("error: no code given");
                ExitCode::from(2)
            }
        },
        "lsp" | "--lsp" => lsp::run(),
        "fmt" => fmt::cmd(rest),
        "check" => {
            if rest.is_empty() {
                eprintln!("error: no source file given");
                return ExitCode::from(2);
            }
            cmd_check(rest)
        }
        "build" | "-exe" | "--executable" => cmd_build(rest),
        "dump" | "-d" | "--debug" => {
            let (what, file) = match rest {
                [w, f] if w.starts_with("--") => (w.trim_start_matches("--").to_string(), f.clone()),
                [f] => ("hir".to_string(), f.clone()),
                _ => {
                    eprintln!("usage: burn dump [--hir|--bytecode|--asm|--js] <file.bn>");
                    return ExitCode::from(2);
                }
            };
            cmd_dump(Path::new(&file), &what)
        }
        "run" => {
            let mut native_mode = false;
            let mut file = None;
            let mut prog_args = Vec::new();
            for a in rest {
                if file.is_none() && (a == "--native" || a == "-n") {
                    native_mode = true;
                } else if file.is_none() {
                    file = Some(a.clone());
                } else {
                    prog_args.push(a.clone());
                }
            }
            match file {
                Some(f) => cmd_run(Path::new(&f), prog_args, native_mode),
                None => {
                    eprintln!("error: no source file given");
                    ExitCode::from(2)
                }
            }
        }
        _ => {
            if first.starts_with('-') {
                eprintln!("error: unknown option `{}`", first);
                usage();
                return ExitCode::from(2);
            }
            cmd_run(Path::new(first), rest.to_vec(), false)
        }
    }
}

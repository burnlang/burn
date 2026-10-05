mod ast;
mod check;
mod diag;
mod doc;
mod driver;
mod fix;
mod fmt;
mod hir;
mod init;
mod js;
mod lexer;
mod loader;
mod lsp;
mod native;
mod own;
mod parser;
mod project;
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
  burn <file.bn> [args...]            run a program instantly on the Burn VM (bvm)
  burn <file.bvm|file.bvmc> [args...] run a bvm module
  burn init <name> [--lib] [--target <native|js|bvm>]
                                      create a project, named like github.com/you/app
  burn run [file.bn] [--native] [args...]
                                      run a program, or the project's main file; --native compiles it first
  burn build [file.bn] [options]      compile to a standalone executable, or build the project
      -o, --output <path>             output file (default: file name without .bn)
      --target <native|js|bvm>        native executable (default), JavaScript or bvm bytecode
      --emit-asm <path>               also write the generated assembly (x86-64, or bvm text)
      --no-strip                      keep symbols in the executable
      --no-std                        build without the standard runtime (same as `std = false` in burn.toml)
  burn check [files...]               type-check without running (default: the project)
  burn fix [--dry-run] <files...>     apply the compiler's suggested fixes
  burn doc [files...] [-o dir]        generate HTML documentation from Burndoc comments
  burn fmt [-w] [--check] <files...>  format source files
  burn repl                           start the interactive REPL
  burn eval '<code>'                  evaluate code from the command line
  burn lsp                            start the language server (stdio)
  burn sources                        write the standard library and built-in declarations for editors, print the folder
  burn version                        print the version

Legacy flags: -r (repl), -e <code> (eval), -exe <file> [name] (build), -d <file> (show IR)"
    );
}

fn default_output(file: &Path, ext: &str) -> PathBuf {
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into());
    let mut name = stem;
    if !ext.is_empty() {
        name.push('.');
        name.push_str(ext);
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

fn is_bvm_file(file: &Path) -> bool {
    matches!(file.extension().and_then(|e| e.to_str()), Some("bvm") | Some("bvmc") | Some("bar"))
}

fn run_bvm(file: &Path, args: Vec<String>) -> ExitCode {
    match bvm::run_file(file, args) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::from(1)
        }
    }
}

fn cmd_run(file: &Path, args: Vec<String>, native_mode: bool) -> ExitCode {
    if is_bvm_file(file) && !native_mode {
        return run_bvm(file, args);
    }
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
    let code = vm::run_program(&c.program, args);
    ExitCode::from(code as u8)
}

fn current_project() -> Result<project::Project, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    match project::find_root(&cwd) {
        Some(root) => project::load(&root),
        None => Err("no source file given, and there is no burn.toml here or above\n  = help: pass a file like `burn build app.bn`, or create a project with `burn init github.com/you/app`".into()),
    }
}

fn project_main() -> Result<(project::Project, PathBuf), ExitCode> {
    match current_project() {
        Ok(p) => {
            let main = p.main_path();
            Ok((p, main))
        }
        Err(e) => {
            eprintln!("error: {}", e);
            Err(ExitCode::from(2))
        }
    }
}

fn cmd_build(args: &[String]) -> ExitCode {
    let mut file: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut target = "native".to_string();
    let mut explicit_target = false;
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
                explicit_target = true;
            }
            "--emit-asm" | "-S" => {
                i += 1;
                emit_asm = args.get(i).map(PathBuf::from);
            }
            "--no-strip" => strip = false,
            _ if a.starts_with("--target=") => {
                target = a["--target=".len()..].to_string();
                explicit_target = true;
            }
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
            let (p, main) = match project_main() {
                Ok(x) => x,
                Err(c) => return c,
            };
            if p.manifest.kind == project::Kind::Lib && !explicit_target {
                return match compile(&main) {
                    Some(_) => {
                        println!("checked {} (a library is used through imports, so there is nothing to build)", p.manifest.name);
                        ExitCode::SUCCESS
                    }
                    None => ExitCode::from(1),
                };
            }
            if !explicit_target {
                target = p.manifest.target.clone();
            }
            println!("building {} {} ({})", p.manifest.name, p.manifest.version, target);
            if output.is_none() {
                let short = project::short_name(&p.manifest.name);
                let rel = p.manifest.output.clone().unwrap_or_else(|| match target.as_str() {
                    "js" | "javascript" | "node" => format!("build/{}.js", short),
                    "bvm" | "bytecode" => format!("build/{}.bvmc", short),
                    "bar" => format!("build/{}.bar", short),
                    _ => format!("build/{}", short),
                });
                let out = p.root.join(rel);
                let out = std::env::current_dir()
                    .ok()
                    .and_then(|c| out.strip_prefix(&c).ok().map(|r| r.to_path_buf()))
                    .unwrap_or(out);
                if let Some(parent) = out.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                output = Some(out);
            }
            main
        }
    };
    if let Some(parent) = output.as_ref().and_then(|o| o.parent()) {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    if is_bvm_file(&file) {
        return build_bundle(&file, output, emit_asm, strip);
    }
    let c = match compile(&file) {
        Some(c) => c,
        None => return ExitCode::from(1),
    };
    match target.as_str() {
        "js" | "javascript" | "node" => {
            let out = output.unwrap_or_else(|| default_output(&file, "js"));
            if let Err(e) = js::validate(&c.program) {
                eprintln!("error: {}", e);
                return ExitCode::from(1);
            }
            let js = js::generate(&c.program);
            if let Err(e) = std::fs::write(&out, js) {
                eprintln!("error: cannot write {}: {}", out.display(), e);
                return ExitCode::from(1);
            }
            println!("wrote {}", out.display());
            ExitCode::SUCCESS
        }
        "bvm" | "bytecode" => {
            let out = output.unwrap_or_else(|| default_output(&file, "bvmc"));
            let m = match vm::linked(&c.program) {
                Ok((m, _)) => m,
                Err(e) => {
                    eprintln!("error: {}", e);
                    return ExitCode::from(1);
                }
            };
            if let Err(e) = bvm::verify(&m) {
                eprintln!("internal error: the compiler produced an invalid bvm module: {}", e);
                return ExitCode::from(70);
            }
            if let Some(path) = &emit_asm {
                if let Err(e) = std::fs::write(path, bvm::asm::disassemble(&m)) {
                    eprintln!("error: cannot write {}: {}", path.display(), e);
                    return ExitCode::from(1);
                }
            }
            if let Err(e) = std::fs::write(&out, bvm::binary::encode(&m)) {
                eprintln!("error: cannot write {}: {}", out.display(), e);
                return ExitCode::from(1);
            }
            println!("wrote {}", out.display());
            ExitCode::SUCCESS
        }
        "bar" => {
            let out = output.unwrap_or_else(|| default_output(&file, "bar"));
            let p = &c.program;
            let mut a = bvm::archive::Archive::new(&p.name);
            a.add_module(&p.name, vm::module(p));
            for l in &p.libs {
                let parts = match check::libs::library_modules(&l.bytes) {
                    Ok(ms) => ms,
                    Err(e) => {
                        eprintln!("error: {}: {}", l.path.display(), e);
                        return ExitCode::from(1);
                    }
                };
                for (k, m) in parts.into_iter().enumerate() {
                    let name = if !m.name.is_empty() {
                        m.name.clone()
                    } else if k == 0 {
                        l.name.clone()
                    } else {
                        format!("{}.{}", l.name, k)
                    };
                    a.add_module(&name, m);
                }
                if let Ok(lib) = bvm::archive::Archive::decode(&l.bytes) {
                    for (n, d) in lib.resources {
                        a.add_resource(&n, d);
                    }
                }
            }
            if let Err(e) = a.link() {
                eprintln!("error: {}", e);
                return ExitCode::from(1);
            }
            if let Err(e) = std::fs::write(&out, a.encode(true)) {
                eprintln!("error: cannot write {}: {}", out.display(), e);
                return ExitCode::from(1);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&out) {
                    let mut perm = meta.permissions();
                    perm.set_mode(perm.mode() | 0o111);
                    let _ = std::fs::set_permissions(&out, perm);
                }
            }
            println!("wrote {}", out.display());
            ExitCode::SUCCESS
        }
        "native" | "exe" | "asm" => {
            let out = output.unwrap_or_else(|| default_output(&file, ""));
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
            eprintln!("error: unknown target `{}` (expected `native`, `js`, `bvm` or `bar`)", other);
            ExitCode::from(2)
        }
    }
}

fn build_bundle(file: &Path, output: Option<PathBuf>, emit_asm: Option<PathBuf>, strip: bool) -> ExitCode {
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", file.display(), e);
            return ExitCode::from(1);
        }
    };
    let bytes = if bvm::archive::is_archive(&bytes) || bvm::binary::is_binary(&bytes) {
        bytes
    } else {
        match bvm::parse(&bytes) {
            Ok(m) => bvm::binary::encode(&m),
            Err(e) => {
                eprintln!("error: {}: {}", file.display(), e);
                return ExitCode::from(1);
            }
        }
    };
    match bvm::load_bytes(&bytes) {
        Ok((m, host)) => {
            if m.entry.is_none() {
                eprintln!("error: {} has no entry function", file.display());
                return ExitCode::from(1);
            }
            if let Err(e) = bvm::load(&m, &host) {
                eprintln!("error: {}: {}", file.display(), e);
                return ExitCode::from(1);
            }
        }
        Err(e) => {
            eprintln!("error: {}: {}", file.display(), e);
            return ExitCode::from(1);
        }
    }
    let out = output.unwrap_or_else(|| default_output(file, ""));
    let opts = native::BuildOptions {
        output: out.clone(),
        emit_asm,
        strip,
    };
    match native::build_launcher(&bytes, &opts) {
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

fn cmd_eval(code: &str) -> ExitCode {
    match driver::compile_source("<eval>", code) {
        Ok(c) => {
            if !c.warnings.is_empty() {
                driver::report(&c.sm, &c.warnings);
            }
            ExitCode::from(vm::run_program(&c.program, vec![]) as u8)
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
        "bytecode" | "bvm" => print!("{}", bvm::asm::disassemble(&vm::module(&c.program))),
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
  burni <file.bn> [args...]   type-check and run a program on the Burn VM (bvm)
  burni <file.bvm|file.bvmc>  run a bvm module
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
      --target <native|js|bvm>
                              native executable (default), JavaScript or bvm bytecode
      --emit-asm <path>       also write the generated assembly (x86-64, or bvm text)
      --no-strip              keep symbols in the executable
      --no-std                build without the standard runtime
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

fn take_no_std(args: Vec<String>) -> Vec<String> {
    let program = args
        .iter()
        .position(|a| !a.starts_with('-') && (a.ends_with(".bn") || is_bvm_file(Path::new(a))));
    let compiles = matches!(args.first().map(|s| s.as_str()), Some("build" | "check" | "dump")) || tool_name() == "burnc";
    let mut out = Vec::new();
    for (i, a) in args.into_iter().enumerate() {
        if a == "--no-std" && (compiles || program.map(|p| i < p).unwrap_or(true)) {
            driver::set_no_std(true);
        } else {
            out.push(a);
        }
    }
    out
}

fn main() -> ExitCode {
    let args = take_no_std(std::env::args().skip(1).collect());
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
        "sources" => {
            let mut ok = lsp::sources::builtins_file().is_some();
            for s in loader::STDLIB {
                ok &= lsp::sources::std_file(s.name).is_some();
            }
            if !ok {
                eprintln!("error: cannot write the sources to {}", lsp::sources::dir().display());
                return ExitCode::from(1);
            }
            println!("{}", lsp::sources::dir().display());
            ExitCode::SUCCESS
        }
        "fmt" => fmt::cmd(rest),
        "fix" => fix::cmd(rest),
        "doc" => doc::cmd(rest),
        "init" | "new" => init::cmd(rest),
        "check" => {
            if rest.is_empty() {
                return match project_main() {
                    Ok((_, main)) => cmd_check(&[main.display().to_string()]),
                    Err(c) => c,
                };
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
                Some(f) if f == "--" => match project_main() {
                    Ok((_, main)) => cmd_run(&main, prog_args, native_mode),
                    Err(c) => c,
                },
                Some(f) => cmd_run(Path::new(&f), prog_args, native_mode),
                None => match project_main() {
                    Ok((p, main)) => {
                        if p.manifest.kind == project::Kind::Lib {
                            eprintln!("error: {} is a library, so it has no program to run", p.manifest.name);
                            eprintln!("  = help: run its tests with `ash test`, or run a file with `burn run <file.bn>`");
                            return ExitCode::from(2);
                        }
                        cmd_run(&main, prog_args, native_mode)
                    }
                    Err(c) => c,
                },
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

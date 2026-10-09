use bvm::archive::{self, Archive};
use bvm::{asm, binary, verify, LinkOptions, Module};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "bvm - the Burn Virtual Machine

usage:
  bvm <file> [args...]                run a module (.bvm, .bvmc) or an archive (.bar)
  bvm run <file> [args...]            same as above
  bvm asm <file.bvm> [-o out.bvmc]    assemble text into bytecode
  bvm dis <file> [-o out.bvm]         disassemble a module or archive into text
  bvm check <file>...                 verify modules and archives without running them
  bvm link <files>... [-o out] [--lib]
                                      link modules into one; --lib allows unresolved externs
  bvm pack <files>... -o app.bar [options]
                                      bundle modules and resources into a runnable archive
      --name <name>                   application name (default: the output file name)
      --version <version>             application version
      --main <module>                 the module whose entry runs (default: the first module)
      --entry <function>              the function that runs (default: the main module's entry)
      --no-shebang                    leave out the #!/usr/bin/env bvm line
  bvm list <app.bar>                  show an archive's manifest and contents
  bvm unpack <app.bar> [-d dir]       extract an archive's modules and resources
  bvm runtime                         list the runtime functions available to `rt`
  bvm version                         print the version
  bvm help                            show this help";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(|s| s.as_str()) {
        None | Some("help") | Some("-h") | Some("--help") => {
            println!("{}", USAGE);
            ExitCode::SUCCESS
        }
        Some("version") | Some("-v") | Some("--version") => {
            println!(
                "bvm {} (bytecode format {}, archive format {})",
                bvm::VERSION,
                bvm::FORMAT_VERSION,
                archive::VERSION
            );
            ExitCode::SUCCESS
        }
        Some("run") => match rest.first() {
            Some(f) => run(f, rest[1..].to_vec()),
            None => usage_error("bvm run needs a file"),
        },
        Some("asm") => convert(rest, "bvmc", binary::encode),
        Some("dis") => convert(rest, "bvm", |m| asm::disassemble(m).into_bytes()),
        Some("check") => check(rest),
        Some("link") => link(rest),
        Some("pack") => pack(rest),
        Some("list") => list(rest),
        Some("unpack") => unpack(rest),
        Some("runtime") => {
            for f in bvm_runtime::RtFn::all() {
                if !verify::RESERVED_RT.contains(f) {
                    println!("{:<18} {}", bvm::op::rt_name(*f), f.argc());
                }
            }
            ExitCode::SUCCESS
        }
        Some(f) if !f.starts_with('-') => run(f, rest.to_vec()),
        Some(f) => usage_error(&format!("unknown option {}", f)),
    }
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("error: {}\n\n{}", msg, USAGE);
    ExitCode::from(2)
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {}", msg);
    ExitCode::from(1)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("could not read {}: {}", path.display(), e))
}

fn run(file: &str, args: Vec<String>) -> ExitCode {
    match bvm::run_file(Path::new(file), args) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => fail(e),
    }
}

fn output_arg(args: &[String]) -> Result<(Vec<String>, Option<PathBuf>), ExitCode> {
    let mut files = Vec::new();
    let mut out = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-o" {
            i += 1;
            match args.get(i) {
                Some(o) => out = Some(PathBuf::from(o)),
                None => return Err(usage_error("-o needs a file name")),
            }
        } else {
            files.push(args[i].clone());
        }
        i += 1;
    }
    Ok((files, out))
}

fn write_out(out: &Path, bytes: &[u8]) -> ExitCode {
    if out == Path::new("-") {
        use std::io::Write;
        let _ = std::io::stdout().write_all(bytes);
        return ExitCode::SUCCESS;
    }
    match std::fs::write(out, bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(format!("could not write {}: {}", out.display(), e)),
    }
}

fn convert(args: &[String], ext: &str, f: impl Fn(&Module) -> Vec<u8>) -> ExitCode {
    let (files, out) = match output_arg(args) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let [input] = files.as_slice() else {
        return usage_error("expected exactly one input file");
    };
    let m = match bvm::read(Path::new(input)) {
        Ok(m) => m,
        Err(e) => return fail(e),
    };
    if let Err(e) = verify(&m) {
        return fail(format!("{}: {}", input, e));
    }
    let out = out.unwrap_or_else(|| Path::new(input).with_extension(ext));
    write_out(&out, &f(&m))
}

fn check(files: &[String]) -> ExitCode {
    if files.is_empty() {
        return usage_error("bvm check needs at least one file");
    }
    let mut failed = false;
    for f in files {
        if let Err(e) = bvm::read(Path::new(f)).and_then(|m| verify(&m).map_err(|e| format!("{}: {}", f, e))) {
            eprintln!("error: {}", e);
            failed = true;
        }
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn modules_of(path: &Path) -> Result<Vec<(String, Module)>, String> {
    let bytes = read_bytes(path)?;
    if archive::is_archive(&bytes) {
        let a = Archive::decode(&bytes).map_err(|e| format!("{}: {}", path.display(), e))?;
        return Ok(a.modules);
    }
    let m = bvm::parse(&bytes).map_err(|e| format!("{}: {}", path.display(), e))?;
    let name = if m.name.is_empty() {
        path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    } else {
        m.name.clone()
    };
    Ok(vec![(name, m)])
}

fn link(args: &[String]) -> ExitCode {
    let opts = LinkOptions {
        allow_unresolved: args.iter().any(|a| a == "--lib"),
        ..LinkOptions::default()
    };
    let args: Vec<String> = args.iter().filter(|a| *a != "--lib").cloned().collect();
    let (files, out) = match output_arg(&args) {
        Ok(x) => x,
        Err(c) => return c,
    };
    if files.is_empty() {
        return usage_error("bvm link needs modules to link");
    }
    let mut modules = Vec::new();
    for f in &files {
        match modules_of(Path::new(f)) {
            Ok(ms) => modules.extend(ms.into_iter().map(|(_, m)| m)),
            Err(e) => return fail(e),
        }
    }
    let m = match bvm::link_with(&modules, &opts) {
        Ok(m) => m,
        Err(e) => return fail(e),
    };
    let out = out.unwrap_or_else(|| Path::new(&files[0]).with_extension("linked.bvmc"));
    let bytes = if out.extension().and_then(|e| e.to_str()) == Some("bvm") {
        asm::disassemble(&m).into_bytes()
    } else {
        binary::encode(&m)
    };
    write_out(&out, &bytes)
}

fn is_module_file(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()), Some("bvm") | Some("bvmc") | Some("bar"))
}

#[derive(Default)]
struct PackArgs {
    files: Vec<String>,
    out: Option<PathBuf>,
    name: Option<String>,
    version: String,
    main: Option<String>,
    entry: Option<String>,
    no_shebang: bool,
}

fn pack_args(args: &[String]) -> Result<PackArgs, ExitCode> {
    let mut p = PackArgs::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |what: &str| it.next().cloned().ok_or_else(|| usage_error(&format!("{} needs a value", what)));
        match a.as_str() {
            "-o" => p.out = Some(PathBuf::from(value("-o")?)),
            "--name" => p.name = Some(value("--name")?),
            "--version" => p.version = value("--version")?,
            "--main" => p.main = Some(value("--main")?),
            "--entry" => p.entry = Some(value("--entry")?),
            "--no-shebang" => p.no_shebang = true,
            f if f.starts_with("--") => return Err(usage_error(&format!("unknown option {}", f))),
            f => p.files.push(f.to_string()),
        }
    }
    Ok(p)
}

fn pack(args: &[String]) -> ExitCode {
    let p = match pack_args(args) {
        Ok(p) => p,
        Err(c) => return c,
    };
    let Some(out) = p.out else {
        return usage_error("bvm pack needs -o <app.bar>");
    };
    if p.files.is_empty() {
        return usage_error("bvm pack needs at least one module");
    }
    let app = p
        .name
        .unwrap_or_else(|| out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    let mut a = Archive::new(&app);
    a.manifest.version = p.version;
    for f in &p.files {
        let path = Path::new(f);
        if is_module_file(path) {
            match modules_of(path) {
                Ok(ms) => ms.into_iter().for_each(|(n, m)| a.add_module(&n, m)),
                Err(e) => return fail(e),
            }
        } else {
            match read_bytes(path) {
                Ok(data) => {
                    let rname = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    a.add_resource(&rname, data);
                }
                Err(e) => return fail(e),
            }
        }
    }
    if let Some(mn) = p.main {
        if !a.modules.iter().any(|(n, _)| *n == mn) {
            return fail(format!("there is no module called {} to use as the main module", mn));
        }
        a.manifest.main = mn;
    }
    a.manifest.entry = p.entry;
    if let Err(e) = a.link() {
        return fail(format!("the archive does not link: {}", e));
    }
    let code = write_out(&out, &a.encode(!p.no_shebang));
    #[cfg(unix)]
    if !p.no_shebang {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&out) {
            let mut perm = meta.permissions();
            perm.set_mode(perm.mode() | 0o111);
            let _ = std::fs::set_permissions(&out, perm);
        }
    }
    code
}

fn read_archive(path: &str) -> Result<Archive, String> {
    let bytes = read_bytes(Path::new(path))?;
    Archive::decode(&bytes).map_err(|e| format!("{}: {}", path, e))
}

fn list(args: &[String]) -> ExitCode {
    let [file] = args else {
        return usage_error("bvm list needs one archive");
    };
    let a = match read_archive(file) {
        Ok(a) => a,
        Err(e) => return fail(e),
    };
    print!("{}", a.manifest.to_text());
    println!();
    for (n, m) in &a.modules {
        let externs = m.funcs.iter().filter(|f| f.external).count();
        let extra = if externs > 0 { format!(", {} external", externs) } else { String::new() };
        println!(
            "module    {:<24} {} functions, {} instructions{}",
            n,
            m.funcs.len() - externs,
            m.code_size(),
            extra
        );
    }
    for (n, d) in &a.resources {
        println!("resource  {:<24} {} bytes", n, d.len());
    }
    ExitCode::SUCCESS
}

fn unpack(args: &[String]) -> ExitCode {
    let mut file = None;
    let mut dir = PathBuf::from(".");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "-d" {
            match it.next() {
                Some(d) => dir = PathBuf::from(d),
                None => return usage_error("-d needs a directory"),
            }
        } else if file.is_none() {
            file = Some(a.clone());
        } else {
            return usage_error(&format!("unexpected argument {}", a));
        }
    }
    let Some(file) = file else {
        return usage_error("bvm unpack needs an archive");
    };
    let a = match read_archive(&file) {
        Ok(a) => a,
        Err(e) => return fail(e),
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return fail(format!("could not create {}: {}", dir.display(), e));
    }
    let safe = |n: &str| {
        Path::new(n)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unnamed".into())
    };
    let mut writes: Vec<(PathBuf, Vec<u8>)> = vec![(dir.join("MANIFEST"), a.manifest.to_text().into_bytes())];
    for (n, m) in &a.modules {
        writes.push((dir.join(format!("{}.bvmc", safe(n))), binary::encode(m)));
    }
    for (n, d) in &a.resources {
        writes.push((dir.join(safe(n)), d.clone()));
    }
    for (p, d) in writes {
        if let Err(e) = std::fs::write(&p, d) {
            return fail(format!("could not write {}: {}", p.display(), e));
        }
        println!("{}", p.display());
    }
    ExitCode::SUCCESS
}

use crate::project::{self, Kind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: burn init <name> [dir] [--lib] [--target native|js|bvm] [--no-git]

  <name>     where the code lives, for example github.com/you/hello
  [dir]      the directory to create it in (default: the last part of the name)
  --lib      create a library instead of an app
  --target   what `burn build` produces: native (default), js or bvm
  --no-git   do not create a git repository";

fn fail(msg: &str, help: &str) -> ExitCode {
    eprintln!("error: {}", msg);
    if !help.is_empty() {
        eprintln!("  = help: {}", help);
    }
    ExitCode::from(2)
}

fn build_command(short: &str, target: &str, main: &str) -> String {
    match target {
        "js" => format!("burnc {} --target js -o build/{}.js", main, short),
        "bvm" => format!("burnc {} --target bvm -o build/{}.bvmc", main, short),
        _ => format!("burnc {} -o build/{}", main, short),
    }
}

fn start_command(short: &str, target: &str) -> String {
    match target {
        "js" => format!("node build/{}.js", short),
        "bvm" => format!("bvm build/{}.bvmc", short),
        _ => format!("./build/{}", short),
    }
}

pub fn manifest(name: &str, kind: Kind, target: &str) -> String {
    let short = project::short_name(name);
    let (kind_s, main) = match kind {
        Kind::App => ("app", "src/main.bn"),
        Kind::Lib => ("lib", "src/lib.bn"),
    };
    let scripts = match kind {
        Kind::App => format!(
            "dev = \"burni {main}\"\nrelease = \"{}\"\nstart = \"{}\"\n",
            build_command(short, target, main),
            start_command(short, target)
        ),
        Kind::Lib => "test = \"burni tests/main.bn\"\n".to_string(),
    };
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nkind = \"{kind_s}\"\ntarget = \"{target}\"\nmain = \"{main}\"\n\n[dependencies]\n\n[scripts]\n{scripts}"
    )
}

fn files(name: &str, kind: Kind, target: &str) -> Vec<(&'static str, String)> {
    let short = project::short_name(name);
    let mut out = vec![("burn.toml", manifest(name, kind, target)), (".gitignore", "build/\n".to_string())];
    match kind {
        Kind::App => {
            out.push(("src/main.bn", format!("fun main() {{\n    print(\"Hello from {}!\")\n}}\n", short)));
            out.push((
                "README.md",
                format!("# {short}\n\nA Burn app.\n\n```sh\nburn run      # run it\nburn build    # build it into build/\n```\n"),
            ));
        }
        Kind::Lib => {
            out.push((
                "src/lib.bn",
                "/**\n * Returns a friendly greeting.\n *\n * @param name who to greet\n * @return the greeting\n * @since 0.1.0\n * @example\n *     greet(\"Burn\")  // \"Hello, Burn!\"\n */\npub fun greet(name: string): string {\n    return \"Hello, ${name}!\"\n}\n".to_string(),
            ));
            out.push((
                "tests/main.bn",
                format!("import \"{name}\"\n\nassert(greet(\"Burn\") == \"Hello, Burn!\")\nprint(\"all tests passed\")\n"),
            ));
            out.push((
                "README.md",
                format!("# {short}\n\nA Burn library.\n\n```sh\nash install {name}\n```\n\n```burn\nimport \"{name}\"\n\nprint(greet(\"Burn\"))\n```\n"),
            ));
        }
    }
    out
}

fn inside_git(dir: &Path) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn cmd(args: &[String]) -> ExitCode {
    let mut name: Option<String> = None;
    let mut dir: Option<PathBuf> = None;
    let mut kind = Kind::App;
    let mut target = "native".to_string();
    let mut git = true;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "-h" | "--help" => {
                println!("{}", USAGE);
                return ExitCode::SUCCESS;
            }
            "--lib" => kind = Kind::Lib,
            "--app" => kind = Kind::App,
            "--no-git" => git = false,
            "--target" | "-t" => {
                i += 1;
                target = args.get(i).cloned().unwrap_or_default();
            }
            _ if a.starts_with("--target=") => target = a["--target=".len()..].to_string(),
            _ if a.starts_with('-') => return fail(&format!("unknown option `{}`", a), USAGE),
            _ if name.is_none() => name = Some(a.to_string()),
            _ if dir.is_none() => dir = Some(PathBuf::from(a)),
            _ => return fail(&format!("unexpected argument `{}`", a), ""),
        }
        i += 1;
    }
    let target = match target.as_str() {
        "native" | "exe" => "native",
        "js" | "javascript" | "node" => "js",
        "bvm" | "bytecode" => "bvm",
        t => return fail(&format!("unknown target `{}`", t), "choose `native`, `js` or `bvm`"),
    };
    let name = match name {
        Some(n) => n,
        None => {
            eprintln!("{}", USAGE);
            return ExitCode::from(2);
        }
    };
    if !project::valid_name(&name) {
        let short = name.rsplit('/').next().unwrap_or(&name).to_lowercase();
        let short = if short.is_empty() { "hello".to_string() } else { short };
        return fail(
            &format!("`{}` is not a package name", name),
            &format!("a package is named after where its code lives, like `github.com/you/{}`", short),
        );
    }
    let short = project::short_name(&name).to_string();
    let dir = dir.unwrap_or_else(|| PathBuf::from(&short));
    if dir.join(project::MANIFEST).exists() {
        return fail(
            &format!("{} already has a burn.toml", dir.display()),
            "pick another directory, or edit the existing burn.toml",
        );
    }
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return fail(&format!("cannot create {}: {}", dir.display(), e), "");
    }
    let mut skipped = Vec::new();
    for (rel, content) in files(&name, kind, target) {
        let path = dir.join(rel);
        if path.exists() {
            skipped.push(rel);
            continue;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, content) {
            return fail(&format!("cannot write {}: {}", path.display(), e), "");
        }
    }
    if git && !inside_git(&dir) {
        let _ = std::process::Command::new("git").args(["init", "-q"]).current_dir(&dir).status();
    }
    let what = match kind {
        Kind::App => format!("{} app", target),
        Kind::Lib => "library".to_string(),
    };
    println!("created {} in {} ({})", name, dir.display(), what);
    for s in &skipped {
        println!("  kept the existing {}", s);
    }
    println!();
    if dir != Path::new(".") {
        println!("  cd {}", dir.display());
    }
    match kind {
        Kind::App => {
            println!("  burn run             run it");
            println!("  burn build           build it into build/");
        }
        Kind::Lib => println!("  ash test             run tests/main.bn"),
    }
    println!("  ash install <name>   add a package");
    ExitCode::SUCCESS
}

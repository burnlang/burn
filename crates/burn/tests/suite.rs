use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn burn() -> Command {
    Command::new(env!("CARGO_BIN_EXE_burn"))
}

fn output(cmd: &mut Command) -> (String, i32) {
    let out = cmd.output().expect("failed to run command");
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (s, out.status.code().unwrap_or(-1))
}

fn cases() -> Vec<(PathBuf, String)> {
    let dir = root().join("tests/cases");
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map(|x| x == "bn").unwrap_or(false) {
            let expected = std::fs::read_to_string(p.with_extension("out")).unwrap_or_else(|_| panic!("missing .out for {}", p.display()));
            out.push((p, expected));
        }
    }
    out.sort();
    out
}

fn has_node() -> bool {
    Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn native_supported() -> bool {
    cfg!(all(target_arch = "x86_64", any(target_os = "linux", target_os = "macos")))
}

#[test]
fn vm_matches_expected_output() {
    let root = root();
    let mut failures = Vec::new();
    for (file, expected) in cases() {
        let rel = file.strip_prefix(&root).unwrap();
        let (out, _) = output(burn().current_dir(&root).arg(rel));
        if out != expected {
            failures.push(format!("{}:\n--- expected\n{}\n--- got\n{}", rel.display(), expected, out));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn native_matches_vm() {
    if !native_supported() {
        return;
    }
    let root = root();
    let tmp = std::env::temp_dir().join(format!("burn-suite-native-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut failures = Vec::new();
    for (file, expected) in cases() {
        let rel = file.strip_prefix(&root).unwrap();
        let exe = tmp.join(file.file_stem().unwrap());
        let (build_out, code) = output(burn().current_dir(&root).arg("build").arg(rel).arg("-o").arg(&exe));
        if code != 0 {
            failures.push(format!("{}: build failed\n{}", rel.display(), build_out));
            continue;
        }
        for threshold in [None, Some("16384")] {
            let mut cmd = Command::new(&exe);
            cmd.current_dir(&root);
            if let Some(t) = threshold {
                cmd.env("BURN_GC_THRESHOLD", t);
            }
            let (out, _) = output(&mut cmd);
            if out != expected {
                failures.push(format!(
                    "{} (gc threshold {:?}):\n--- expected\n{}\n--- got\n{}",
                    rel.display(),
                    threshold,
                    expected,
                    out
                ));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn js_matches_vm() {
    if !has_node() {
        return;
    }
    let root = root();
    let tmp = std::env::temp_dir().join(format!("burn-suite-js-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut failures = Vec::new();
    for (file, expected) in cases() {
        let rel = file.strip_prefix(&root).unwrap();
        let js = tmp.join(format!("{}.js", file.file_stem().unwrap().to_string_lossy()));
        let (build_out, code) = output(burn().current_dir(&root).arg("build").arg(rel).arg("--target").arg("js").arg("-o").arg(&js));
        if code != 0 {
            failures.push(format!("{}: js build failed\n{}", rel.display(), build_out));
            continue;
        }
        let (out, _) = output(Command::new("node").current_dir(&root).arg(&js));
        if out != expected {
            failures.push(format!("{}:\n--- expected\n{}\n--- got\n{}", rel.display(), expected, out));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn compile_errors_point_at_the_right_line() {
    let root = root();
    let dir = root.join("tests/errors");
    let mut failures = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map(|x| x != "bn").unwrap_or(true) {
            continue;
        }
        let spec = std::fs::read_to_string(p.with_extension("err")).unwrap();
        let spec = spec.trim();
        let (pos, msg) = spec.split_once(' ').unwrap();
        let rel = p.strip_prefix(&root).unwrap();
        let (out, code) = output(burn().current_dir(&root).arg("check").arg(rel));
        let want_pos = format!("{}:{}", p.file_name().unwrap().to_string_lossy(), pos);
        if code == 0 || !out.contains(&want_pos) || !out.contains(msg) {
            failures.push(format!("{}: expected `{}` at {}\n{}", rel.display(), msg, want_pos, out));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn eval_and_exit_codes() {
    let (out, code) = output(burn().args(["eval", "print(1 + 2)"]));
    assert_eq!(out, "3\n");
    assert_eq!(code, 0);
    let (_, code) = output(burn().args(["eval", "exit(7)"]));
    assert_eq!(code, 7);
    let (out, code) = output(burn().args(["eval", "print(1 / (2 - 2))"]));
    assert_eq!(code, 1);
    assert!(out.contains("division by zero"), "{}", out);
}

#[test]
fn repl_keeps_state_between_inputs() {
    use std::io::Write;
    let mut child = burn()
        .arg("repl")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"var x = 20\nfun twice(n: int): int {\n    return n * 2\n}\ntwice(x) + 2\nx = x + 1\nprint(x)\nundefinedThing\nprint(\"still alive\")\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stdout.contains("42"), "{}", stdout);
    assert!(stdout.contains("21"), "{}", stdout);
    assert!(stdout.contains("still alive"), "{}", stdout);
    assert!(stderr.contains("cannot find `undefinedThing`"), "{}", stderr);
}

#[test]
fn formatter_is_idempotent() {
    let root = root();
    for (file, _) in cases() {
        let src = std::fs::read_to_string(&file).unwrap();
        let (once, _) = output(burn().current_dir(&root).arg("fmt").arg(&file));
        let tmp = std::env::temp_dir().join(format!("burn-fmt-{}.bn", std::process::id()));
        std::fs::write(&tmp, &once).unwrap();
        let (twice, _) = output(burn().arg("fmt").arg(&tmp));
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(once, twice, "formatting {} is not idempotent", file.display());
        let _ = src;
    }
}

#[test]
fn repository_sources_are_formatted() {
    let root = root();
    let mut files = Vec::new();
    for dir in ["tests/cases", "examples", "lib/std", "tests/modules", "compiler"] {
        for e in std::fs::read_dir(root.join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().map(|x| x == "bn").unwrap_or(false) {
                files.push(p);
            }
        }
    }
    let (out, code) = output(burn().arg("fmt").arg("--check").args(&files));
    assert_eq!(code, 0, "{}", out);
}

#[test]
fn repository_sources_import_the_standard_modules_they_use() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map(|x| x == "bn").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    let root = root();
    let mut files = Vec::new();
    for dir in ["tests", "examples", "lib/std", "tools", "compiler"] {
        walk(&root.join(dir), &mut files);
    }
    let mut failures = Vec::new();
    for f in files
        .iter()
        .filter(|f| !f.starts_with(root.join("tests/errors")) && !f.starts_with(root.join("tests/fix")))
    {
        let (out, _) = output(burn().current_dir(&root).arg("check").arg(f));
        if out.contains("is in the standard library module") {
            failures.push(format!("{}\n{}", f.display(), out));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn compiler_written_in_burn_matches_the_compiler() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map(|x| x == "bn").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    let root = root();
    let mut files = Vec::new();
    for dir in ["tests", "examples", "lib/std", "tools", "compiler"] {
        walk(&root.join(dir), &mut files);
    }
    files.sort();
    assert!(files.len() > 100, "only {} files", files.len());
    let dir = temp_dir("selfhost");
    let exe = dir.join("dump");
    let (built, code) = output(burn().current_dir(&root).args(["build", "compiler/dump.bn", "-o"]).arg(&exe));
    assert_eq!(code, 0, "{}", built);
    for stage in ["--tokens", "--ast", "--diagnostics"] {
        let (want, code) = output(burn().current_dir(&root).args(["dump", stage]).args(&files));
        assert_eq!(code, 0, "{}", want);
        let mut bvm = burn();
        bvm.current_dir(&root).arg("compiler/dump.bn").arg(stage).args(&files);
        let mut native = Command::new(&exe);
        native.current_dir(&root).arg(stage).args(&files);
        for mut run in [bvm, native] {
            let (got, code) = output(&mut run);
            assert_eq!(code, 0, "{}", got);
            if got != want {
                let line = got.lines().zip(want.lines()).position(|(a, b)| a != b).unwrap_or(0);
                let show = |s: &str| s.lines().skip(line.saturating_sub(3)).take(8).collect::<Vec<_>>().join("\n");
                panic!(
                    "`burn dump {}` and the compiler written in Burn differ at line {}:\n--- burn\n{}\n--- rust\n{}",
                    stage,
                    line + 1,
                    show(&got),
                    show(&want)
                );
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn projects_use_a_standard_layout() {
    let dir = temp_dir("layout");
    let (out, code) = output(burn().current_dir(&dir).args(["init", "github.com/ada/app", "--no-git"]));
    assert_eq!(code, 0, "{}", out);
    let app = dir.join("app");
    let write = |rel: &str, text: &str| {
        let p = app.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    write(
        "src/net/http/mod.bn",
        "import \"@/util/text\"\n\npub fun fetch(url: string): string {\n    return shout(\"GET \" + url)\n}\n",
    );
    write("src/util/text.bn", "pub fun shout(s: string): string {\n    return upper(s)\n}\n");
    write(
        "src/net/dns.bn",
        "import \"@/net/http\"\n\npub fun lookup(host: string): string {\n    return fetch(\"dns://\" + host)\n}\n",
    );
    write(
        "src/main.bn",
        "import \"@/net/http\"\nimport \"github.com/ada/app/net/dns\"\nimport \"github.com/ada/app/src/util/text\"\n\nfun main() {\n    print(fetch(\"a\"), lookup(\"b\"), shout(\"c\"))\n}\n",
    );
    write(
        "src/bin/tool.bn",
        "import \"@/util/text\"\nimport \"std/process\"\n\nfun main() {\n    print(shout(\"tool\"), args())\n}\n",
    );
    write("src/bin/server/main.bn", "fun main() {\n    print(\"server\")\n}\n");
    write("examples/hello.bn", "import \"@/net/http\"\n\nprint(fetch(\"example\"))\n");
    write(
        "tests/text.bn",
        "import \"std/testing\"\nimport \"@/util/text\"\n\nassert(shout(\"a\") == \"A\")\n",
    );
    let run = |args: &[&str]| output(burn().current_dir(&app).args(args));
    assert_eq!(run(&["run"]), ("GET A GET DNS://B C\n".to_string(), 0));
    assert_eq!(run(&["run", "--bin", "tool", "--", "x"]), ("TOOL [\"x\"]\n".to_string(), 0));
    assert_eq!(run(&["run", "--bin", "server"]), ("server\n".to_string(), 0));
    assert_eq!(run(&["run", "--example", "hello"]), ("GET EXAMPLE\n".to_string(), 0));
    let (out, code) = run(&["run", "--bin", "nope"]);
    assert!(code != 0 && out.contains("it has: server, tool"), "{}", out);
    assert_eq!(run(&["check"]).1, 0);
    let (out, code) = run(&["build", "--target", "bvm"]);
    assert_eq!(code, 0, "{}", out);
    for f in ["build/app.bvmc", "build/server.bvmc", "build/tool.bvmc"] {
        assert!(app.join(f).is_file(), "{} missing after:\n{}", f, out);
    }
    let (out, code) = run(&["test"]);
    assert!(
        code == 0 && out.contains("test tests/text.bn ... ok") && out.contains("1 passed, 0 failed"),
        "{}",
        out
    );
    write("tests/broken.bn", "import \"std/testing\"\n\nassert(1 == 2, \"one is not two\")\n");
    let (out, code) = run(&["test"]);
    assert!(
        code != 0 && out.contains("test tests/broken.bn ... FAILED") && out.contains("one is not two"),
        "{}",
        out
    );
    write("src/bad.bn", "import \"@/missing/thing\"\n");
    let (out, code) = run(&["check", "src/bad.bn"]);
    assert!(code != 0 && out.contains("cannot find `@/missing/thing`"), "{}", out);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn workspaces_build_every_member() {
    let dir = temp_dir("workspace");
    let (out, code) = output(burn().current_dir(&dir).args(["init", "github.com/ada/game", "--workspace", "--no-git"]));
    assert_eq!(code, 0, "{}", out);
    let ws = dir.join("game");
    let run = |cwd: &Path, args: &[&str]| output(burn().current_dir(cwd).args(args));
    let toml = std::fs::read_to_string(ws.join("burn.toml")).unwrap();
    assert!(toml.contains("members = [\"common\", \"native\", \"js\", \"bvm\"]"), "{}", toml);
    let (out, code) = run(&ws, &["build"]);
    assert_eq!(code, 0, "{}", out);
    for f in ["native/build/native", "js/build/js.js", "bvm/build/bvm.bvmc"] {
        assert!(ws.join(f).is_file(), "{} missing after:\n{}", f, out);
    }
    for t in ["native", "js", "bvm"] {
        assert_eq!(run(&ws, &["run", "-p", t]), (format!("Hello from {}!\n", t), 0));
    }
    let (out, code) = run(&ws, &["run"]);
    assert!(code != 0 && out.contains("pick one with `-p` (native, js, bvm)"), "{}", out);
    assert_eq!(run(&ws.join("native"), &["run"]), ("Hello from native!\n".to_string(), 0));
    let (out, code) = run(&ws, &["run", "-p", "nope"]);
    assert!(code != 0 && out.contains("has no member `nope`"), "{}", out);
    let (out, code) = run(&ws, &["init", "tools", "--lib", "--no-git"]);
    assert!(code == 0 && out.contains("added \"tools\" to the workspace's members"), "{}", out);
    assert!(std::fs::read_to_string(ws.join("tools/burn.toml"))
        .unwrap()
        .contains("name = \"github.com/ada/game/tools\""));
    std::fs::write(
        ws.join("native/src/main.bn"),
        "import \"github.com/ada/game/common\"\nimport \"github.com/ada/game/tools\"\n\nfun main() {\n    print(greeting(\"native\"), greet(\"tools\"))\n}\n",
    )
    .unwrap();
    assert_eq!(run(&ws, &["run", "-p", "native"]), ("Hello from native! Hello, tools!\n".to_string(), 0));
    let (out, code) = run(&ws, &["test"]);
    assert!(
        code == 0 && out.contains("common/tests/main.bn ... ok") && out.contains("tools/tests/main.bn ... ok") && out.contains("2 passed"),
        "{}",
        out
    );
    assert_eq!(run(&ws, &["check"]).1, 0);
    let (out, code) = run(&ws, &["check", "-p", "native"]);
    assert_eq!(code, 0, "{}", out);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn burnfmt_written_in_burn_matches_the_builtin_formatter() {
    let root = root();
    let tool = root.join("tools/burnfmt/burnfmt.bn");
    let mut files = vec![tool.clone()];
    for dir in ["tests/cases", "examples", "lib/std", "compiler"] {
        for e in std::fs::read_dir(root.join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().map(|x| x == "bn").unwrap_or(false) {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut failures = Vec::new();
    for f in &files {
        let (ours, _) = output(burn().arg(&tool).arg(f));
        let (builtin, _) = output(burn().arg("fmt").arg(f));
        if ours != builtin {
            failures.push(f.display().to_string());
        }
    }
    assert!(failures.is_empty(), "burnfmt differs from burn fmt on: {:?}", failures);
    let messy = "fun   f( a:int ){\nreturn a*2}\n";
    let mut child = burn()
        .arg(&tool)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), messy.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "fun f(a: int) {\n    return a * 2\n}\n");
}

#[test]
fn toolchain_names_select_the_right_mode() {
    let dir = std::env::temp_dir().join(format!("burn-toolchain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = std::path::PathBuf::from(env!("CARGO_BIN_EXE_burn"));
    let burni = dir.join("burni");
    let burnc = dir.join("burnc");
    std::fs::copy(&exe, &burni).unwrap();
    std::fs::copy(&exe, &burnc).unwrap();
    let (out, code) = output(Command::new(&burni).args(["-e", "print(6 * 7)"]));
    assert_eq!((out.as_str(), code), ("42\n", 0));
    let (out, _) = output(Command::new(&burnc).arg("--version"));
    assert!(out.starts_with("burnc "), "{}", out);
    let (out, code) = output(Command::new(&burnc).arg("--check").arg(root().join("examples/fib.bn")));
    assert_eq!(code, 0, "{}", out);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bvm_modules_round_trip_and_run() {
    let root = root();
    let dir = std::env::temp_dir().join(format!("burn-bvm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();
    for (file, expected) in cases() {
        let rel = file.strip_prefix(&root).unwrap();
        let stem = file.file_stem().unwrap().to_string_lossy().into_owned();
        let bin = dir.join(format!("{}.bvmc", stem));
        let text = dir.join(format!("{}.bvm", stem));
        let (out, code) = output(
            burn()
                .current_dir(&root)
                .args(["build", "--target", "bvm", "-o"])
                .arg(&bin)
                .arg("-S")
                .arg(&text)
                .arg(rel),
        );
        assert_eq!(code, 0, "{}", out);
        let bytes = std::fs::read(&bin).unwrap();
        let module = bvm::binary::decode(&bytes).unwrap();
        let src = std::fs::read_to_string(&text).unwrap();
        let assembled = bvm::asm::assemble(&src).unwrap_or_else(|e| panic!("{}: {}", text.display(), e));
        if assembled != module || bvm::binary::encode(&assembled) != bytes {
            failures.push(format!("{}: the text form does not assemble back to the same module", rel.display()));
        }
        for m in [&bin, &text] {
            let (out, _) = output(burn().current_dir(&root).arg(m));
            if out != expected {
                failures.push(format!("{}:\n--- expected\n{}\n--- got\n{}", m.display(), expected, out));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("burn-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn stdout_of(cmd: &mut Command) -> (String, String, i32) {
    let out = cmd.output().expect("failed to run command");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn mixins_rewrite_bytecode_and_are_rejected_for_native_code() {
    let root = root();
    let expected = std::fs::read_to_string(root.join("tests/mixins/mixins.out")).unwrap();
    let (out, err, code) = stdout_of(burn().current_dir(&root).arg("tests/mixins/mixins.bn"));
    assert_eq!(code, 0, "{}", err);
    assert_eq!(out, expected);
    assert!(err.contains("`oldGreet` is deprecated: use greet"), "{}", err);
    let dir = temp_dir("mixins");
    if native_supported() {
        let (_, err, code) = stdout_of(burn().current_dir(&root).args(["build", "tests/mixins/mixins.bn", "-o"]).arg(dir.join("m")));
        assert_ne!(code, 0);
        assert!(err.contains("compiled to native code; mixins can only change bvm bytecode"), "{}", err);
    }
    let (_, err, code) = stdout_of(
        burn()
            .current_dir(&root)
            .args(["build", "--target", "js", "tests/mixins/mixins.bn", "-o"])
            .arg(dir.join("m.js")),
    );
    assert_ne!(code, 0);
    assert!(err.contains("mixin"), "{}", err);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bytecode_libraries_run_on_bvm_in_archives_and_in_native_executables() {
    let root = root();
    let expected = std::fs::read_to_string(root.join("tests/libs/app.out")).unwrap();
    let dir = temp_dir("libs");
    std::fs::copy(root.join("tests/libs/app.bn"), dir.join("app.bn")).unwrap();
    let (_, err, code) = stdout_of(
        burn()
            .args(["build", "--target", "bvm"])
            .arg(root.join("tests/libs/geometry.bn"))
            .arg("-o")
            .arg(dir.join("geometry.bvmc")),
    );
    assert_eq!(code, 0, "{}", err);
    let app = dir.join("app.bn");
    let (out, err, _) = stdout_of(burn().arg(&app));
    assert_eq!(out, expected, "bvm: {}", err);
    let (_, err, code) = stdout_of(burn().args(["build", "--target", "bar"]).arg(&app).arg("-o").arg(dir.join("app.bar")));
    assert_eq!(code, 0, "{}", err);
    let (out, err, _) = stdout_of(burn().arg(dir.join("app.bar")));
    assert_eq!(out, expected, "bar: {}", err);
    let (_, err, code) = stdout_of(burn().args(["build", "--target", "bvm"]).arg(&app).arg("-o").arg(dir.join("static.bvmc")));
    assert_eq!(code, 0, "{}", err);
    let (out, err, _) = stdout_of(burn().arg(dir.join("static.bvmc")));
    assert_eq!(out, expected, "static bvmc: {}", err);
    if native_supported() {
        let (out, err, _) = stdout_of(burn().args(["run", "--native"]).arg(&app));
        assert_eq!(out, expected, "native: {}", err);
        let exe = dir.join("bundled");
        let (_, err, code) = stdout_of(burn().arg("build").arg(dir.join("app.bar")).arg("-o").arg(&exe));
        assert_eq!(code, 0, "{}", err);
        let (out, err, _) = stdout_of(&mut Command::new(&exe));
        assert_eq!(out, expected, "bundled bar: {}", err);
        let (out, _, _) = stdout_of(Command::new(&exe).env("BURN_GC_THRESHOLD", "4096"));
        assert_eq!(out, expected, "bundled bar with a small GC threshold");
    }
    let (_, err, code) = stdout_of(burn().args(["build", "--target", "js"]).arg(&app).arg("-o").arg(dir.join("app.js")));
    assert_ne!(code, 0);
    assert!(err.contains("bytecode library"), "{}", err);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fix_applies_the_compiler_suggestions() {
    let root = root();
    let tmp = std::env::temp_dir().join(format!("burn-fix-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let file = tmp.join("input.bn");
    std::fs::copy(root.join("tests/fix/input.bn"), &file).unwrap();
    let (out, code) = output(burn().arg("fix").arg(&file));
    assert_eq!(code, 0, "{}", out);
    assert!(out.contains("fixed 8 problems"), "{}", out);
    let got = std::fs::read_to_string(&file).unwrap();
    let want = std::fs::read_to_string(root.join("tests/fix/expected.bn")).unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(got, want);
}

#[test]
fn doc_generates_pages_for_programs_the_standard_library_and_builtins() {
    let root = root();
    let out = std::env::temp_dir().join(format!("burn-doc-{}", std::process::id()));
    let (log, code) = output(burn().current_dir(&root).arg("doc").arg("examples/zoo.bn").arg("-o").arg(&out));
    assert_eq!(code, 0, "{}", log);
    let read = |p: &str| std::fs::read_to_string(out.join(p)).unwrap_or_else(|_| panic!("missing {}", p));
    let animal = read("t-zoo.Animal.html");
    assert!(animal.contains("An animal living in the zoo."));
    assert!(animal.contains("Extended by"));
    assert!(animal.contains("the animal&#x27;s name") || animal.contains("the animal's name"));
    let module = read("m-zoo.html");
    assert!(module.contains("Deprecated"));
    assert!(read("t-std-date.Date.html").contains("A calendar date"));
    let builtins = read("builtins.html");
    assert!(builtins.contains("println") && builtins.contains("id=\"sqrt\""));
    assert!(read("search-index.js").contains("\"Animal.describe\""));
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn optimizing_everything_while_running_changes_no_output() {
    let root = root();
    let mut failures = Vec::new();
    for (file, expected) in cases() {
        let rel = file.strip_prefix(&root).unwrap();
        let (out, _) = output(burn().current_dir(&root).env("BVM_HOT", "1").arg(rel));
        if out != expected {
            failures.push(format!("{}:\n--- expected\n{}\n--- got\n{}", rel.display(), expected, out));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn hot_code_is_optimized_while_running() {
    let tmp = std::env::temp_dir().join(format!("burn-hot-{}.bn", std::process::id()));
    std::fs::write(
        &tmp,
        "def struct P(x: int) {\n    fun get(): int {\n        return x\n    }\n}\nfun sq(n: int): int {\n    return n * n\n}\nfun unused(): int {\n    return 1\n}\nfun main() {\n    var p = new P(3)\n    var t = 0\n    var i = 0\n    while (i < 200000) {\n        t += sq(p.get()) + 2 * 3\n        i += 1\n    }\n    print(t)\n}\n",
    )
    .unwrap();
    let (out, code) = output(burn().env("BVM_STATS", "1").arg(&tmp));
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(code, 0, "{}", out);
    assert!(out.starts_with("3000000\n"), "{}", out);
    assert!(out.contains("never loaded"), "{}", out);
    let inlined: u32 = out
        .split(" calls inlined")
        .next()
        .and_then(|s| s.rsplit(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    assert!(inlined >= 2, "{}", out);
    assert!(out.contains("1 loops switched to optimized code"), "{}", out);
}

#[test]
fn init_creates_projects_that_build_and_import_packages() {
    let dir = temp_dir("projects");
    let home = dir.join("home");
    let run = |cwd: &Path, args: &[&str]| output(burn().current_dir(cwd).env("BURN_HOME", &home).args(args));

    let (out, code) = run(&dir, &["init", "hello"]);
    assert_eq!(code, 2, "{}", out);
    assert!(out.contains("github.com/you/hello"), "{}", out);

    let (out, code) = run(&dir, &["init", "example.com/ada/greet", "--lib", "--no-git"]);
    assert_eq!(code, 0, "{}", out);
    let (out, code) = run(&dir.join("greet"), &["run", "tests/main.bn"]);
    assert_eq!((out.as_str(), code), ("all tests passed\n", 0));
    let (out, code) = run(&dir.join("greet"), &["run"]);
    assert_eq!(code, 2, "{}", out);
    assert!(out.contains("is a library"), "{}", out);
    let (out, code) = run(&dir, &["init", "example.com/ada/dotted.bn", "--lib", "--no-git"]);
    assert_eq!(code, 0, "{}", out);
    let (out, code) = run(&dir.join("dotted.bn"), &["run", "tests/main.bn"]);
    assert_eq!((out.as_str(), code), ("all tests passed\n", 0));
    std::fs::write(
        dir.join("greet/tests/case.bn"),
        "import \"example.com/Ada/greet.bn\"\n\nprint(greet(\"case\"))\n",
    )
    .unwrap();
    let (out, code) = run(&dir.join("greet"), &["run", "tests/case.bn"]);
    assert_eq!(code, 0, "{}", out);
    assert!(out.contains("case"), "{}", out);

    let (out, code) = run(&dir, &["init", "example.com/ada/app", "--target", "js", "--no-git"]);
    assert_eq!(code, 0, "{}", out);
    let app = dir.join("app");
    let toml = std::fs::read_to_string(app.join("burn.toml")).unwrap();
    assert!(toml.contains("target = \"js\"") && toml.contains("[scripts]"), "{}", toml);
    let (out, code) = run(&app, &["run"]);
    assert_eq!((out.as_str(), code), ("Hello from app!\n", 0));

    std::fs::write(app.join("src/main.bn"), "import \"example.com/ada/greet\"\nimport \"example.com/ada/cached\"\nimport \"example.com/ada/cached/src/more\"\n\nfun main() {\n    print(greet(\"packages\"), twice(21), more())\n}\n").unwrap();
    let (out, code) = run(&app, &["check"]);
    assert_eq!(code, 1);
    assert!(out.contains("not a dependency; add it with `ash install example.com/ada/greet`"), "{}", out);
    assert!(out.contains("this project is `example.com/ada/app`"), "{}", out);

    std::fs::write(
        app.join("burn.toml"),
        toml.replace(
            "[dependencies]\n",
            "[dependencies]\n\"example.com/ada/greet\" = { path = \"../greet\" }\n\"example.com/ada/cached\" = \"^1.0\"\n",
        ),
    )
    .unwrap();
    let (out, code) = run(&app, &["check"]);
    assert_eq!(code, 1);
    assert!(out.contains("`example.com/ada/cached` is not installed yet; run `ash install`"), "{}", out);

    let cached = home.join("packages/example.com/ada/cached@0123456789ab");
    std::fs::create_dir_all(cached.join("src")).unwrap();
    std::fs::write(cached.join("burn.toml"), "[package]\nname = \"example.com/ada/cached\"\nkind = \"lib\"\n").unwrap();
    std::fs::write(cached.join("src/lib.bn"), "pub fun twice(x: int): int {\n    return x * 2\n}\n").unwrap();
    std::fs::write(cached.join("src/more.bn"), "pub fun more(): string {\n    return \"more\"\n}\n").unwrap();
    std::fs::write(
        app.join("burn.lock"),
        "version = 1\n\n[[package]]\nname = \"example.com/ada/cached\"\nversion = \"v1.0.0\"\nrev = \"0123456789abcdef\"\nsource = \"git+https://example.com/ada/cached\"\ndependencies = []\n",
    )
    .unwrap();
    let (out, code) = run(&app, &["run"]);
    assert_eq!((out.as_str(), code), ("Hello, packages! 42 more\n", 0));
    let (out, code) = run(&app, &["build"]);
    assert_eq!(code, 0, "{}", out);
    assert!(out.contains("building example.com/ada/app 0.1.0 (js)"), "{}", out);
    if has_node() {
        let (out, _) = output(Command::new("node").arg(app.join("build/app.js")));
        assert_eq!(out, "Hello, packages! 42 more\n");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn runaway_programs_stop_at_the_heap_limit() {
    let dir = temp_dir("heap");
    let file = dir.join("grow.bn");
    std::fs::write(
        &file,
        "var all: [[int]] = []\nwhile (true) {\n    var chunk: [int] = []\n    for (i in 0..50000) {\n        chunk.push(i)\n    }\n    all.push(chunk)\n}\n",
    )
    .unwrap();
    let (out, code) = output(burn().env("BURN_MAX_HEAP_MB", "64").arg(&file));
    assert_eq!(code, 1, "{}", out);
    assert!(out.contains("out of memory") && out.contains("BURN_MAX_HEAP_MB"), "{}", out);
    if native_supported() {
        let (out, code) = output(burn().env("BURN_MAX_HEAP_MB", "64").args(["run", "--native"]).arg(&file));
        assert_eq!(code, 1, "{}", out);
        assert!(out.contains("out of memory"), "{}", out);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn freed_values_are_never_used_again() {
    let root = root();
    let mut failures = Vec::new();
    for (file, expected) in cases() {
        let rel = file.strip_prefix(&root).unwrap();
        let (out, _) = output(burn().current_dir(&root).env("BURN_RC_CHECK", "1").env("BURN_GC_THRESHOLD", "4096").arg(rel));
        if out != expected {
            failures.push(format!("{}:\n--- expected\n{}\n--- got\n{}", rel.display(), expected, out));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn values_that_only_reference_each_other_are_freed() {
    let dir = temp_dir("cycles");
    let file = dir.join("cycles.bn");
    std::fs::write(
        &file,
        "def struct Node(name: string) {\n    Node? next = null\n    [any] seen = []\n}\n\nfun ring(n: int): int {\n    var first = new Node(\"first\")\n    var cur = first\n    for i in 0..n {\n        var next = new Node(\"node ${i}\")\n        cur.next = next\n        next.seen.push(cur)\n        cur = next\n    }\n    cur.next = first\n    return n\n}\n\nvar total = 0\nfor round in 0..30000 {\n    total += ring(20)\n}\nvar count = fun(): fun(): int {\n    var n = 0\n    var self: any = null\n    var f = fun(): int {\n        n += 1\n        return n\n    }\n    self = f\n    return f\n}\nfor i in 0..100000 {\n    count()()\n}\nprint(total)\n",
    )
    .unwrap();
    let (out, code) = output(burn().env("BURN_MAX_HEAP_MB", "24").arg(&file));
    assert_eq!((out.as_str(), code), ("600000\n", 0));
    if native_supported() {
        let (out, code) = output(burn().env("BURN_MAX_HEAP_MB", "24").args(["run", "--native"]).arg(&file));
        assert_eq!((out.as_str(), code), ("600000\n", 0));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn language_server_completes_imports_and_reports_ambiguity_once() {
    use std::io::{BufRead, BufReader, Read, Write};
    let dir = temp_dir("lsp-imports");
    std::fs::create_dir_all(dir.join("util")).unwrap();
    std::fs::write(dir.join("util/helpers.bn"), "pub fun greet(): int {\n    return 1\n}\n").unwrap();
    std::fs::write(dir.join("other.bn"), "pub fun greet(): int {\n    return 2\n}\n").unwrap();
    let main = dir.join("main.bn");
    let uri = format!("file://{}", main.display());
    let mut child = burn()
        .arg("lsp")
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut send = |body: String| {
        write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        stdin.flush().unwrap();
    };
    let mut read = || -> String {
        let mut len = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.trim().is_empty() {
                break;
            }
            if let Some(v) = line.trim().strip_prefix("Content-Length:") {
                len = v.trim().parse().unwrap();
            }
        }
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    };
    send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#.to_string());
    read();
    let text = "import \"util/helpers\"\nimport \"other\"\nprint(greet())\nimport \"u";
    send(format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{}","languageId":"burn","version":1,"text":{:?}}}}}}}"#,
        uri, text
    ));
    let diags = loop {
        let m = read();
        if m.contains("publishDiagnostics") && m.contains("main.bn") {
            break m;
        }
    };
    assert!(diags.contains("is ambiguous"), "{}", diags);
    assert!(!diags.contains("cannot find `greet`"), "{}", diags);
    send(format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{{"textDocument":{{"uri":"{}"}},"position":{{"line":3,"character":9}}}}}}"#,
        uri
    ));
    let items = loop {
        let m = read();
        if m.contains("\"id\":2") {
            break m;
        }
    };
    assert!(items.contains("\"util/\""), "{}", items);
    assert!(!items.contains("\"print\""), "{}", items);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apps_can_be_imported_as_bytecode_and_changed_with_mixins() {
    let dir = temp_dir("app-bytecode");
    let run = |cwd: &Path, args: &[&str]| output(burn().current_dir(cwd).env("BURN_HOME", dir.join("home")).args(args));
    let (out, code) = run(&dir, &["init", "example.com/ada/game", "--no-git"]);
    assert_eq!(code, 0, "{}", out);
    let game = dir.join("game/src/main.bn");
    std::fs::write(
        &game,
        "pub fun score(points: int): int {\n    return points * 10\n}\n\nfun main() {\n    print(\"score\", score(3))\n}\n",
    )
    .unwrap();
    let (out, code) = run(&dir, &["init", "example.com/ada/mod", "--no-git"]);
    assert_eq!(code, 0, "{}", out);
    let m = dir.join("mod");
    let toml = std::fs::read_to_string(m.join("burn.toml")).unwrap();
    std::fs::write(
        m.join("burn.toml"),
        toml.replace("[dependencies]\n", "[dependencies]\n\"example.com/ada/game\" = { path = \"../game\" }\n"),
    )
    .unwrap();
    std::fs::write(
        m.join("src/main.bn"),
        "import \"example.com/ada/game.bvmc\"\n\n@Inject(target: \"score\", at: \"return\")\nfun doubled(points: int, result: int): int {\n    return result * 2\n}\n",
    )
    .unwrap();
    let (out, code) = run(&m, &["run"]);
    assert_eq!((out.as_str(), code), ("score 60\n", 0));
    assert!(dir.join("game/build/game.bvmc").is_file());
    if native_supported() {
        let (out, code) = run(&m, &["run", "--native"]);
        assert_eq!((out.as_str(), code), ("score 60\n", 0));
    }
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(
        &game,
        "pub fun score(points: int): int {\n    return points * 100\n}\n\nfun main() {\n    print(\"score\", score(3))\n}\n",
    )
    .unwrap();
    let (out, _) = run(&m, &["run"]);
    assert_eq!(out, "score 600\n");
    std::fs::write(m.join("src/self.bn"), "import \"example.com/ada/mod.bvmc\"\n").unwrap();
    let (out, code) = run(&m, &["check", "src/self.bn"]);
    assert_eq!(code, 1);
    assert!(out.contains("cannot import its own bytecode"), "{}", out);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn programs_without_the_standard_runtime_are_small_and_behave_the_same() {
    if !native_supported() {
        return;
    }
    let root = root();
    let tmp = temp_dir("nostd");
    let mut failures = Vec::new();
    let mut built = 0;
    for (file, expected) in cases() {
        let rel = file.strip_prefix(&root).unwrap();
        let (_, code) = output(burn().current_dir(&root).args(["check", "--no-std"]).arg(rel));
        if code != 0 {
            continue;
        }
        let exe = tmp.join(file.file_stem().unwrap());
        let (build_out, code) = output(burn().current_dir(&root).args(["build", "--no-std"]).arg(rel).arg("-o").arg(&exe));
        if code != 0 {
            failures.push(format!("{}: build failed\n{}", rel.display(), build_out));
            continue;
        }
        built += 1;
        let (out, _) = output(Command::new(&exe).current_dir(&root).env("BURN_RC_CHECK", "1").env("BURN_GC_THRESHOLD", "4096"));
        if out != expected {
            failures.push(format!("{}:\n--- expected\n{}\n--- got\n{}", rel.display(), expected, out));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(built >= 15, "only {} cases built without the standard runtime", built);

    let hello = tmp.join("hello.bn");
    std::fs::write(&hello, "fun main() {\n    print(\"hello\")\n}\n").unwrap();
    let size = |no_std: bool| {
        let exe = tmp.join(if no_std { "small" } else { "full" });
        let mut cmd = burn();
        cmd.arg("build").arg(&hello).arg("-o").arg(&exe);
        if no_std {
            cmd.arg("--no-std");
        }
        let (out, code) = output(&mut cmd);
        assert_eq!(code, 0, "{}", out);
        assert_eq!(output(&mut Command::new(&exe)).0, "hello\n");
        std::fs::metadata(&exe).unwrap().len()
    };
    let (small, full) = (size(true), size(false));
    assert!(small * 4 < full, "no-std hello is {} bytes, the full one {}", small, full);

    let project = tmp.join("app");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("burn.toml"), "[package]\nname = \"example.com/ada/app\"\nstd = false\n").unwrap();
    std::fs::write(
        project.join("src/main.bn"),
        "import \"std/http\"\nimport \"std/strings\"\n\nasync fun later(): int {\n    return 1\n}\n\nfun main() {\n    print(toJSON([1]), await later())\n}\n",
    )
    .unwrap();
    let (out, code) = output(burn().current_dir(&project).arg("check"));
    assert_eq!(code, 1, "{}", out);
    for msg in [
        "`std/http` is part of the standard library",
        "`std/strings` is part of the standard library",
        "an `async fun` needs the standard library",
        "`toJSON` is in the standard library module `std/json`",
        "`await` needs the standard library",
        "std = false",
    ] {
        assert!(out.contains(msg), "missing `{}` in\n{}", msg, out);
    }
    assert!(!out.contains("import it with"), "{}", out);

    std::fs::write(
        project.join("src/main.bn"),
        "fun main() {\n    var names = [\"b\", \"a\"]\n    names.sort()\n    print(names.join(\",\"), \"x\".upper(), toInt(\"41\") + 1)\n}\n",
    )
    .unwrap();
    let (out, code) = output(burn().current_dir(&project).arg("build"));
    assert_eq!(code, 0, "{}", out);
    let (out, code) = output(Command::new(project.join("build/app")).current_dir(&project));
    assert_eq!((out.as_str(), code), ("a,b X 42\n", 0));
    std::fs::write(
        project.join("src/main.bn"),
        "import \"std/fs\"\n\nfun main() {\n    print(readFile(\"x\"))\n}\n",
    )
    .unwrap();
    let (out, code) = output(burn().current_dir(&project).arg("check"));
    assert_eq!(code, 1, "{}", out);
    assert!(out.contains("`std/fs` is part of the standard library"), "{}", out);
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn uint64_uses_all_64_bits_on_bvm_and_natively() {
    let dir = temp_dir("uint64");
    let src = dir.join("hash.bn");
    std::fs::write(
        &src,
        "fun fnv(text: string): uint64 {\n    uint64 h = 14695981039346656037\n    for b in text.bytes() {\n        h = (h ^ b).wrappingMul(1099511628211)\n    }\n    return h\n}\n\nfun main() {\n    uint64 top = 18446744073709551615\n    print(fnv(\"hello\"), top, top - 1, top > 9223372036854775807, top >> 63, top / 3, 0xFFFFFFFFFFFFFFFF as uint64 == top)\n    print(top + 1)\n}\n",
    )
    .unwrap();
    let expected = "11831194018420276491 18446744073709551615 18446744073709551614 true 1 6148914691236517205 true\n";
    let (out, code) = output(burn().arg(&src));
    assert_eq!(code, 1, "{}", out);
    assert!(out.starts_with(expected), "{}", out);
    assert!(out.contains("integer overflow: 18446744073709551615 + 1 does not fit in uint64"), "{}", out);
    if native_supported() {
        for no_std in [false, true] {
            let exe = dir.join(if no_std { "small" } else { "full" });
            let mut cmd = burn();
            cmd.arg("build").arg(&src).arg("-o").arg(&exe);
            if no_std {
                cmd.arg("--no-std");
            }
            let (build, code) = output(&mut cmd);
            assert_eq!(code, 0, "{}", build);
            let (native, _) = output(&mut Command::new(&exe));
            assert_eq!(native, out);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn language_server_completes_enum_variants_with_data() {
    use std::io::BufReader;
    let dir = temp_dir("lsp-enums").canonicalize().unwrap();
    let main = dir.join("main.bn");
    let text = "def enum Token {\n    Word(text: string),\n    End\n}\n\nfun main() {\n    var t = Token.\n}\n";
    std::fs::write(&main, text).unwrap();
    let uri = format!("file://{}", main.display());
    let mut child = burn()
        .arg("lsp")
        .current_dir(&dir)
        .env("BURN_HOME", dir.join("home"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut lsp = Lsp {
        stdin: child.stdin.take().unwrap(),
        out: BufReader::new(child.stdout.take().unwrap()),
        id: 0,
    };
    lsp.request("initialize", r#"{"capabilities":{}}"#);
    lsp.open(&uri, text);
    let at = |line: usize, ch: usize| format!(r#"{{"textDocument":{{"uri":"{}"}},"position":{{"line":{},"character":{}}}}}"#, uri, line, ch);
    let items = lsp.request("textDocument/completion", &at(6, 18));
    assert!(items.contains("\"label\":\"Word\"") && items.contains("Token.Word(text: string)"), "{}", items);
    assert!(items.contains("\"label\":\"End\""), "{}", items);
    let symbols = lsp.request("textDocument/documentSymbol", &format!(r#"{{"textDocument":{{"uri":"{}"}}}}"#, uri));
    assert!(symbols.contains("\"Token\"") && symbols.contains("\"Word\""), "{}", symbols);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

struct Lsp {
    stdin: std::process::ChildStdin,
    out: std::io::BufReader<std::process::ChildStdout>,
    id: u32,
}
impl Lsp {
    fn open(&mut self, uri: &str, text: &str) {
        self.send(&format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{}","languageId":"burn","version":1,"text":{:?}}}}}}}"#,
            uri, text
        ));
    }
    fn change(&mut self, uri: &str, text: &str) {
        self.send(&format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didChange","params":{{"textDocument":{{"uri":"{}","version":2}},"contentChanges":[{{"text":{:?}}}]}}}}"#,
            uri, text
        ));
    }
    fn send(&mut self, body: &str) {
        use std::io::Write;
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        self.stdin.flush().unwrap();
    }
    fn request(&mut self, method: &str, params: &str) -> String {
        use std::io::{BufRead, Read};
        self.id += 1;
        let body = format!(r#"{{"jsonrpc":"2.0","id":{},"method":"{}","params":{}}}"#, self.id, method, params);
        self.send(&body);
        loop {
            let mut len = 0usize;
            loop {
                let mut line = String::new();
                self.out.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
                if let Some(v) = line.trim().strip_prefix("Content-Length:") {
                    len = v.trim().parse().unwrap();
                }
            }
            let mut buf = vec![0u8; len];
            self.out.read_exact(&mut buf).unwrap();
            let m = String::from_utf8(buf).unwrap();
            if m.contains(&format!("\"id\":{},", self.id)) || m.contains(&format!("\"id\":{}}}", self.id)) {
                return m;
            }
        }
    }
}

#[test]
fn language_server_navigates_into_libraries_and_renames_across_files() {
    use std::io::BufReader;
    let dir = temp_dir("lsp-nav").canonicalize().unwrap();
    let home = dir.join("home");
    std::fs::write(
        dir.join("shapes.bn"),
        "pub def interface Shape {\n    fun area(): float\n}\n\npub def struct Square(side: float) :: Shape {\n    fun area(): float {\n        return side * side\n    }\n}\n\npub fun twice(x: int): int {\n    return x * 2\n}\n",
    )
    .unwrap();
    let main = dir.join("main.bn");
    let text = "import \"shapes.bn\"\nimport \"std/date\"\nimport \"std/math\"\n\nfun total(s: Shape): float {\n    return s.area()\n}\n\nfun main() {\n    var sq = new Square(2.0)\n    print(total(sq), twice(3), sqrt(2.0), Date.today())\n    twice(4)\n}\n";
    std::fs::write(&main, text).unwrap();
    let uri = format!("file://{}", main.display());
    let mut child = burn()
        .arg("lsp")
        .current_dir(&dir)
        .env("BURN_HOME", &home)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut lsp = Lsp {
        stdin: child.stdin.take().unwrap(),
        out: BufReader::new(child.stdout.take().unwrap()),
        id: 0,
    };
    let init = lsp.request("initialize", &format!(r#"{{"rootUri":"file://{}","capabilities":{{}}}}"#, dir.display()));
    for cap in [
        "renameProvider",
        "referencesProvider",
        "signatureHelpProvider",
        "inlayHintProvider",
        "workspaceSymbolProvider",
    ] {
        assert!(init.contains(cap), "{}", init);
    }
    lsp.send(&format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{}","languageId":"burn","version":1,"text":{:?}}}}}}}"#,
        uri, text
    ));
    let at = |line: usize, ch: usize| format!(r#"{{"textDocument":{{"uri":"{}"}},"position":{{"line":{},"character":{}}}}}"#, uri, line, ch);
    let def = lsp.request("textDocument/definition", &at(10, 32));
    assert!(def.contains("/cache/sources/") && def.contains("builtins.bn"), "{}", def);
    let builtins = std::fs::read_to_string(home.join(format!("cache/sources/{}/builtins.bn", env!("CARGO_PKG_VERSION")))).unwrap();
    assert!(builtins.contains("fun sqrt(x: float): float"));
    let def = lsp.request("textDocument/definition", &at(10, 42));
    assert!(def.contains("std/date.bn"), "{}", def);
    let def = lsp.request("textDocument/definition", &at(5, 15));
    assert!(def.contains("shapes.bn") && def.contains("\"line\":1"), "{}", def);
    let refs = lsp.request(
        "textDocument/references",
        &at(10, 23).replace("}}", "},\"context\":{\"includeDeclaration\":true}}"),
    );
    assert_eq!(refs.matches("\"uri\"").count(), 3, "{}", refs);
    let rename = lsp.request("textDocument/rename", &at(5, 15).replace("}}", "},\"newName\":\"size\"}"));
    assert!(rename.contains("shapes.bn") && rename.contains("main.bn"), "{}", rename);
    assert_eq!(rename.matches("\"newText\":\"size\"").count(), 3, "{}", rename);
    let bad = lsp.request("textDocument/rename", &at(10, 32).replace("}}", "},\"newName\":\"root\"}"));
    assert!(bad.contains("built into Burn"), "{}", bad);
    let sig = lsp.request("textDocument/signatureHelp", &at(11, 11));
    assert!(sig.contains("fun twice(x: int): int"), "{}", sig);
    let hints = lsp.request(
        "textDocument/inlayHint",
        &format!(
            r#"{{"textDocument":{{"uri":"{}"}},"range":{{"start":{{"line":0,"character":0}},"end":{{"line":20,"character":0}}}}}}"#,
            uri
        ),
    );
    assert!(hints.contains(": Square"), "{}", hints);
    let symbols = lsp.request("workspace/symbol", r#"{"query":"twi"}"#);
    assert!(symbols.contains("\"twice\"") && symbols.contains("shapes.bn"), "{}", symbols);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn language_server_keeps_working_while_typing_and_links_imports() {
    use std::io::BufReader;
    let dir = temp_dir("lsp-ide").canonicalize().unwrap();
    let home = dir.join("home");
    std::fs::write(
        dir.join("shapes.bn"),
        "pub def struct Square(side: float) {\n    fun area(): float {\n        return side * side\n    }\n}\n\npub fun twice(x: int): int {\n    return x * 2\n}\n",
    )
    .unwrap();
    let main = dir.join("main.bn");
    let text =
        "import \"shapes.bn\"\nimport \"std/date\"\n\nfun main() {\n    var sq = new Square(2.0)\n    var name = \"burn\"\n    println(sq.area(), name)\n}\n";
    std::fs::write(&main, text).unwrap();
    let uri = format!("file://{}", main.display());
    let mut child = burn()
        .arg("lsp")
        .current_dir(&dir)
        .env("BURN_HOME", &home)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut lsp = Lsp {
        stdin: child.stdin.take().unwrap(),
        out: BufReader::new(child.stdout.take().unwrap()),
        id: 0,
    };
    let init = lsp.request("initialize", &format!(r#"{{"rootUri":"file://{}","capabilities":{{}}}}"#, dir.display()));
    for cap in [
        "documentLinkProvider",
        "foldingRangeProvider",
        "typeDefinitionProvider",
        "implementationProvider",
    ] {
        assert!(init.contains(cap), "{}", init);
    }
    lsp.open(&uri, text);
    let at = |line: usize, ch: usize| format!(r#"{{"textDocument":{{"uri":"{}"}},"position":{{"line":{},"character":{}}}}}"#, uri, line, ch);
    let doc = format!(r#"{{"textDocument":{{"uri":"{}"}}}}"#, uri);
    let links = lsp.request("textDocument/documentLink", &doc);
    assert!(links.contains("shapes.bn") && links.contains("std/date.bn"), "{}", links);
    let def = lsp.request("textDocument/definition", &at(0, 10));
    assert!(def.contains("shapes.bn"), "{}", def);
    let hover = lsp.request("textDocument/hover", &at(1, 10));
    assert!(hover.contains("std/date") && hover.contains("Exports"), "{}", hover);
    let folds = lsp.request("textDocument/foldingRange", &doc);
    assert!(folds.contains("\"startLine\":3"), "{}", folds);
    let tdef = lsp.request("textDocument/typeDefinition", &at(4, 9));
    assert!(tdef.contains("shapes.bn"), "{}", tdef);

    let typing = "import \"shapes.bn\"\nimport \"std/date\"\n\nfun main() {\n    var sq = new Square(2.0)\n    var name = \"burn\"\n    if name != \"\" {\n        sq.\n}\n";
    lsp.change(&uri, typing);
    let members = lsp.request("textDocument/completion", &at(7, 11));
    assert!(members.contains("\"area\"") && members.contains("\"side\""), "{}", members);
    let typing = "import \"shapes.bn\"\n\nfun main() {\n    var sq = new Square(2.0)\n    var name = \"burn\"\n    name.\n}\n";
    lsp.change(&uri, typing);
    let strings = lsp.request("textDocument/completion", &at(5, 9));
    assert!(
        strings.contains("\"upper\"") && strings.contains("\"trim\"") && !strings.contains("__exec"),
        "{}",
        strings
    );
    let typing = "import \"shapes.bn\"\n\nfun main() {\n    var word = \"x\"\n    println(wo\n}\n";
    lsp.change(&uri, typing);
    let locals = lsp.request("textDocument/completion", &at(4, 14));
    assert!(locals.contains("\"word\"") && locals.contains("import \\\"std/strings\\\""), "{}", locals);

    let typing = "def annotation Route {\n    string path\n    string method = \"GET\"\n}\n\n@\n";
    lsp.change(&uri, typing);
    let annotations = lsp.request("textDocument/completion", &at(5, 1));
    assert!(
        annotations.contains("\"Route\"") && annotations.contains("\"Deprecated\"") && !annotations.contains("\"println\""),
        "{}",
        annotations
    );
    let typing = "def annotation Route {\n    string path\n    string method = \"GET\"\n}\n\n@Route(\"/x\", \n";
    lsp.change(&uri, typing);
    let fields = lsp.request("textDocument/completion", &at(5, 14));
    assert!(fields.contains("\"method\"") && fields.contains("method: "), "{}", fields);
    let typing = "import \"shapes.bn\"\n\nfun area(s: \n";
    lsp.change(&uri, typing);
    let types = lsp.request("textDocument/completion", &at(2, 12));
    assert!(
        types.contains("\"Square\"") && types.contains("\"uint64\"") && !types.contains("\"println\""),
        "{}",
        types
    );
    let typing = "def \n";
    lsp.change(&uri, typing);
    let kinds = lsp.request("textDocument/completion", &at(0, 4));
    assert!(
        kinds.contains("\"struct\"") && kinds.contains("\"interface\"") && !kinds.contains("\"println\""),
        "{}",
        kinds
    );

    let missing = "fun main() {\n    println(twice(2), padLeft(\"a\", 2, \" \"))\n}\n";
    lsp.change(&uri, missing);
    let diags = r#"[{"range":{"start":{"line":1,"character":12},"end":{"line":1,"character":17}},"message":"cannot find `twice` in this scope"},{"range":{"start":{"line":1,"character":22},"end":{"line":1,"character":29}},"message":"cannot find `padLeft` in this scope"}]"#;
    let actions = lsp.request(
        "textDocument/codeAction",
        &format!(
            r#"{{"textDocument":{{"uri":"{}"}},"range":{{"start":{{"line":1,"character":0}},"end":{{"line":1,"character":40}}}},"context":{{"diagnostics":{}}}}}"#,
            uri, diags
        ),
    );
    assert!(actions.contains("Import `twice` from \\\"shapes.bn\\\""), "{}", actions);
    assert!(actions.contains("Import `padLeft` from \\\"std/strings\\\""), "{}", actions);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn language_server_imports_the_project_itself_by_package_name() {
    use std::io::BufReader;
    let dir = temp_dir("lsp-self").canonicalize().unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("tests")).unwrap();
    std::fs::write(
        dir.join("burn.toml"),
        "[package]\nname = \"example.com/Ada/shapes\"\nversion = \"0.1.0\"\nkind = \"lib\"\nmain = \"src/lib.bn\"\n\n[dependencies]\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/lib.bn"), "pub fun twice(x: int): int {\n    return x * 2\n}\n").unwrap();
    let test = dir.join("tests/main.bn");
    let text = "import \"example.com/ada/\"\n\nassert(twice(2) == 4)\n";
    std::fs::write(&test, text).unwrap();
    let uri = format!("file://{}", test.display());
    let mut child = burn()
        .arg("lsp")
        .current_dir(&dir)
        .env("BURN_HOME", dir.join("home"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut lsp = Lsp {
        stdin: child.stdin.take().unwrap(),
        out: BufReader::new(child.stdout.take().unwrap()),
        id: 0,
    };
    lsp.request("initialize", &format!(r#"{{"rootUri":"file://{}","capabilities":{{}}}}"#, dir.display()));
    lsp.open(&uri, text);
    let paths = lsp.request(
        "textDocument/completion",
        &format!(r#"{{"textDocument":{{"uri":"{}"}},"position":{{"line":0,"character":24}}}}"#, uri),
    );
    assert!(paths.contains("\"example.com/Ada/shapes\""), "{}", paths);
    let diags = r#"[{"range":{"start":{"line":2,"character":7},"end":{"line":2,"character":12}},"message":"cannot find `twice` in this scope"}]"#;
    let actions = lsp.request(
        "textDocument/codeAction",
        &format!(
            r#"{{"textDocument":{{"uri":"{}"}},"range":{{"start":{{"line":2,"character":0}},"end":{{"line":2,"character":20}}}},"context":{{"diagnostics":{}}}}}"#,
            uri, diags
        ),
    );
    assert!(actions.contains("Import `twice` from \\\"example.com/Ada/shapes\\\""), "{}", actions);
    let fixed = "import \"example.com/ada/shapes\"\n\nassert(twice(2) == 4)\n";
    lsp.change(&uri, fixed);
    let def = lsp.request(
        "textDocument/definition",
        &format!(r#"{{"textDocument":{{"uri":"{}"}},"position":{{"line":2,"character":8}}}}"#, uri),
    );
    assert!(def.contains("src/lib.bn"), "{}", def);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn language_server_reloads_the_project_with_ash_sync() {
    use std::io::BufReader;
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("lsp-reload").canonicalize().unwrap();
    let home = dir.join("home");
    std::fs::create_dir_all(home.join("bin")).unwrap();
    let ash = home.join("bin/ash");
    std::fs::write(&ash, format!("#!/bin/sh\necho \"$@\" > \"{}\"\n", dir.join("ran").display())).unwrap();
    std::fs::set_permissions(&ash, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("burn.toml"),
        "[package]\nname = \"example.com/ada/app\"\nversion = \"0.1.0\"\nmain = \"src/main.bn\"\n\n[dependencies]\n\"example.com/ada/colors\" = \"^1.0\"\n",
    )
    .unwrap();
    let main = dir.join("src/main.bn");
    let text = "import \"example.com/ada/colors\"\n\nfun main() {}\n";
    std::fs::write(&main, text).unwrap();
    let uri = format!("file://{}", main.display());
    let mut child = burn()
        .arg("lsp")
        .current_dir(&dir)
        .env("BURN_HOME", &home)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut lsp = Lsp {
        stdin: child.stdin.take().unwrap(),
        out: BufReader::new(child.stdout.take().unwrap()),
        id: 0,
    };
    let init = lsp.request("initialize", &format!(r#"{{"rootUri":"file://{}","capabilities":{{}}}}"#, dir.display()));
    assert!(init.contains("burn.server.reloadProject"), "{}", init);
    lsp.open(&uri, text);
    let diags = r#"[{"range":{"start":{"line":0,"character":7},"end":{"line":0,"character":31}},"message":"the package `example.com/ada/colors` is not installed yet; run `ash install`"}]"#;
    let actions = lsp.request(
        "textDocument/codeAction",
        &format!(
            r#"{{"textDocument":{{"uri":"{}"}},"range":{{"start":{{"line":0,"character":0}},"end":{{"line":0,"character":31}}}},"context":{{"diagnostics":{}}}}}"#,
            uri, diags
        ),
    );
    assert!(
        actions.contains("Reload project (ash sync)") && actions.contains("burn.server.reloadProject"),
        "{}",
        actions
    );
    let done = lsp.request(
        "workspace/executeCommand",
        &format!(r#"{{"command":"burn.server.reloadProject","arguments":["{}"]}}"#, uri),
    );
    assert!(done.contains("\"result\":null"), "{}", done);
    assert_eq!(std::fs::read_to_string(dir.join("ran")).unwrap().trim(), "sync");
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

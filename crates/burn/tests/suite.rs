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
    for dir in ["tests/cases", "examples", "lib/std", "tests/modules"] {
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
fn burnfmt_written_in_burn_matches_the_builtin_formatter() {
    let root = root();
    let tool = root.join("tools/burnfmt/burnfmt.bn");
    let mut files = vec![tool.clone()];
    for dir in ["tests/cases", "examples", "lib/std"] {
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
    assert!(out.contains("fixed 7 problems"), "{}", out);
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

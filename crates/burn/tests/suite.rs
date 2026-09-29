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
                failures.push(format!("{} (gc threshold {:?}):\n--- expected\n{}\n--- got\n{}", rel.display(), threshold, expected, out));
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
    let mut child = burn().arg("repl").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    child.stdin.as_mut().unwrap().write_all(b"var x = 20\nfun twice(n: int): int {\n    return n * 2\n}\ntwice(x) + 2\nx = x + 1\nprint(x)\nundefinedThing\nprint(\"still alive\")\n").unwrap();
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

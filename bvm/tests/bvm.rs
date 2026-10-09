use bvm::asm::{assemble, disassemble};
use bvm::binary::{decode, encode};
use bvm::runtime::io;
use bvm::{verify, Cmp, Desc, FuncBuilder, Host, ModuleBuilder, Op, RtFn, Runner};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

static CAPTURE: Mutex<()> = Mutex::new(());

fn capture_lock() -> MutexGuard<'static, ()> {
    CAPTURE.lock().unwrap_or_else(|e| e.into_inner())
}

const SAMPLE: &str = r#"
; every section and most instructions
type Shape = interface
type Point = record { x: int, y: int }
type Circle = struct { center: Point, r: float } implements Shape
type Color = enum { Red, Green, "Light Blue" }
type #16 = record "odd name" { "a b": [Point], m: map<string, int?> }
type Pending = future<[int]>

global counter
global "odd global"
import clock_hint 0
table Shape.area 1 { Circle: circle_area }

func circle_area(c)
    load c
    getf 1
    dup
    fmul
    const 3.25
    fmul
    ret
end

func sum_to(n)
    local acc
    local i
    const 0
    store acc
    const 0
    store i
top:
    load i
    load n
    ilt
    jz done
    load acc
    load i
    iadd
    store acc
    load i
    const 1
    iadd
    store i
    jmp top
done:
    load acc
    ret
end

func bits(2)
    locals 1
    load 0
    load 1
    and
    load 0
    const 3
    shl
    xor
    const -1
    ushr
    const 0x10
    or
    load 1
    const 2
    shr
    swap
    pop
    ret
end

func main()
    local p
    local arr
    const 1
    const 2
    new Point
    store p
    load p
    const 5
    setf Point.y
    pop
    load p
    getf Point.x
    const 1
    const 2
    const 3
    newarr [int] 3
    store arr
    load arr
    const 0
    index
    iadd
    load arr
    len
    iadd
    const 10
    call sum_to
    iadd
    gtee counter
    gstore "odd global"
    str "a \"quoted\" string\n\u{1}"
    str "a \"quoted\" string\n\u{1}"
    rt str_eq
    jnz ok
    fref sum_to
    const 3
    swap
    calli 1
    pop
ok:
    const 7
    const 2
    irem
    const 2
    ule
    jzk skip
    pop
    const 1
skip:
    pop
    const 2.5
    f2i
    i2f
    fneg
    const 1.5
    fge
    pop
    retv
end
"#;

#[test]
fn text_and_bytecode_round_trip() {
    let m = assemble(SAMPLE).unwrap_or_else(|e| panic!("{}", e));
    verify(&m).unwrap_or_else(|e| panic!("{}", e));
    let text = disassemble(&m);
    let again = assemble(&text).unwrap_or_else(|e| panic!("{}\n{}", e, text));
    assert_eq!(m, again, "text round trip changed the module:\n{}", text);
    assert_eq!(disassemble(&again), text);
    let bytes = encode(&m);
    assert_eq!(decode(&bytes).unwrap(), m);
    assert_eq!(m.entry, m.func("main"));
    assert!(matches!(&m.types[16], Desc::Record { name, .. } if name == "odd name"));
    assert_eq!(m.funcs[m.func("sum_to").unwrap() as usize].names, ["n", "acc", "i"]);
}

#[test]
fn builder_produces_verified_modules() {
    let mut mb = ModuleBuilder::new();
    let fact = mb.declare("fact", 1);
    let mut f = FuncBuilder::with_params(&["n"]);
    let base = f.label();
    f.load(0).int(1).icmp(Cmp::Le).jnz(base);
    f.load(0).load(0).int(1).emit(Op::ISub).call(fact).emit(Op::IMul).ret();
    f.bind(base).int(1).ret();
    mb.define(fact, f);
    let mut main = FuncBuilder::new(0);
    main.int(10).call(fact).int(3).rt(RtFn::ToStr).rt(RtFn::Print).ret_void();
    let id = mb.function("main", main);
    mb.entry(id);
    assert!(mb.undefined().is_empty());
    let m = mb.finish();
    verify(&m).unwrap();
    let text = disassemble(&m);
    assert!(text.contains("func fact(n)"), "{}", text);
    assert_eq!(assemble(&text).unwrap(), m);
}

fn rejects(src: &str, needle: &str) {
    let m = assemble(src).unwrap_or_else(|e| panic!("{}", e));
    let e = verify(&m).expect_err(src).to_string();
    assert!(e.contains(needle), "expected {:?} in {:?}", needle, e);
}

#[test]
fn verifier_rejects_malformed_code() {
    rejects("func f()\n    iadd\n    retv\nend", "needs 2 values");
    rejects("func f()\n    const 1\nend", "run past the last instruction");
    rejects(
        "func f()\n    const 1\n    jz a\n    const 1\na:\n    retv\nend",
        "holds 1 values on one path here and 0 on another",
    );
    rejects("func f()\n    load 3\n    retv\nend", "local 3 does not exist");
    rejects("func f()\n    gload @4\n    retv\nend", "global @4 does not exist");
    rejects("func f()\n    call @9\n    retv\nend", "function @9 does not exist");
    rejects("func f()\n    rt rt_init\n    retv\nend", "reserved for native code");
    rejects("func f(a)\n    retv\nend\nentry f", "must not take parameters");
    rejects(
        "type P = record { x: int }\nfunc f()\n    const 1\n    const 2\n    newarr P 2\n    retv\nend",
        "needs an array type",
    );
    rejects(
        "type S = interface\ntype C = class { x: int } implements S\ntable t 1 { C: g }\nfunc g(a, b)\n    const 0\n    ret\nend",
        "takes 1 arguments but function g takes 2",
    );
    let mut m = assemble("func f()\n    retv\nend").unwrap();
    m.funcs[0].code.insert(0, Op::IncLocal(0, 1));
    assert!(verify(&m).unwrap_err().to_string().contains("internal instruction"));
}

#[test]
fn assembler_reports_lines() {
    let e = assemble("func f()\n    const 1\n    bogus\nend").unwrap_err();
    assert_eq!(e.line, 3);
    assert!(e.msg.contains("unknown instruction bogus"));
    assert_eq!(assemble("func f()\n    jmp nowhere\nend").unwrap_err().line, 2);
    assert!(assemble("type A = record { x: Missing }").unwrap_err().msg.contains("unknown type Missing"));
    assert!(assemble("end").unwrap_err().msg.contains("without a matching func"));
}

#[test]
fn damaged_bytecode_is_rejected_without_panicking() {
    let m = assemble(SAMPLE).unwrap();
    let bytes = encode(&m);
    for n in 0..bytes.len() {
        assert!(decode(&bytes[..n]).is_err(), "a module cut to {} bytes decoded", n);
    }
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    for _ in 0..3000 {
        let mut b = bytes.clone();
        for _ in 0..3 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let i = (seed as usize) % b.len();
            b[i] ^= (seed >> 32) as u8 | 1;
        }
        if let Ok(m) = decode(&b) {
            let _ = verify(&m);
            let _ = disassemble(&m);
        }
    }
}

#[test]
fn modules_run_in_process_with_host_functions() {
    let src = r#"
import add3 3
global total

func twice(x)
    load x
    const 2
    imul
    ret
end

func main()
    const 1
    const 2
    const 3
    host add3
    call twice
    gstore total
    str "total is "
    gload total
    const 3
    rt to_str
    rt str_concat
    rt print
    pop
    retv
end
"#;
    let m = assemble(src).unwrap();
    assert!(matches!(bvm::load(&m, &Host::new()), Err(bvm::LoadError::MissingImport(n)) if n == "add3"));
    let mut host = Host::new();
    host.register("add3", 2, |a| a[0] + a[1]);
    assert!(matches!(bvm::load(&m, &host), Err(bvm::LoadError::ImportArity { .. })));
    host.register("add3", 3, |a| a[0] + a[1] + a[2]);
    let prog = bvm::load(&m, &host).unwrap();
    let _guard = capture_lock();
    io::start_capture();
    let mut r = Runner::new(prog.clone(), Vec::new());
    r.call(prog.entry.unwrap());
    let twice = prog.func("twice").unwrap();
    assert_eq!(r.call_with(twice, &[21]), 42);
    let globals = r.finish();
    assert_eq!(io::take_capture(), "total is 12\n");
    assert_eq!(globals[0], 12);
}

fn bvm_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bvm"))
}

#[test]
fn command_line_tool_assembles_runs_and_disassembles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let fib = root.join("examples/bvm/fib.bvm");
    let out = bvm_cmd().arg(&fib).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("fib(0) = 0\n") && text.ends_with("fib(14) = 377\n"), "{}", text);
    let dir = std::env::temp_dir().join(format!("bvm-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("fib.bvmc");
    assert!(bvm_cmd().arg("asm").arg(&fib).arg("-o").arg(&bin).status().unwrap().success());
    let out2 = bvm_cmd().arg("run").arg(&bin).output().unwrap();
    assert_eq!(out.stdout, out2.stdout);
    let dis = bvm_cmd().args(["dis"]).arg(&bin).args(["-o", "-"]).output().unwrap();
    let m = assemble(&String::from_utf8_lossy(&dis.stdout)).unwrap();
    assert_eq!(encode(&m), std::fs::read(&bin).unwrap());
    let bad = dir.join("bad.bvm");
    std::fs::write(&bad, "func main()\n    pop\n    retv\nend\n").unwrap();
    let out = bvm_cmd().arg("check").arg(&bad).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("pop needs 1 values"));
    let _ = std::fs::remove_dir_all(&dir);
}

const LIB: &str = r#"
module greetlib
type Point = record { x: int, y: int }

func greet(name: string): string
    str "hello "
    load name
    rt str_concat
    ret
end

func make(x: int): Point
    load x
    load x
    new Point
    ret
end

func secret(): int
    const 7
    ret
end
"#;

const APP: &str = r#"
module app
type Point = record { x: int, y: int }
extern func greet(name: string): string
extern func "greetlib::make"(x: int): Point

func main(): int
    str "bvm"
    call greet
    pop
    const 2
    call "greetlib::make"
    getf Point.y
    ret
end
"#;

fn link_src(srcs: &[&str]) -> Result<bvm::Module, String> {
    let ms: Vec<bvm::Module> = srcs.iter().map(|s| assemble(s).unwrap_or_else(|e| panic!("{}", e))).collect();
    bvm::link(&ms)
}

#[test]
fn linker_resolves_externs_and_merges_types() {
    let m = link_src(&[APP, LIB]).unwrap();
    assert!(m.funcs.iter().all(|f| !f.external));
    let points = m.types.iter().filter(|d| matches!(d, Desc::Record { name, .. } if name == "Point")).count();
    assert_eq!(points, 1, "identical record types are merged");
    assert_eq!(m.name, "app");
    assert_eq!(m.funcs[m.entry.unwrap() as usize].name, "main");
    let err = link_src(&[APP]).unwrap_err();
    assert!(err.contains("needs function greet"), "{}", err);
    let dup = LIB.replace("module greetlib", "module other");
    let err = link_src(&[APP, LIB, &dup]).unwrap_err();
    assert!(err.contains("defined in several modules"), "{}", err);
    let partial = bvm::link_with(
        &[assemble(APP).unwrap()],
        &bvm::LinkOptions {
            allow_unresolved: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(partial.funcs.iter().filter(|f| f.external).count(), 2);
    let bad = APP.replace("extern func greet(name: string): string", "extern func greet(name: int): string");
    let err = link_src(&[&bad, LIB]).unwrap_err();
    assert!(err.contains("different parameter or return types"), "{}", err);
}

fn run_capture(m: &bvm::Module, host: &Host) -> String {
    let prog = bvm::load(m, host).unwrap_or_else(|e| panic!("{}", e));
    let _guard = capture_lock();
    io::start_capture();
    let mut r = Runner::new(prog.clone(), Vec::new());
    r.call(prog.entry.unwrap());
    r.finish();
    io::take_capture()
}

const MIXIN_TARGETS: &str = r#"
func shout(s: string): string
    load s
    rt str_upper
    ret
end

func show(s: string): void
    load s
    rt print
    pop
    retv
end

func main()
    str "a"
    call shout
    call show
    pop
    str "abcdefgh"
    call shout
    call show
    pop
    retv
end
"#;

const MIXIN_HOOKS: &str = r#"
@Inject target="shout" at="head" cancellable=true
func guard(s: string): string
    load s
    rt str_len
    const 3
    igt
    jz ok
    str "(long)"
    ret
ok:
    const 0
    ret
end

@Inject target="shout" at="return"
func bang(s: string, result: string): string
    load result
    str "!"
    rt str_concat
    ret
end

@Redirect target="show" rt="print"
func framed(s: string): int
    str "["
    load s
    rt str_concat
    str "]"
    rt str_concat
    rt print
    ret
end
"#;

#[test]
fn mixins_inject_redirect_and_refuse_native_targets() {
    let m = link_src(&[MIXIN_TARGETS, MIXIN_HOOKS]).unwrap();
    assert_eq!(run_capture(&m, &Host::new()), "[A!]\n[(long)]\n");
    let relinked = bvm::link(std::slice::from_ref(&m)).unwrap();
    assert_eq!(bvm::binary::encode(&relinked), bvm::binary::encode(&m), "applying mixins twice changes nothing");
    let text = disassemble(&m);
    assert!(text.contains("@Mixed by=\"guard\""), "{}", text);
    let native = "import clock 0\n@Overwrite target=\"clock\"\nfunc fake(): int\n    const 1\n    ret\nend\n";
    let err = link_src(&[native]).unwrap_err();
    assert!(err.contains("native host function"), "{}", err);
    let overwrite = "@Overwrite target=\"shout\"\nfunc quiet(s: string): string\n    load s\n    rt str_lower\n    ret\nend\n";
    let m = link_src(&[MIXIN_TARGETS, overwrite]).unwrap();
    assert_eq!(run_capture(&m, &Host::new()), "a\nabcdefgh\n");
    let missing = "@Inject target=\"nope\" at=\"head\"\nfunc h(): void\n    retv\nend\n";
    assert!(link_src(&[MIXIN_TARGETS, missing]).unwrap_err().contains("no function has that name"));
}

#[test]
fn imports_bind_to_exported_functions() {
    let plugin = "import log 1\nfunc plugin(): void\n    str \"hi\"\n    host log\n    pop\n    retv\nend\n";
    let host = "extern func plugin(): void\n@Export\nfunc log(s: string): void\n    str \"log: \"\n    load s\n    rt str_concat\n    rt print\n    pop\n    retv\nend\nfunc main()\n    call plugin\n    pop\n    retv\nend\n";
    let m = link_src(&[host, plugin]).unwrap();
    assert!(m.imports.is_empty());
    assert_eq!(run_capture(&m, &Host::new()), "log: hi\n");
}

#[test]
fn archives_bundle_modules_and_resources() {
    use bvm::archive::Archive;
    let mut a = Archive::new("demo");
    a.manifest.version = "1.2.3".into();
    a.add_module("app", assemble(APP).unwrap());
    a.add_module("greetlib", assemble(LIB).unwrap());
    let reader = "module reader\nimport resource 1\nfunc main()\n    str \"hello.txt\"\n    host resource\n    rt print\n    pop\n    retv\nend\n";
    a.add_module("reader", assemble(reader).unwrap());
    a.add_resource("hello.txt", b"hi from a resource".to_vec());
    a.manifest.main = "reader".into();
    let bytes = a.encode(true);
    assert!(bytes.starts_with(b"#!/usr/bin/env bvm\n"));
    assert!(bvm::archive::is_archive(&bytes));
    let back = Archive::decode(&bytes).unwrap();
    assert_eq!(back, a);
    let (m, host) = bvm::load_bytes(&bytes).unwrap();
    assert_eq!(m.name, "reader");
    assert_eq!(run_capture(&m, &host), "hi from a resource\n");
    let mut damaged = bytes.clone();
    let last = damaged.len() - 1;
    damaged[last] ^= 1;
    assert!(Archive::decode(&damaged).unwrap_err().contains("damaged"));
    for n in 0..bytes.len() {
        assert!(Archive::decode(&bytes[..n]).is_err());
    }
    let mut entry = a.clone();
    entry.manifest.main = "app".into();
    entry.manifest.entry = Some("main".into());
    assert!(entry.link().is_ok());
    entry.manifest.entry = Some("missing".into());
    assert!(entry.link().is_err());
}

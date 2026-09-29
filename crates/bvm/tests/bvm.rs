use bvm::asm::{assemble, disassemble};
use bvm::binary::{decode, encode};
use bvm::runtime::io;
use bvm::{verify, Cmp, Desc, FuncBuilder, Host, ModuleBuilder, Op, RtFn, Runner};
use std::process::Command;

const SAMPLE: &str = r#"
; every section and most instructions
type Shape = interface
type Point = record { x: int, y: int }
type Circle = class { center: Point, r: float } implements Shape
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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

# The Burn compiler, written in Burn

This directory holds the Burn compiler and its tools, written in Burn. They were ported from the compiler written in Rust (`crates/burn`, last released as `v26.1.0-experimental-2`) part by part, and each part produced exactly the same result as the Rust version before the next one started. Since `26.1.0-experimental-3` they are the only Burn compiler, and since `26.1.0-experimental-7` there is no Rust left at all: bvm (`src/bvm/`) and the runtime (`lib/rt`) are written in Burn as well, and the toolchain is a native executable built from the seeds in `boot/`.

## Layout

`compiler/` is a Burn project (`burn.toml`) with the standard layout. Each folder of `src/` holds one stage of the compiler:

| Folder | What it holds | Ported from (Rust) |
| --- | --- | --- |
| `src/syntax/` | the lexer, the parser and the syntax tree | `lexer.rs`, `parser.rs`, `ast.rs` |
| `src/diag/` | diagnostics and how they are printed | `diag.rs`, `source.rs` |
| `src/project/` | `burn.toml`, `burn.lock`, workspaces, paths, the module loader and the embedded standard library | `project.rs`, `loader.rs` |
| `src/check/` | the type table and the type checker | `types.rs`, `hir.rs`, `check/` |
| `src/lower/` | passes over the checked program: ownership (reference counting) | `own.rs` |
| `src/vm/` | bvm code generation and the bvm assembly text it is written as | `vm/compile.rs`, and from the bvm crate `op.rs`, `module.rs`, `builder.rs` and `asm.rs` |
| `src/dump/` | the `burn dump` text forms that the tests compare | `astdump.rs`, `loaddump.rs`, `check/declsdump.rs`, `check/hirdump.rs` |
| `src/main.bn` | the compiler on its own: `build [--target bvm\|asm\|runtime] <file.bn> [-o <out>]` and the `burn dump` forms | |
| `src/cli/` | the `burn` command line: running, checking and building programs and projects, `burni`, `burnc` and the `burn dump` forms | `main.rs`, `driver.rs`, `targets.rs` |
| `src/doc/` | Burndoc comments and the HTML sites of `burn doc` | `doc/` |
| `src/bin/gendoc.bn` | writes `src/doc/assets.bn` from `lib/doc` | |
| `src/js/` | the JavaScript backend: `burn build --target js` and `burn dump --js` | `js/mod.rs` |
| `src/native/` | the x86-64 and arm64 backends: `burn build`, `burn run --native` and `burn dump --asm` | `native/mod.rs`, `native/x86.rs`, `native/entry_x86_64.s` |
| `src/bin/genjs.bn` | writes `src/js/prelude.bn` from `lib/js/prelude.js`, the runtime of programs compiled to JavaScript | `js/prelude.js` |
| `src/bin/burn.bn` | the toolchain, `bin/burn`: the command line when it is started as `burn`, `burni`, `burnc` or `burn-lsp`, and bvm under any other name | |
| `src/lsp/` | the language server | `lsp/` |
| `src/build.bn` | loads, checks and compiles a program to bvm assembly, or to native assembly for the bootstrap | `driver.rs`, `vm/mod.rs` |
| `src/bin/genstd.bn` | writes `src/project/stdlib.bn` from `lib/std` | |
| `src/bvm/` | bvm: the bytecode format, the assembler, the verifier, the linker, mixins, archives, the interpreter, the JIT (`jit.bn`, x86-64) and the `bvm` command | `bvm/src/` |

The sources import each other with relative paths (`"../syntax/ast.bn"`) because they were first built by a stage0 that did not know `@/` imports.

## How each part was checked

The Rust compiler could print every stage in a fixed text form. The Burn port prints the same form, and until the Rust compiler was removed the test suite ran both over every `.bn` file in the repository; the two outputs had to match byte for byte. The `burn dump` forms stay, for debugging the compiler.

| Stage | Rust | Burn | Printed by |
| --- | --- | --- | --- |
| Tokens | `crates/burn/src/lexer.rs` | `src/syntax/lexer.bn` | `burn dump --tokens <files...>` |
| Syntax tree and parse errors | `crates/burn/src/parser.rs`, `ast.rs` | `src/syntax/parser.bn`, `src/syntax/ast.bn` | `burn dump --ast <files...>` |
| Rendered errors with snippets and fixes | `crates/burn/src/diag.rs`, `source.rs` | `src/diag/diag.bn` | `burn dump --diagnostics <files...>` |
| `burn.toml` and `burn.lock` as TOML | `crates/burn/src/project.rs` | `src/project/toml.bn` | `burn dump --toml <files...>` |
| Projects, workspaces and package resolution | `crates/burn/src/project.rs` | `src/project/project.bn`, `src/project/paths.bn` | `burn dump --project <paths...>` |
| Every module a program loads, and import errors | `crates/burn/src/loader.rs` | `src/project/loader.bn`, `src/project/stdlib.bn` | `burn dump --modules <files...>` |
| Declared types, functions, globals and their errors | `crates/burn/src/types.rs`, `check/` up to `declare` | `src/check/types.bn`, `src/check/check.bn`, `src/check/hir.bn` | `burn dump --decls <files...>` |
| Checked program (functions, statements and expressions after type checking) and checker errors | `crates/burn/src/check/` | `src/check/body.bn` and the files next to it | `burn dump --checked <files...>` |
| The program after ownership, with `Retain`, `Release` and their temporaries | `crates/burn/src/own.rs` | `src/lower/own.bn` | `burn dump --owned <files...>` |
| What the language server knows: hovers, definitions, locals and expression types | `crates/burn/src/check/` with `want_index` | `src/check/` with `wantIndex`, `src/dump/index.bn` | `burn dump --index <files...>` |
| The bvm module in bvm assembly | `crates/burn/src/vm/compile.rs`, `bvm/src/asm.rs` | `src/vm/compile.bn`, `src/vm/asm.bn` | `burn dump --bvm <files...>` |

Print a stage with `burn dump --tokens examples/*.bn`, or with `burn compiler/src/main.bn --tokens examples/*.bn` from source. `--project` takes folders or files and prints the project found from each. Set `BURN_HOME=tests/projects/home` to use the downloaded packages of the fixtures.

The inputs include the error-recovery fixtures in `tests/lexer` and `tests/parser`, the TOML files in `tests/toml`, and the projects and workspaces in `tests/projects`. On bvm, the Burn version lexes and parses all of the repository's Burn files in about half a second.

`src/project/stdlib.bn` holds the sources of `lib/std`, the way the Rust compiler embedded them with `include_str!`. It is generated; after changing `lib/std`, run `burn compiler/src/bin/genstd.bn > compiler/src/project/stdlib.bn` from the repository root. The suite fails when it is out of date.

## Rules the port followed

- Port faithfully: same structure, same names where Burn allows, same error messages and spans. Improvements happened in Rust first, then got ported.
- A bug found while porting was fixed in the Rust compiler in the same PR, with a fixture in `tests/`. The lexer port found two:
  - a stray non-ASCII symbol (`→`) hung the lexer;
  - an unknown escape before a multi-byte character (`"\é"`) crashed it.

  The diagnostics port found one in the checker: a pattern binding with the same name as a function (`Kind.Big(digits)` next to `fun digits`) was compared against the function instead of binding.
- Every stage gets a `burn dump` form before it is ported, so the comparison never depends on parsing human-readable output.

## Roadmap

1. **Lexer** (done). Tokens, string templates, numbers in every base and lexer errors.
2. **Parser and AST** (done).
   - `src/syntax/parser.bn` ports `parser.rs`, including:
     - recovery after syntax errors
     - splitting `>>` in generic types
     - speculative parsing
     - `@Getter`/`@Setter` expansion
   - `src/syntax/ast.bn` uses enums with data for every node kind.
3. **Diagnostics** (done).
   - `src/diag/diag.bn` ports the diagnostic type, line and column lookup, and the `-->` snippet renderer with notes, helps and fix suggestions.
   - It renders every lexer and parser error the same way `burn check` does without colours. Checker errors are compared by `burn dump --checked`.
4. **Loader and projects** (done).
   - `src/project/toml.bn` ports the TOML reader, `src/project/project.bn` the manifest, lock file, workspaces and package resolution, and `src/project/loader.bn` the module loader with `@/` imports, `mod.bn` folders, packages and the standard library.
   - Burn has no way yet to resolve symbolic links or read file times, and the boot compiler must still build these sources. Until the language adds them:
     - `src/project/paths.bn` makes paths absolute and removes `.` and `..` without following symbolic links;
     - importing a package's bytecode (`import "<package>.bvmc"`) uses the file in its `build/` folder if there is one, and building it needs the checker and code generation.
5. **Checker** (done). `src/check/` ports `check/` file by file:
   - declarations and types: `src/check/check.bn` runs everything `run` does before it checks struct bodies, and `burn dump --decls` prints the type table, every record, interface, enum, module scope, function signature and global, and the errors found so far. The fixtures in `tests/check/decls` cover each of those errors.
     Importing bytecode libraries (`check/libs.rs`) needs a reader for bvm modules, which the command line gets from bvm (see step 9); `src/main.bn` alone reports an error for each library import.
   - function bodies: `src/check/body.bn` checks every function and top-level statement, and `burn dump --checked` prints the checked program (types, functions with their statements and expressions, globals, strings, source locations and interface slots) or the errors.
     The files next to it port the matching Rust files: `expr.bn`, `calls.bn`, `builtins.bn`, `stmt.bn`, `closures.bn`, `generics.bn`, `numeric.bn` (with `wide.bn` for 128-bit constant folding), `matching.bn` (`match`), `nullsafe.bn` (`?.`, `??` and `as?`), `structs.bn` (constructors, inheritance, abstract methods, virtual calls and their devirtualization, static values, `new`, generic structs, adding functions to objects and `destroy`), `annotations.bn` (`annotationsOf` and mixins) and `init_order.bn`.
     The suite compares `burn dump --checked` on every Burn file in the repository. The fixtures in `tests/check/bodies` cover closures, generic functions, return types, initialization order, sized numbers, `match`, the nullable operators and structs.
6. **Ownership** (done). `src/lower/own.bn` ports `own.rs`: which locals own their values, retains for borrowed values that are kept, releases at the end of statements, scopes and functions, temporaries for nested calls, and in-place appends for `s += ...` on locals, globals and fields. `burn dump --owned` prints the program after the pass, and the suite compares it on every Burn file in the repository.
7. **bvm code generation** (done). `src/vm/compile.bn` ports `vm/compile.rs`, and `src/vm/asm.bn` writes the module as bvm assembly (`.bvm`), like `disassemble` in the bvm crate. Burn has no binary file output yet and stage0 cannot get one, so the Burn compiler writes the text form, which `burn <file.bvm>` assembles and runs. `burn dump --bvm` prints it, and the suite compares it on every Burn file in the repository. `src/vm/bits.bn` converts floats to and from their bits for constants, and `src/vm/runtime.bn` names the runtime functions.
8. **Bootstrap** (done, see *The boot compiler* below). `scripts/bootstrap.sh` lets the boot compiler build the compiler in `compiler/src` (stage 1), lets the result build it again until two stages are identical, and checks that. The CI job `bootstrap` runs it on every push, then lets the result compile and run `examples/fib.bn`. Until `26.1.0-experimental-4` a released Burn (stage0) started the bootstrap; the boot compiler in the repository replaced it.
   - `scripts/package.sh` bootstraps the compiler, and stage 2 builds the toolchain (`build --target asm` and `--target runtime`).
9. **The command line** (done). `src/bin/burn.bn` is `burn` written in Burn, a native executable: the command line when it is started as `burn`, `burni`, `burnc` or `burn-lsp`, and bvm (`src/bvm/`) under any other name, such as the `bvm` link next to it. `src/cli/host.bn` gives it what the system and bvm provide: paths, files, other programs, the terminal, and linking, writing and running bytecode with the bvm written in Burn. Until `26.1.0-experimental-7` the Rust bvm ran the command line as bytecode and provided these as host functions.
   - Ported: running files and projects (`run`, `--bin`, `--example`, `-p`), `check`, `build` (to `.bvmc` and `.bar`), `eval`, `test`, `fmt` (with `lib/tools/fmt.bn`), `fix`, `init`, `burni`, `burnc`, `--no-std`, `dump` and the error output with colours. The suite runs the Rust `burn` and the one written in Burn on the test programs and projects and compares stdout, stderr and the exit code.
   - `burn init` makes projects: `--target native` (the default), `js`, `bvm` or `bar`, and `--workspace` makes a `common` library with one app per target (`native,js,bvm` by default).
   - Bytecode libraries: `import "lib.bvmc"` and `import "<package>.bvmc"` work like in the Rust compiler. bvm describes each library's exported functions and types (`burn.library`), `src/check/check.bn` imports them the way `check/libs.rs` does, and bvm links them when the program runs or is written (`.bvmc` and `.bar`, with their resources). A package's bytecode is rebuilt when its sources are newer (`burn.stale`), through the `PackageBuilder` hook of the loader. Without the hooks, as when stage0 runs `src/main.bn`, library imports are still an error.
   - `burn repl` (`src/cli/repl.bn`) ports `repl.rs`. `checkRepl` checks the code typed so far again without errors or code and prints the value of the last expression, like the `skip_before` and `repl_echo` options of the Rust checker. `replEval` keeps the session's globals between inputs and runs each input with the runtime in panic mode, where `burn_rt_try` turns a runtime error or `exit` into a message instead of stopping the command line.
   - `burn doc` (`src/cli/doc.bn`) ports `doc/`: `src/doc/comment.bn` parses Burndoc comments, `src/doc/model.bn` collects what each module documents, `src/doc/html.bn` writes the pages and the search index, and `src/doc/builtin_docs.bn` reads the built-in functions. Their declarations, the search script and the style sheet live in `lib/doc/`, and `src/bin/gendoc.bn` embeds them in `src/doc/assets.bn`. The suite compares whole sites with the Rust `burn doc`.
   - The language server's index: with `wantIndex` (`checkIndexed`), the checker records the hover text of every name (with its Burndoc), where each name is defined, every local with its type and scope, and the type of every expression, like `want_index` in the Rust checker. `burn dump --index` prints it, and the suite compares it on every Burn file in the repository.
   - `burn lsp` (and `burn-lsp`) ports `lsp/`: `src/lsp/server.bn` reads the messages and checks open documents, and the files next to it port the matching Rust files: `json.bn` (JSON), `analysis.bn` (what a checked document knows), `ide.bn` (links, folding, symbols, type definitions, imports), `nav.bn` (definitions, references, highlights, renames, workspace symbols), `members.bn` and `complete.bn` (completion), `assist.bn` (signature help and inlay hints), `imports.bn` (auto-imports), `repair.bn` (closing braces while typing), `reload.bn` (`ash sync`) and `sources.bn` (the sources it shows for the standard library, built-ins and bytecode libraries, also written by `burn sources`). `src/cli/host.bn` reads and writes the messages and caps the server's memory; each message is handled in its own task, and in panic mode a runtime error fails only that request. The suite sends the same session to both servers, with requests at every position of two documents, and compares the answers byte for byte.
   - `--target js` (`src/js/js.bn`) ports `js/mod.rs`: the checked program, before the ownership pass, becomes one JavaScript file after the runtime in `lib/js/prelude.js`, which `src/bin/genjs.bn` embeds in `src/js/prelude.bn`. `burn dump --js` prints it; it was compared byte for byte with the Rust `burn dump --js` on every Burn file in the repository, and the suite runs every test program with Node.js.
   - Native executables (`src/native/`) port `native/`: `x86.bn` turns the program after the ownership pass into x86-64 assembly, `meta.bn` encodes the types and source locations the runtime reads, and `native.bn` checks what native code cannot do (`@Native` functions, mixins on native functions) and writes the launcher that runs a `.bvmc` or `.bar` as an executable. `src/cli/native.bn` links the assembly with `cc` and the runtime in `share/burn/lib`, which `scripts/native-runtime.sh` builds from `bvm/` (`libburn_runtime.a`, and `libburn_core.a` for `--no-std`), the way the Rust compiler's `build.rs` did. `burn dump --asm` prints the assembly; it was compared byte for byte with the Rust `burn dump --asm` on every Burn file in the repository, and the suite runs every test program as a native executable.
   - arm64 (`src/native/arm64.bn`) is new in Burn: the same program, with the same data layout and argument order, as 64-bit ARM assembly for Linux and macOS (Apple silicon). `shared.bn` holds what both generators use. Arguments go into a 16-byte aligned area the callee reads above its frame, temporaries are pushed in 16-byte slots, and the entry point and trampoline are written for the AAPCS64 calling convention, so the runtime calls compiled functions the same way on both processors. `BURN_PLATFORM` picks the platform to generate for. The suite cross-compiles every test program to Linux arm64 and runs it under qemu (when `qemu-aarch64` and `aarch64-linux-gnu-gcc` are installed), and CI runs the whole suite on macOS with Apple silicon.
   - `scripts/package.sh` builds it as a native executable, `bin/burn`, with the compiler the bootstrap made; the runtime it links is written in Burn in `lib/rt`.
10. **Retiring the Rust compiler** (done in `26.1.0-experimental-3`). `crates/burn` is removed. The end-to-end tests are written in Burn (`tests/run.bn`), and `scripts/test.sh` runs them against a toolchain built from the seeds.

## The seed

The compiler is written in Burn, so something has to build it the first time. That is the seed in `compiler/boot`:
the compiler (`src/main.bn`) from an earlier commit and the runtime it links (`lib/rt`), compiled to assembly for
each platform (`linux-x86_64.tar.xz`, `linux-aarch64.tar.xz`, `macos-x86_64.tar.xz`, `macos-aarch64.tar.xz`), like
`zig1.wasm` in Zig. Only a C compiler, tar and xz are needed to start: the C compiler assembles the seed into a native
compiler. No released Burn is downloaded, and no Rust is needed.

- **The bootstrap:** `scripts/bootstrap.sh` assembles the seed of this platform (stage 0), lets it build
  `compiler/src/main.bn` and `lib/rt` (stage 1), lets stage 1 build them again (stage 2), and, when the two differ
  because the code generator changed since the seed was made, lets stage 2 build them a third time (stage 3). The
  assembly of the last two stages must be identical; each stage takes a few seconds. The CI job `bootstrap` runs it on
  every push, and `scripts/package.sh` builds the toolchain with the stage it ends with.
- **The rule:** `compiler/src` may only use language features that the seed understands. To use a new one in the
  compiler, add it to the language first, then run `sh scripts/bootstrap.sh --update`, which replaces the seeds of
  every platform with the compiler just built, and commit them together with the first use.
- **Keeping it small:** each seed is about 56 MB of assembly, 1.3 MB compressed. Update the seeds only when the
  compiler's own sources need something new.

To cut a release by hand, run the **Release** workflow from the Actions tab with the tag to create. The tag must be `v` followed by the version in `src/version.bn`.

## Using it

`burn` itself is the command line written in Burn and compiled to native code, which contains the compiler. `src/main.bn` is the compiler on its own, the program the seeds hold and the bootstrap builds:

```sh
sh scripts/bootstrap.sh                                                # compiler/build/stage2 (or stage3), a native compiler
compiler/build/stage2 build examples/fib.bn -o fib.bvm                 # bvm assembly
burn fib.bvm                                                           # run it
compiler/build/stage2 build --target asm examples/fib.bn -o fib.s      # native assembly for this platform
compiler/build/stage2 build --target runtime lib/rt/runtime.bn -o rt.s # a runtime library
burn compiler/src/main.bn build examples/fib.bn -o fib.bvm             # the compiler from source, run with any burn
```

Errors and warnings print the same way as `burn check` without colours.

## The JIT

`src/bvm/jit.bn` compiles bvm code to x86-64 machine code while a program runs. Each function record counts its calls and the backward jumps of its loops; at `BURN_JIT_THRESHOLD` (1000) the interpreter hands the function to the JIT, which writes machine code from the prepared instruction records (`src/bvm/layout.bn` describes them) into memory it maps executable. Compiled code keeps the interpreter's frame, so the two call each other freely, and a loop that gets hot in the middle of a call continues in compiled code at the same instruction (on-stack replacement). Instructions without a machine code form call `burn_bvm_slow`, the function that runs them for the interpreter as well. `BURN_JIT=0` turns the JIT off; the suite runs every test program with the JIT compiling everything at once (`BURN_JIT_THRESHOLD=1`) and with it off, and compares the output with the native executables.

## Language features this relies on

These features were added to Burn for the port:

- **`s += piece` appends in place**, so building output is linear.
- **Enums whose variants carry data**, with `match` destructuring, for tokens, AST nodes and types.
- **`toInt(text, radix)`, `isInt(text, radix)` and `toString(n, radix)`** for number literals and escapes.

Numbers above the `int` range (`BigInt` tokens) are kept as decimal text, so the port needs no unsigned 64-bit arithmetic.

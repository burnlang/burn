# The Burn compiler, written in Burn

This directory holds the self-hosted Burn compiler. It is ported from the compiler in `crates/burn` part by part. Each part has to produce exactly the same result as the Rust version before the next one starts.

## Layout

`compiler/` is a Burn project (`burn.toml`) with the standard layout. Each folder of `src/` holds one stage of the compiler:

| Folder | What it holds | Ported from |
| --- | --- | --- |
| `src/syntax/` | the lexer, the parser and the syntax tree | `lexer.rs`, `parser.rs`, `ast.rs` |
| `src/diag/` | diagnostics and how they are printed | `diag.rs`, `source.rs` |
| `src/project/` | `burn.toml`, `burn.lock`, workspaces, paths, the module loader and the embedded standard library | `project.rs`, `loader.rs` |
| `src/check/` | the type table and the type checker | `types.rs`, `hir.rs`, `check/` |
| `src/lower/` | passes over the checked program: ownership (reference counting) | `own.rs` |
| `src/vm/` | bvm code generation and the bvm assembly text it is written as | `vm/compile.rs`, and from the bvm crate `op.rs`, `module.rs`, `builder.rs` and `asm.rs` |
| `src/dump/` | the `burn dump` text forms that the tests compare | `astdump.rs`, `loaddump.rs`, `check/declsdump.rs`, `check/hirdump.rs` |
| `src/main.bn` | the command line: `build <file.bn> [-o <file.bvm>]` and the `burn dump` forms | |
| `src/cli/` | the `burn` command line: running, checking and building programs and projects, `burni`, `burnc` and the `burn dump` forms | `main.rs`, `driver.rs`, `targets.rs` |
| `src/bin/burn.bn` | the program bvm runs when it is started as `burn`, `burni` or `burnc` (`share/burn/burn.bvm`) | |
| `src/build.bn` | loads, checks and compiles a program to bvm assembly | `driver.rs`, `vm/mod.rs` |
| `src/bin/genstd.bn` | writes `src/project/stdlib.bn` from `lib/std` | |
| `src/bin/genrt.bn` | writes `src/vm/runtime.bn`, the symbol of each runtime function, from `bvm/runtime/src/lib.rs` | |

The sources import each other with relative paths (`"../syntax/ast.bn"`) because stage0 does not know `@/` imports yet.

## How each part is checked

The Rust compiler can print every stage in a fixed text form. The Burn port prints the same form, and the test suite runs both over every `.bn` file in the repository. The inputs include the edge cases in `tests/lexer`, and the two outputs must match byte for byte.

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
| The bvm module in bvm assembly | `crates/burn/src/vm/compile.rs`, `bvm/src/asm.rs` | `src/vm/compile.bn`, `src/vm/asm.bn` | `burn dump --bvm <files...>` |

Run the comparison by hand with:

```sh
burn dump --tokens examples/*.bn > rust.txt
burn compiler/src/main.bn --tokens examples/*.bn > burn.txt
diff rust.txt burn.txt
```

The same works with the other stages. `--project` takes folders or files and prints the project found from each. Set `BURN_HOME=tests/projects/home` to use the downloaded packages of the fixtures. The suite test `compiler_written_in_burn_matches_the_compiler` compares every stage on bvm and on a native build.

The inputs include the error-recovery fixtures in `tests/lexer` and `tests/parser`, the TOML files in `tests/toml`, and the projects and workspaces in `tests/projects`. On bvm, the Burn version lexes and parses all of the repository's Burn files in about half a second.

`src/project/stdlib.bn` holds the sources of `lib/std`, the way the Rust compiler embeds them with `include_str!`. It is generated; after changing `lib/std`, run `burn compiler/src/bin/genstd.bn > compiler/src/project/stdlib.bn` from the repository root. The suite fails when it is out of date.

## Rules for porting

- Port faithfully: same structure, same names where Burn allows, same error messages and spans. Improvements happen in Rust first, then get ported.
- A bug found while porting is fixed in the Rust compiler in the same PR, with a fixture in `tests/`. The lexer port found two:
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
   - Burn has no way yet to resolve symbolic links or read file times, and stage0 must still build these sources. Until a later stage0 adds them:
     - `src/project/paths.bn` makes paths absolute and removes `.` and `..` without following symbolic links;
     - importing a package's bytecode (`import "<package>.bvmc"`) uses the file in its `build/` folder if there is one, and building it needs the checker and code generation.
5. **Checker** (done). `src/check/` ports `check/` file by file:
   - declarations and types: `src/check/check.bn` runs everything `run` does before it checks struct bodies, and `burn dump --decls` prints the type table, every record, interface, enum, module scope, function signature and global, and the errors found so far. The fixtures in `tests/check/decls` cover each of those errors.
     Importing bytecode libraries (`check/libs.rs`) needs a reader for bvm modules, which the command line gets from bvm (see step 9); `src/main.bn` alone reports an error for each library import.
   - function bodies: `src/check/body.bn` checks every function and top-level statement, and `burn dump --checked` prints the checked program (types, functions with their statements and expressions, globals, strings, source locations and interface slots) or the errors.
     The files next to it port the matching Rust files: `expr.bn`, `calls.bn`, `builtins.bn`, `stmt.bn`, `closures.bn`, `generics.bn`, `numeric.bn` (with `wide.bn` for 128-bit constant folding), `matching.bn` (`match`), `nullsafe.bn` (`?.`, `??` and `as?`), `structs.bn` (constructors, inheritance, abstract methods, virtual calls and their devirtualization, static values, `new`, generic structs, adding functions to objects and `destroy`), `annotations.bn` (`annotationsOf` and mixins) and `init_order.bn`.
     The suite compares `burn dump --checked` on every Burn file in the repository. The fixtures in `tests/check/bodies` cover closures, generic functions, return types, initialization order, sized numbers, `match`, the nullable operators and structs.
6. **Ownership** (done). `src/lower/own.bn` ports `own.rs`: which locals own their values, retains for borrowed values that are kept, releases at the end of statements, scopes and functions, temporaries for nested calls, and in-place appends for `s += ...` on locals, globals and fields. `burn dump --owned` prints the program after the pass, and the suite compares it on every Burn file in the repository.
7. **bvm code generation** (done). `src/vm/compile.bn` ports `vm/compile.rs`, and `src/vm/asm.bn` writes the module as bvm assembly (`.bvm`), like `disassemble` in the bvm crate. Burn has no binary file output yet and stage0 cannot get one, so the Burn compiler writes the text form, which `burn <file.bvm>` assembles and runs. `burn dump --bvm` prints it, and the suite compares it on every Burn file in the repository. `src/vm/bits.bn` converts floats to and from their bits for constants, and `src/vm/runtime.bn` (written by `src/bin/genrt.bn`) names the runtime functions.
8. **Bootstrap** (done, see *Stage0* below). `scripts/bootstrap.sh` runs the compiler in `compiler/src` on itself (stage 1), lets stage 1 build the compiler again (stage 2) and checks that both modules are identical. The CI job `bootstrap` runs it with stage0 on every push, then lets stage 2 compile and run `examples/fib.bn`.
   - `scripts/package.sh` bootstraps the compiler and installs stage 2 as `share/burn/compiler.bvm`, which `--compiler burn` uses.
9. **The command line** (in progress). `src/bin/burn.bn` is `burn` written in Burn. bvm runs it when it is started as `burn`, `burni` or `burnc` (a link to `bvm` with that name), from `share/burn/burn.bvm` next to its `bin/` or from the module in `BURN_CLI_BVM`. It reaches bvm through host functions (`@Native("burn.runModule")` and the others in `src/cli/host.bn`), which bvm provides only in that mode. That keeps the compiler in `src/main.bn` free of them, so stage0 can still build it and run it.
   - Ported: running files and projects (`run`, `--bin`, `--example`, `-p`), `check`, `build` (to `.bvmc` and `.bar`), `eval`, `test`, `fmt` (with `lib/tools/fmt.bn`), `fix`, `init`, `burni`, `burnc`, `--no-std`, `dump` and the error output with colours. The suite runs the Rust `burn` and the one written in Burn on the test programs and projects and compares stdout, stderr and the exit code.
   - `burn init` makes bvm projects: `--target bvm` (the default) or `--target bar`, and `--workspace` makes a `common` library with one app per target (`bvm,bar` by default).
   - Bytecode libraries: `import "lib.bvmc"` and `import "<package>.bvmc"` work like in the Rust compiler. bvm describes each library's exported functions and types (`burn.library`), `src/check/check.bn` imports them the way `check/libs.rs` does, and bvm links them when the program runs or is written (`.bvmc` and `.bar`, with their resources). A package's bytecode is rebuilt when its sources are newer (`burn.stale`), through the `PackageBuilder` hook of the loader. Without the hooks, as when stage0 runs `src/main.bn`, library imports are still an error.
   - `burn repl` (`src/cli/repl.bn`) ports `repl.rs`. `checkRepl` checks the code typed so far again without errors or code and prints the value of the last expression, like the `skip_before` and `repl_echo` options of the Rust checker. bvm keeps the session's globals between inputs (`burn.replEval`), turns runtime errors into messages, and gives the command line its own type table back after each input.
   - Next: `sources`, `doc` and `lsp`. It builds bvm bytecode only: the native x86-64 and JavaScript backends stay behind with the Rust compiler.
   - `scripts/package.sh` builds it with `compiler.bvm` and installs it as `share/burn/burn.bvm`. The runtime core (`bvm/runtime`) stays in Rust and is shared by both compilers; the rest of the runtime is written in Burn in `lib/runtime`.

## Stage0: the last compiler written in Rust

`compiler/STAGE0` names a release of the Rust compiler, currently `v26.1.0-experimental-2`. It is the bootstrap compiler: it builds the Burn compiler for the first time, and nothing after it needs Rust to compile Burn code.

- **The action:** `.github/actions/stage0` downloads that release for the runner's platform, checks it against the release's `SHA256SUMS`, and puts its `burn` on `PATH`. It runs on Linux x86-64 and macOS (Intel and Apple silicon).
- **The guard:** the CI job `stage0` builds the Burn compiler in `compiler/` with that release on every push. So the sources here may only use language features that stage0 already understands. To use a newer feature in the compiler, cut a new release first, then move `compiler/STAGE0` to it.
- **The bootstrap:** the CI job `bootstrap` runs `scripts/bootstrap.sh` with stage0. Stage0 runs the compiler in `compiler/src`, which builds itself (`stage1.bvm`); stage0's bvm runs `stage1.bvm`, which builds the compiler again (`stage2.bvm`); the two must be identical. Because stage 1 is already the compiler written in Burn, this is the same check as comparing stage 2 with stage 3. The release will ship `stage2.bvm`. From then on, each release can bootstrap from the previous Burn release instead of stage0. The Rust compiler crate can then be retired. The runtime core (`bvm/runtime`) stays in Rust.

To cut a release by hand, run the **Release** workflow from the Actions tab with the tag to create. The tag must be `v` followed by the workspace version.

## Using it

The compiler runs on bvm. Run it with any `burn`, or build it once and run the module:

Toolchains ship the bootstrapped compiler as `share/burn/compiler.bvm`, and `--compiler burn` makes `burn run`, `burn check` and `burn build --target bvm` use it. `BURN_COMPILER_BVM` points `burn` at another module, such as `compiler/build/stage2.bvm`.

```sh
burn run --compiler burn examples/fib.bn                     # compile with the shipped Burn compiler, run on bvm
burn compiler/src/main.bn build examples/fib.bn -o fib.bvm   # compile with the Burn compiler from source
burn fib.bvm                                                 # run the result on bvm
sh scripts/bootstrap.sh                                      # build compiler/build/stage2.bvm
burn compiler/build/stage2.bvm build examples/fib.bn         # the compiler built by itself
```

It writes bvm assembly (`.bvm`) rather than binary `.bvmc`, because Burn cannot write binary files yet. Errors and warnings print the same way as `burn check` without colours.

## Language features this relies on

These features were added to Burn for the port:

- **`s += piece` appends in place**, so building output is linear.
- **Enums whose variants carry data**, with `match` destructuring, for tokens, AST nodes and types.
- **`toInt(text, radix)`, `isInt(text, radix)` and `toString(n, radix)`** for number literals and escapes.

Numbers above the `int` range (`BigInt` tokens) are kept as decimal text, so the port needs no unsigned 64-bit arithmetic.

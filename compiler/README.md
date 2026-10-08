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
| `src/dump/` | the `burn dump` text forms that the tests compare | `astdump.rs`, `loaddump.rs`, `check/declsdump.rs` |
| `src/main.bn` | the entry point; for now it prints the `burn dump` forms | |
| `src/bin/genstd.bn` | writes `src/project/stdlib.bn` from `lib/std` | |

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
   - It renders every lexer and parser error the same way `burn check` does without colours. Checker errors are compared once the checker is ported.
4. **Loader and projects** (done).
   - `src/project/toml.bn` ports the TOML reader, `src/project/project.bn` the manifest, lock file, workspaces and package resolution, and `src/project/loader.bn` the module loader with `@/` imports, `mod.bn` folders, packages and the standard library.
   - Burn has no way yet to resolve symbolic links or read file times, and stage0 must still build these sources. Until a later stage0 adds them:
     - `src/project/paths.bn` makes paths absolute and removes `.` and `..` without following symbolic links;
     - importing a package's bytecode (`import "<package>.bvmc"`) uses the file in its `build/` folder if there is one, and building it needs the checker and code generation.
5. **Checker.** The largest part (about 10,000 lines). Split it into PRs, roughly one per Rust file:
   - declarations and types (done): `src/check/check.bn` runs everything `run` does before it checks struct bodies, and `burn dump --decls` prints the type table, every record, interface, enum, module scope, function signature and global, and the errors found so far. The fixtures in `tests/check/decls` cover each of those errors.
     Importing bytecode libraries (`check/libs.rs`) needs a reader for bvm modules and is not ported yet; the Burn checker reports an error for each library import.
   - function bodies (done for programs without the parts listed below): `src/check/body.bn` checks every function and top-level statement, and `burn dump --checked` prints the checked program (types, functions with their statements and expressions, globals, strings, source locations and interface slots) or the errors.
     The files next to it port the matching Rust files: `expr.bn`, `calls.bn`, `builtins.bn`, `stmt.bn`, `closures.bn`, `generics.bn`, `numeric.bn` (with `wide.bn` for 128-bit constant folding), `structs.bn`, `matching.bn` (`match`), `nullsafe.bn` (`?.`, `??` and `as?`) and `init_order.bn`.
     Parts that are not ported yet make the Burn checker print `not ported yet: ...` instead of a program: `annotationsOf` and mixins.
     The suite compares every Burn file in the repository that is fully ported, and `tests/check/checked.txt` lists the files that must stay ported. The fixtures in `tests/check/bodies` cover closures, generic functions, return types, initialization order, sized numbers, `match`, the nullable operators and structs.
   - `match` and the nullable operators `?.`, `??` and `as?` (done)
   - structs and interfaces (done): constructors, inheritance, abstract methods, virtual calls and their devirtualization, static values, `new`, generic structs, adding functions to objects and `destroy`
   - annotations

   Compare `burn dump --checked`.
6. **Ownership.** Port `own.rs` and compare the HIR after ownership.
7. **bvm code generation.** Port `vm/compile.rs` and compare `burn dump --bytecode`.
8. **Bootstrap** (see *Stage0* below).
   - The Rust compiler builds the Burn compiler (stage 1).
   - Stage 1 builds itself (stage 2), and CI checks that both produce identical bytecode.
   - From then on, the Burn compiler can be chosen at the command line.
9. **After that:** the native x86-64 and JavaScript backends, then the tools (`fmt`, `doc`, `lsp`). The runtime (`crates/burn-runtime`) stays in Rust and is shared by both compilers.

## Stage0: the last compiler written in Rust

`compiler/STAGE0` names a release of the Rust compiler, currently `v26.1.0-experimental-2`. It is the bootstrap compiler: it builds the Burn compiler for the first time, and nothing after it needs Rust to compile Burn code.

- **The action:** `.github/actions/stage0` downloads that release for the runner's platform, checks it against the release's `SHA256SUMS`, and puts its `burn` on `PATH`. It runs on Linux x86-64 and macOS (Intel and Apple silicon).
- **The guard:** the CI job `stage0` builds the Burn compiler in `compiler/` with that release on every push. So the sources here may only use language features that stage0 already understands. To use a newer feature in the compiler, cut a new release first, then move `compiler/STAGE0` to it.
- **After the switch:** once the port is complete, the release workflow will build:
  1. `stage1` = stage0 compiling `compiler/`
  2. `stage2` = `stage1` compiling `compiler/`
  3. `stage3` = `stage2` compiling `compiler/`

  It checks that `stage2` and `stage3` produce identical bytecode and ships `stage2`. From then on, each release can bootstrap from the previous Burn release instead of stage0. The Rust compiler crate can then be retired. The runtime (`crates/burn-runtime`) stays in Rust.

To cut a release by hand, run the **Release** workflow from the Actions tab with the tag to create. The tag must be `v` followed by the workspace version.

## Language features this relies on

These features were added to Burn for the port:

- **`s += piece` appends in place**, so building output is linear.
- **Enums whose variants carry data**, with `match` destructuring, for tokens, AST nodes and types.
- **`toInt(text, radix)`, `isInt(text, radix)` and `toString(n, radix)`** for number literals and escapes.

Numbers above the `int` range (`BigInt` tokens) are kept as decimal text, so the port needs no unsigned 64-bit arithmetic.

# The Burn compiler, written in Burn

This directory holds the self-hosted Burn compiler. It is ported from the compiler in `crates/burn` part by part. Each part has to produce exactly the same result as the Rust version before the next one starts.

## How each part is checked

The Rust compiler can print every stage in a fixed text form. The Burn port prints the same form, and the test suite runs both over every `.bn` file in the repository. The inputs include the edge cases in `tests/lexer`, and the two outputs must match byte for byte.

| Stage | Rust | Burn | Printed by |
| --- | --- | --- | --- |
| Tokens | `crates/burn/src/lexer.rs` | `compiler/lexer.bn` | `burn dump --tokens <files...>` |
| Syntax tree and parse errors | `crates/burn/src/parser.rs`, `ast.rs` | `compiler/parser.bn`, `compiler/ast.bn` | `burn dump --ast <files...>` |
| Rendered errors with snippets and fixes | `crates/burn/src/diag.rs`, `source.rs` | `compiler/diag.bn` | `burn dump --diagnostics <files...>` |

Run the comparison by hand with:

```sh
burn dump --tokens examples/*.bn > rust.txt
burn compiler/dump.bn --tokens examples/*.bn > burn.txt
diff rust.txt burn.txt
```

The same works with `--ast` and `--diagnostics`. The suite test `compiler_written_in_burn_matches_the_compiler` compares every stage on bvm and on a native build.

The inputs include the error-recovery fixtures in `tests/lexer` and `tests/parser`. On bvm, the Burn version lexes and parses all of the repository's Burn files in about half a second.

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
   - `compiler/parser.bn` ports `parser.rs`, including:
     - recovery after syntax errors
     - splitting `>>` in generic types
     - speculative parsing
     - `@Getter`/`@Setter` expansion
   - `compiler/ast.bn` uses enums with data for every node kind.
3. **Diagnostics** (done).
   - `compiler/diag.bn` ports the diagnostic type, line and column lookup, and the `-->` snippet renderer with notes, helps and fix suggestions.
   - It renders every lexer and parser error the same way `burn check` does without colours. Checker errors are compared once the checker is ported.
4. **Loader and projects.**
   - Port imports, the standard library modules (read from `lib/std`), `burn.toml` and `burn.lock` parsing, and package resolution.
   - Include the project layout (`@/` imports, source roots, `mod.bn`, `src/bin`) and workspaces.
5. **Checker.** The largest part (about 10,000 lines). Split it into PRs, roughly one per Rust file:
   - declarations and types
   - expressions
   - statements and flow narrowing
   - structs and interfaces
   - generics
   - closures
   - `match` and enums
   - numbers
   - annotations
   - no-std rules

   Compare `burn dump --hir`.
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

# Changelog

## Unreleased (26.1.0-experimental-3)

The release that ships the compiler written in Burn.

- Projects have a standard layout:
  - `src/` is the source root, and its folders are packages.
  - `src/bin/` holds extra programs (`burn run --bin`), `tests/` holds tests (`burn test`), and `examples/` holds examples (`burn run --example`).
  - `@/path` imports from the project's source root, and `<package>/path` imports from another package's `src/`.
  - A folder with a `mod.bn` can be imported as a module.
- Workspaces: a `[workspace]` with `members` builds several projects together, like Gradle multi-projects or Cargo workspaces.
  - `burn init <name> --workspace` creates the multi-target layout: a `common` library and one app per target (native, js, bvm).
  - Members are named under the workspace (`github.com/you/game/common`), import each other directly and share one `burn.lock`.
  - At the root, `burn build`/`check`/`test` cover every member, and `-p <member>` picks one.
- Self-hosting: the lexer, the parser, the diagnostic renderer, the `burn.toml` reader, the module loader the declaration pass of the checker and the checking of function bodies are ported to Burn (`compiler/`). They produce output identical to the Rust compiler's, checked by `burn dump --tokens`, `--ast`, `--diagnostics`, `--toml`, `--project`, `--modules`, `--decls` and `--checked` on every Burn file in the repository and on project fixtures. The body checker does not cover structs, generic structs, `match`, `?.`/`??`/`as?`, `annotationsOf` and mixins yet; programs that use them are left out of the `--checked` comparison.
- Fixed: when several names were equally close to a misspelled one, "did you mean" (and `burn fix`) picked one at random on each run; it now picks the first in alphabetical order.
- Fixed: the "used before it is initialized" error could name a different chain of calls on each run when functions were called through values, and a use after `destroy` could point at a different `destroy` when there were several; both are now the same on every run.
- Fixed: a pattern binding named like a function (`Kind.Big(digits)` with a `fun digits` in scope) was compared against the function instead of binding a new name.
- Fixed: a stray non-ASCII symbol made the lexer loop forever, and an unknown escape before a multi-byte character crashed it.

## 26.1.0-experimental-2

The last release whose compiler is written in Rust. It is the stage0 compiler that bootstraps the compiler written in Burn (`compiler/STAGE0`).

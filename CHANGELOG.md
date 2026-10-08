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
- Self-hosting: the lexer, the parser, the diagnostic renderer, the `burn.toml` reader and the module loader are ported to Burn (`compiler/`). They produce output identical to the Rust compiler's, checked by `burn dump --tokens`, `--ast`, `--diagnostics`, `--toml`, `--project` and `--modules` on every Burn file in the repository and on project fixtures.
- Fixed: a pattern binding named like a function (`Kind.Big(digits)` with a `fun digits` in scope) was compared against the function instead of binding a new name.
- Fixed: a stray non-ASCII symbol made the lexer loop forever, and an unknown escape before a multi-byte character crashed it.

## 26.1.0-experimental-2

The last release whose compiler is written in Rust. It is the stage0 compiler that bootstraps the compiler written in Burn (`compiler/STAGE0`).

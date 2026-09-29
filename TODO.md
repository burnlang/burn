# TODO

## Done

- Fix Date: the date library is now written in Burn and fully working
- Fix compilation to executable: `burn build` produces real native x86-64 executables
- Add interfaces: `def interface`, checked conformance, dynamic dispatch and smart casts
- Add better documentation using MDX: see `docs/`
- Add compiling to JS: `burn build --target js` covers the whole language and standard library
- Errors show up on line 1 even though the error is on another line: every diagnostic now has an exact line and column
- Finish the VSCode Burn LSP: `burn lsp` plus the extension in `editors/vscode`
- Add private and public for imports: `pub` and `priv`
- Add async: `async fun`, `Future<T>` and `await`
- Toolchain installer with `burni`, `burnc`, `burnfmt` and `burn-lsp`
- Formatter written in Burn (`tools/burnfmt`)

## Next

- Native backends for Windows (PE/COFF) and ARM64
- Closures that capture local variables
- Generics for user-defined types and functions
- `when`/`match` expressions
- Precise (non-conservative) garbage collection and generational allocation
- Package manager
- Self-hosting (the formatter is the first tool written in Burn)
- Windows installer

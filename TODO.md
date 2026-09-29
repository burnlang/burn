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
- bvm: a general-purpose virtual machine with an assembler, bytecode format, verifier, host functions and an example language (Ember)
- bvm: linking, `.bar` archives, mixins (`@Inject`, `@Overwrite`, `@Redirect`) and a native bridge
- Annotations: `def annotation`, `@Getter`, `@Setter`, `@Deprecated`, `@Export`, `@Native`, `annotationsOf`
- Bytecode libraries in Burn (`import "lib.bvmc"`) on bvm, in archives and in native executables

## Next

- Native backends for Windows (PE/COFF) and ARM64
- Closures that capture local variables
- Generics for user-defined types and functions
- `when`/`match` expressions
- Precise (non-conservative) garbage collection and generational allocation
- Package manager
- Self-hosting (the formatter is the first tool written in Burn)
- Windows installer
- bvm: a JIT or threaded-code dispatch and a debugger
- Annotations on parameters and reflection for functions and fields
- Compile a Burn program as several bytecode modules, one per source file
- More languages on bvm (a Burn-to-bvm compiler written in Burn, a Lisp)

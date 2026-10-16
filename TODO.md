# TODO

## Done

- Fix Date: the date library is now written in Burn and fully working
- Fix compilation to executable: `burn build` produces real native x86-64 and arm64 executables (Linux and macOS, Apple silicon included)
- Add interfaces: `def interface`, checked conformance, dynamic dispatch and smart casts
- Add better documentation using MDX: see [burnlang/docs](https://github.com/burnlang/docs) and the website at https://s42yt.github.io/burn-docs/
- Add compiling to JS: `burn build --target js` covers the whole language and standard library
- Errors show up on line 1 even though the error is on another line: every diagnostic now has an exact line and column
- Finish the VSCode Burn LSP: `burn lsp` plus the extension in [vscode-burn](https://github.com/burnlang/vscode-burn)
- Add private and public for imports: `pub` and `priv`
- Add async: `async fun`, `Future<T>` and `await`
- Toolchain installer with `burni`, `burnc`, `burnfmt` and `burn-lsp`
- Formatter written in Burn (`lib/tools/fmt.bn`)
- bvm: a general-purpose virtual machine with an assembler, bytecode format, verifier, host functions and an example language (Ember)
- bvm: linking, `.bar` archives, mixins (`@Inject`, `@Overwrite`, `@Redirect`) and a native bridge
- Annotations: `def annotation`, `@Getter`, `@Setter`, `@Deprecated`, `@Export`, `@Native`, `annotationsOf`
- Bytecode libraries in Burn (`import "lib.bvmc"`) on bvm, in archives and in native executables
- Package manager: ash, `burn init`, `burn.toml`, `burn.lock` and package imports; burnup installs the toolchain
- Closures that capture local variables
- Generics for functions, `def type` and `def struct`
- `match` expressions and statements
- Bit operators (`& | ^ ~ << >> >>>`)
- Automatic memory management without a garbage collector: compiler-inserted reference counting with a cycle collector
- Sized number types (`int8` to `uint64`, `float32`) with packed arrays
- Building without the standard library (`--no-std`, `std = false`)
- Self-hosting: the compiler, the command line, `burn doc`, the REPL and the language server are written in Burn and the compiler builds itself
- No Rust: bvm and the runtime are written in Burn too, the toolchain is a native executable, and Burn builds from assembly seeds with only a C compiler
- Fast builds: the toolchain builds itself in about 25 seconds and the test suite runs in under 2 minutes
- `unsafe`, inline assembly for every target, raw memory (`std/memory`) and calling C both ways (`@Extern`)
- Import aliases (`import "std/memory" as Mem`) and imports inside functions and blocks
- bvm: a JIT that compiles hot functions and loops to x86-64 machine code, with on-stack replacement, values in registers and inline arithmetic, arrays and reference counting

## Next

- Native backends for Windows (PE/COFF)
- bvm: the JIT on arm64, floats in registers and inline method calls
- Generic methods, generic interfaces, and generic structs that extend other structs
- Windows installer
- bvm: a debugger
- Annotations on parameters and reflection for functions and fields
- Compile a Burn program as several bytecode modules, one per source file
- More languages on bvm (a Lisp)

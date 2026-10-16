<p align="center">
    <img src="assets/logo.svg" alt="Burn logo" width="160">
</p>

<h1 align="center">Burn</h1>

Burn is a statically typed programming language with type inference and smart casts. You run programs directly while
you work on them, and build them into native executables, bytecode or JavaScript when you ship them. Burn is written
in Burn.

Current version: **26.1.0-experimental-9**. Burn is not ready for production yet, and its syntax may still change.

```burn
def interface Shape {
    fun area(): float
}

def struct Circle(radius: float) :: Shape {
    fun area(): float {
        return 3.14159 * radius * radius
    }
}

fun describe(value: any): string {
    if (value is Shape) {
        return "a shape with area ${value.area()}"
    }
    return "something else"
}

fun main() {
    print(describe(new Circle(2)), describe("text"))
}
```

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/burnlang/burnup/master/install.sh | sh
```

This installs [burnup](https://github.com/burnlang/burnup), the toolchain and [ash](https://github.com/burnlang/ash),
the package manager, into `~/.burn/bin`. Burn runs on Linux and macOS, on x86-64 and arm64. Building native
executables needs a C compiler (`cc`).

## Use

```sh
burni app.bn                    # run a program
burni                           # start the REPL
burnc app.bn                    # build a native executable ./app
burnc app.bn --target js        # or JavaScript, bytecode (bvm) or a runnable archive (bar)
burnfmt -w app.bn               # format a file
ash init                        # start a project
ash install github.com/you/lib  # add a package
```

Editors get completion, errors, hover and go to definition from `burn lsp`; there are extensions for
[VS Code](https://github.com/burnlang/vscode-burn), [IntelliJ](https://github.com/burnlang/intellij-burn) and
[Zed](https://github.com/burnlang/zed-burn).

## The language

- Types are inferred, and checks such as `x is string` or `x != null` narrow a variable's type (smart casts).
  `T?` marks a value that can be `null`.
- `def type`, `def struct`, `def interface`, `def enum` and `def annotation` define every kind of type. Structs have
  constructors, methods, inheritance and static members.
- Generics, `match`, closures, `async` / `await` on threads, sized numbers (`uint8`, `float32`, …) and checked
  integer overflow.
- Memory is freed by reference counting that the compiler inserts, with a collector for cycles; there is no garbage
  collector pause and nothing to free by hand.
- `unsafe` code can use raw memory, inline assembly and C functions (`@Extern`).
- The standard library covers strings, collections, math, files, processes, dates and times, JSON and HTTP.

## How it runs

`burni` compiles a program to bytecode for bvm, the Burn virtual machine, and runs it right away. bvm interprets code
at first and compiles functions that run often to x86-64 or arm64 machine code, so hot code runs close to native
speed. `burnc` compiles a program to native code instead, or to JavaScript.

The compiler, the toolchain, bvm and the runtime are all written in Burn. To build Burn from source you only need a C
compiler: the repository keeps the compiler as assembly (`compiler/boot`), which builds the current sources.

## Documentation

The documentation is at **[s42yt.github.io/burn-docs](https://s42yt.github.io/burn-docs/)**: the language, the
standard library, the tools, bvm, how Burn works inside, and the
[changelog](https://s42yt.github.io/burn-docs/changelog). Its sources are in
[burnlang/docs](https://github.com/burnlang/docs).

## Repository

| Folder | Holds |
| --- | --- |
| `compiler/` | the compiler, the command line, bvm, the language server and `burn doc` ([compiler/README.md](compiler/README.md)) |
| `lib/std/` | the standard library |
| `lib/rt/` | the runtime of compiled programs and bvm |
| `lib/tools/` | the formatter and other tools written in Burn |
| `tests/` | test programs and the end-to-end tests |
| `examples/` | example programs, and Ember, a small language built on bvm |
| `scripts/` | building, testing and bootstrapping the toolchain |

```sh
sh scripts/package.sh --prefix ~/.burn   # build and install the toolchain from this checkout
sh scripts/test.sh                       # build a toolchain and run every test
```

Contributions are welcome: open an issue or a pull request, and add a test in `tests/cases/` for new language
features.

## License

GPL-3.0, see [LICENSE](LICENSE).

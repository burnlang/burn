<p align="center">
    <img src="assets/logo.svg" alt="Burn logo" width="160">
</p>

<h1 align="center">Burn</h1>

Burn is an easy-to-use, statically typed, general-purpose programming language with smart casts.
Burn is written in **Burn**: the compiler, the command line, the formatter, the documentation generator and the
language server are Burn programs compiled to native code, and **bvm**, the Burn virtual machine, is written in Burn
too. No other language is involved: the toolchain only links the C library, and a C compiler assembles the first
build. Programs run instantly on bvm during development, where a JIT compiles hot code to machine code, and compile to
small **native executables**, portable bytecode, self-contained `.bar` archives or JavaScript for shipping.

Current version: **26.1.0-experimental-7**

> [!WARNING]
> Burn is **not** ready for production. Syntax may still change. Please report bugs as issues.

```burn
def interface Shape {
    fun area(): float
}

def struct Circle(radius: float) :: Shape {
    fun area(): float {
        return 3.14159 * radius * radius
    }
}

def type Point {
    float x
    float y
}

fun describe(value: any): string {
    if (value is Shape) {
        return "a shape with area ${value.area()}"
    }
    if (value is string) {
        return "a string of length ${value.length}"
    }
    return "something else"
}

fun main() {
    String name = "Burn"
    var p = Point { x: 1, y: 2 }
    print(describe(new Circle(2)), describe(name), p)
}
```

## Features

- Static types with inference, so you rarely write them
- Sized numbers for speed and memory: `uint8`, `int16`, `uint32`, `uint64`, `float32` and more, with packed arrays
  (a `[uint8]` uses 1 byte per element) and checked overflow
- Smart casts: `is` checks, `!= null` checks, early returns and assignments narrow types automatically
- Null safety with `T?`
- Type-first declarations: `String name = "Burn"`, `[int] ids = []`
- One definition syntax for everything: `def type`, `def struct`, `def interface`, `def enum`, `def annotation`
- Annotations like Java's: `@Getter`, `@Setter`, `@Deprecated`, your own `def annotation`s and `annotationsOf(value)`
- Structs with constructors, methods, static and private members, inheritance, abstract and static structs, per-object functions and `destroy`; interfaces with checked conformance
- Generics: `fun first<T>(items: [T]): T?`, `def struct Stack<T>()`, with the types worked out for you
- `match` on values, enum variants, ranges and types, with guards
- Arrays, maps, records, enums, first-class functions and closures, string templates, bit operators
- Automatic memory without a garbage collector: the compiler tracks ownership and frees each value as soon as it is no
  longer used, cycles included, with no pauses and nothing to write by hand
- `async fun` / `await` running on real threads
- Tiny executables without the standard library: `std = false` in burn.toml or `--no-std`
- Low-level code when you need it: `unsafe` blocks and functions, inline assembly with a section per target
  (x86-64, arm64, bvm, JavaScript) and an `else` fallback in Burn, and raw memory with `std/memory`
- Four ways to ship sharing one type checker and runtime: native x86-64 and arm64 (Apple silicon), bvm bytecode, `.bar` archives, JavaScript
- Modules with `pub` and `priv`, import aliases (`import "std/memory" as Mem`) and imports inside functions and blocks
- Standard library for dates, times, HTTP, JSON, math, strings, processes and files
- Projects and packages: `burn init github.com/you/app`, `burn.toml`, `burn.lock` and ash, the package manager
- A self-hosted toolchain: the compiler, the command line, the runtime and bvm are written in Burn, and the compiler
  builds itself, starting from a copy kept in the repository as assembly, so building Burn only needs a C compiler
- bvm, a general-purpose virtual machine with its own assembly language, bytecode format and verifier, which other languages can target too
- A JIT in bvm: functions and loops that run often are compiled to x86-64 machine code while the program runs, so
  `burni` gets close to native speed on hot code
- Write once, run everywhere: `.bar` archives bundle an application's bytecode and resources and run wherever bvm runs
- Mixins: `@Inject`, `@Overwrite` and `@Redirect` rewrite existing bytecode functions when modules are linked
- Native interop: Burn calls C functions and C calls Burn functions (`@Extern`), and runtime libraries written in Burn link into native programs
- Precise error messages with line, column and suggestions
- Built-in REPL, formatter and language server, plus a VS Code extension

## Installation

```sh
curl -fsSL https://raw.githubusercontent.com/burnlang/burnup/master/install.sh | sh
```

[burnup](https://github.com/burnlang/burnup), the Burn version manager, puts the toolchain and ash into `~/.burn/bin`
and adds it to your `PATH`. It installs and switches versions (`burnup install 26.1`, `burnup default master`), and
a project can pin its version with `burn = "26.1"` in `burn.toml`. It builds from source when no
prebuilt release is available, which needs a C compiler, tar and xz. Native executables need a C toolchain (`cc`) and are built for Linux and macOS on x86-64 and arm64, Apple silicon Macs included.
See [installation](https://burnlang.github.io/docs/tooling/installation) for all options, updating and uninstalling.

## The toolchain

| Command | What it does |
| --- | --- |
| `burni` | the interpreter: runs programs instantly on bvm (with its JIT), starts the REPL without arguments |
| `burnc` | the compiler: standalone native executables, bvm bytecode with `--target bvm`, a runnable archive with `--target bar`, JavaScript with `--target js` |
| `burnfmt` | the code formatter, written in Burn itself |
| `burn-lsp` | the language server for editors |
| `bvm` | the Burn virtual machine: runs, assembles, disassembles and verifies bytecode |
| `ash` | the package manager, written in Burn itself ([burnlang/ash](https://github.com/burnlang/ash)) |
| `burn` | all of the Burn commands as subcommands |

```sh
burni app.bn                    # run instantly
burni                           # REPL
burni -e 'print(6 * 7)'         # run a snippet
burnc app.bn                    # standalone executable ./app
burnc app.bn -o bin/app --emit-asm app.s
burnc app.bn --no-std           # a small executable without the standard library
burnc app.bn --target bvm       # portable bytecode app.bvmc
burnc app.bn --target bar       # a runnable archive app.bar
burnc app.bn --target js        # app.js for Node.js
burn app.bvmc                   # run bytecode (or bvm app.bvmc)
burn run --native app.bn        # compile to machine code and run
burnc --check app.bn            # type-check only
burnfmt -w app.bn               # format in place
burn init github.com/you/app    # start a project with burn.toml
ash install github.com/you/lib  # add a package
```

See [the toolchain](https://burnlang.github.io/docs/tooling/toolchain) and the [command line reference](https://burnlang.github.io/docs/tooling/cli).

## Language at a glance

### Variables and types

```burn
var count = 3
var ratio: float = 2.5
String name = "Burn"
const LIMIT = 10
string? nickname = null
[int] numbers = [1, 2, 3]
{string: int} ages = {"ada": 36}
```

### Functions

```burn
fun add(a: int, b: int): int {
    return a + b
}

fun square(x: float) {
    return x * x
}

var triple = fun(x: int): int {
    return x * 3
}
```

### Definitions

```burn
def type Person {
    name: string,
    age: int
}

def type UserId = int

def enum Color { Red, Green, Blue }

def interface Greeter {
    fun greet(): string
}

def struct Human(name: string) :: Greeter {
    fun greet(): string {
        return "Hello, " + name
    }
}
```

### Control flow

```burn
if (count > 2) {
    print("many")
} else {
    print("few")
}

for i in 0..3 {
    print(i)
}

for i, n in numbers {
    print(i, n)
}

while (count > 0) {
    count -= 1
}
```

### Imports

```burn
import "std/date"
import (
    "http"
    "utils.bn"
)

print(Date.today())
```

### Annotations

```burn
def annotation Route {
    string path
    string method = "GET"
}

@Route("/accounts", method: "POST")
@Getter
@Setter
def struct CreateAccount(owner: string) {
    int balance = 0
}

var c = new CreateAccount("Ada")
c.setBalance(100)
for a in annotationsOf(c) {
    if (a is Route) {
        print(a.method, a.path, c.getBalance())
    }
}
```

### Async

```burn
import "std/time"

async fun fetch(n: int): int {
    sleep(100)
    return n * 2
}

fun main() {
    var a = fetch(1)
    var b = fetch(2)
    print(await a + await b)
}
```

## bvm, the Burn Virtual Machine

bvm runs Burn programs, and it isn't tied to Burn. It has a readable assembly language, a compact bytecode format,
a verifier, a reference-counted runtime with about 100 built-in functions, host functions and threads, and it is
written in Burn (`compiler/src/bvm/`): a language written in Burn can build modules with `compiler/src/vm/module.bn`
and run them, like Ember does. That's everything a new language needs for a backend.

```bvm
func main()
    local i
loop:
    load i
    const 3
    ilt
    jz done
    str "hello from bvm"
    rt print
    pop
    load i
    const 1
    iadd
    store i
    jmp loop
done:
    retv
end
```

```sh
bvm hello.bvm                   # assemble, verify and run
bvm asm hello.bvm -o hello.bvmc # bytecode
bvm dis hello.bvmc              # and back
```

[`examples/bvm/ember/ember.bn`](examples/bvm/ember/ember.bn) is a complete small language built on bvm, written in Burn.

### The JIT

bvm interprets code at first and counts how often each function is called and each loop turns. Once that reaches
1000, it compiles the function to x86-64 machine code and keeps running it there; a loop that gets hot in the middle of
a call moves into the compiled code without starting over (on-stack replacement). Interpreted and compiled functions
call each other freely, and runtime errors, `exit`, the REPL and stack overflows behave the same in both.

| Program | Interpreted | With the JIT | Native |
| --- | --- | --- | --- |
| `fib(32)` | 1.4 s | 45 ms | 20 ms |
| 30 million loop turns at the top level | 3.7 s | 0.1 s | 0.08 s |

`BURN_JIT=0` turns the JIT off, and `BURN_JIT_THRESHOLD=<n>` sets how many calls or loop turns make code hot. On
arm64, bvm interprets.

### Libraries, archives, mixins and native code

```burn
import "geometry.bvmc"

@Export
fun hostName(): string {
    return "the app"
}

@Inject(target: "describe", at: "return")
fun bracket(p: Point, result: string): string {
    return "<" + result + ">"
}

fun main() {
    print(describe(Point { x: 3, y: 4 }), greeting())
}
```

```sh
burnc geometry.bn --target bvm              # a bytecode library
burni app.bn                                # run it on bvm
burnc app.bn -o app                         # a native executable with the library embedded
burnc app.bn --target bar -o app.bar        # one archive: ./app.bar runs anywhere bvm runs
```

The library calls `hostName()` back in the program, and the program's mixin rewrites the library's `describe`,
both on bvm and in the native executable. See [the bvm documentation](https://burnlang.github.io/docs/bvm/overview),
[archives](https://burnlang.github.io/docs/bvm/archives), [mixins](https://burnlang.github.io/docs/bvm/mixins) and [native interop](https://burnlang.github.io/docs/bvm/native-interop).

## Documentation

The documentation is at **[burnlang.github.io/docs](https://burnlang.github.io/docs/)**: the whole language, the
standard library, the command line and tools, [how Burn works in the background](https://burnlang.github.io/docs/internals/overview)
and the Burn Virtual Machine. Its sources and the website live in [burnlang/docs](https://github.com/burnlang/docs).

<p align="center"><img src="https://raw.githubusercontent.com/burnlang/docs/master/content/images/pipeline.svg" alt="The pipeline from source to bvm, native code and JavaScript" width="820"></p>

## Examples

See [`examples/`](examples/) and the test programs in [`tests/cases/`](tests/cases/). bvm assembly and Ember
examples are in [`examples/bvm/`](examples/bvm/).

## Project structure

- `compiler/`: the compiler and tools, written in Burn (see [compiler/README.md](compiler/README.md))
  - `src/syntax/`, `src/diag/`, `src/project/`: lexer, parser, error messages, `burn.toml` and the module loader
  - `src/check/`: type checker with smart casts, lowers to the typed IR in `hir.bn`
  - `src/lower/`, `src/vm/`: ownership, then bvm code generation
  - `src/native/`, `src/js/`: the x86-64, arm64 and JavaScript backends
  - `src/cli/`, `src/lsp/`, `src/doc/`: the command line, the language server and `burn doc`
  - `src/main.bn` is the compiler on its own, and `src/bin/burn.bn` the command line that toolchains ship
  - `src/bvm/`: bvm, the Burn Virtual Machine: bytecode format, assembler, verifier, linker, mixins, archives, the
    interpreter, the JIT and the `bvm` command
- `lib/rt/`: the runtime written in Burn (memory, strings, collections, printing, files, processes, JSON, HTTP and
  tasks), which native executables and bvm link
- `lib/tools/`: tools written in Burn that programs can also import, such as the formatter (`import "tools/fmt"`), which `burnfmt` is built from
- `scripts/package.sh`: builds the toolchain into the layout burnup installs and releases ship
- `scripts/bootstrap.sh`: builds the compiler written in Burn with itself, starting from the seed, and checks that the result is stable
- `compiler/boot/`: the seeds, the compiler and its runtime as assembly for each platform, which a C compiler assembles to build `compiler/src` the first time
- `install.sh`: forwards to [burnup](https://github.com/burnlang/burnup), the installer
- `assets/`: the logo
- `lib/std/`: the standard library, written in Burn
- `tests/`: the test programs and the end-to-end tests (`tests/run.bn`, written in Burn), which `scripts/test.sh` runs against a toolchain

Editor support lives in its own repositories: [vscode-burn](https://github.com/burnlang/vscode-burn),
[intellij-burn](https://github.com/burnlang/intellij-burn), [zed-burn](https://github.com/burnlang/zed-burn) and
[tree-sitter-burn](https://github.com/burnlang/tree-sitter-burn). See [editor support](https://burnlang.github.io/docs/tooling/editors).
The documentation is in [burnlang/docs](https://github.com/burnlang/docs).

## Development

```sh
sh scripts/package.sh --prefix ~/.burn   # build and install the toolchain from this checkout
sh scripts/test.sh                       # build a toolchain from this checkout and run every test
sh scripts/test.sh --toolchain ~/.burn native_matches_vm   # run one test against an installed toolchain
```

Building the toolchain only needs a C compiler, tar and xz. The C compiler assembles the seed in `compiler/boot`, the
compiler written in Burn as assembly for this platform, which builds `compiler/src`; from then on the compiler builds
itself. `sh scripts/bootstrap.sh --update` replaces the seeds with the compiler built from `compiler/src`, so the
compiler's own sources can use new language features.

## Contributing

Contributions are welcome:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes
4. Push the branch and open a pull request

Please make sure `sh scripts/test.sh` passes and new language features come with a test in `tests/cases/`.

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Plans for Burn

1. A faster JIT: values kept in registers and more instructions compiled inline
2. The JIT on arm64 (Apple silicon and Linux)
3. Native backends for Windows

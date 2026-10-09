<p align="center">
    <img src="assets/logo.svg" alt="Burn logo" width="160">
</p>

<h1 align="center">Burn</h1>

Burn is an easy-to-use, statically typed, general-purpose programming language with smart casts.
Burn is written in **Rust** and **x86-64 assembly**: programs run instantly on **bvm**, the Burn
virtual machine, during development and compile to small **native executables** for shipping.

Current version: **26.1.0-experimental-2**

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
- Tiny executables without the standard library: `std = false` in burn.toml or `--no-std` (a hello world is 39 KB)
- Modules with `pub` and `priv`
- Standard library for dates, times, HTTP, JSON, math, strings, processes and files
- Projects and packages: `burn init github.com/you/app`, `burn.toml`, `burn.lock` and ash, the package manager
- Three backends sharing one type checker and runtime: bvm bytecode, native x86-64, JavaScript
- bvm, a general-purpose virtual machine with its own assembly language, bytecode format and verifier, which other languages can target too
- Write once, run everywhere: `.bar` archives bundle an application's bytecode and resources and run wherever bvm runs
- Mixins: `@Inject`, `@Overwrite` and `@Redirect` rewrite existing bytecode functions when modules are linked
- Native interop: native executables embed bytecode libraries and call them, and the libraries call back into native code
- Precise error messages with line, column and suggestions
- Built-in REPL, formatter and language server, plus a VS Code extension

## Installation

```sh
curl -fsSL https://raw.githubusercontent.com/burnlang/burnup/master/install.sh | sh
```

[burnup](https://github.com/burnlang/burnup), the Burn version manager, puts the toolchain and ash into `~/.burn/bin`
and adds it to your `PATH`. It installs and switches versions (`burnup install 26.1`, `burnup default master`), and
a project can pin its version with `burn = "26.1"` in `burn.toml`. It builds from source when no
prebuilt release is available, which needs Rust 1.85 or newer (`--install-rust` sets that up for you). Native
executables need a C toolchain (`cc`) and currently target x86-64 Linux and macOS.
See [docs/tooling/installation.mdx](docs/tooling/installation.mdx) for all options, updating and uninstalling.

## The toolchain

| Command | What it does |
| --- | --- |
| `burni` | the interpreter: runs programs instantly on bvm, starts the REPL without arguments |
| `burnc` | the compiler: standalone native executables, JavaScript with `--target js`, bvm bytecode with `--target bvm` |
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
burnc app.bn --target js        # Node.js script app.js
burnc app.bn --target bvm       # portable bytecode app.bvmc
bvm app.bvmc                    # run bytecode
burnc --check app.bn            # type-check only
burnfmt -w app.bn               # format in place
burn run --native app.bn        # compile to machine code and run
burn init github.com/you/app    # start a project with burn.toml
ash install github.com/you/lib  # add a package
```

See [the toolchain](docs/tooling/toolchain.mdx) and the [command line reference](docs/tooling/cli.mdx).

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
a verifier, a reference-counted runtime with about 100 built-in functions, host functions, threads and a Rust API
for generating code. That's everything a new language needs for a backend.

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

[`bvm/examples/ember.rs`](bvm/examples/ember.rs) is a complete small language built on bvm.

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
burnc app.bn --target bar -o app.bar        # one archive: ./app.bar runs anywhere bvm runs
burnc app.bn -o app                         # a native executable with the library embedded
```

The library calls `hostName()` back in the program, and the program's mixin rewrites the library's `describe`,
both on bvm and in the native executable. See [the bvm documentation](docs/bvm/overview.mdx),
[archives](docs/bvm/archives.mdx), [mixins](docs/bvm/mixins.mdx) and [native interop](docs/bvm/native-interop.mdx).

## Documentation

The full documentation is written in MDX in [`docs/`](docs/index.mdx): a syntax tour, types, smart casts,
structs and interfaces, modules, async, the standard library, the command line, how the compiler works and
the Burn Virtual Machine.

## Examples

See [`examples/`](examples/) and the test programs in [`tests/cases/`](tests/cases/). bvm assembly and Ember
examples are in [`examples/bvm/`](examples/bvm/).

## Project structure

- `crates/burn/`: the compiler and tools
  - `lexer.rs`, `parser.rs`, `ast.rs`: front end
  - `check/`: type checker with smart casts, lowers to the typed IR in `hir.rs`
  - `vm/`: lowers the typed IR to bvm modules
  - `native/`: x86-64 code generator, hand-written entry assembly, linker driver
  - `js/`: JavaScript backend
  - `lsp/`, `fmt.rs`, `repl.rs`: tooling
- `bvm/`: the Burn Virtual Machine: instruction set, assembler, bytecode format, verifier, linker, mixins,
  archives, interpreter, the native bridge, the `bvm` command and the Ember example language
- `crates/burn-runtime/`: runtime shared by bvm and native executables (memory, strings, collections, JSON, HTTP, tasks)
- `compiler/`: the compiler written in Burn, which builds itself on bvm (see [compiler/README.md](compiler/README.md))
- `lib/tools/`: tools written in Burn that programs can also import, such as the formatter (`import "tools/fmt"`), which `burnfmt` is built from
- `scripts/package.sh`: builds the toolchain into the layout burnup installs and releases ship
- `scripts/bootstrap.sh`: builds the compiler written in Burn with itself and checks that the result is stable
- `install.sh`: forwards to [burnup](https://github.com/burnlang/burnup), the installer
- `assets/`: the logo
- `lib/std/`: the standard library, written in Burn
- `docs/`: documentation
- `tests/`: end-to-end tests run against every backend

Editor support lives in its own repositories: [vscode-burn](https://github.com/burnlang/vscode-burn),
[intellij-burn](https://github.com/burnlang/intellij-burn), [zed-burn](https://github.com/burnlang/zed-burn) and
[tree-sitter-burn](https://github.com/burnlang/tree-sitter-burn). See [editor support](docs/tooling/editors.mdx).

## Development

```sh
sh scripts/package.sh --prefix ~/.burn   # build and install the toolchain from this checkout
cargo test            # runs every example on bvm, natively and as JavaScript
cargo clippy --all-targets
```

## Contributing

Contributions are welcome:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes
4. Push the branch and open a pull request

Please make sure `cargo test` passes and new language features come with a test in `tests/cases/`.

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Plans for Burn

1. Native backends for Windows and ARM64
2. Shipping the self-hosted compiler (it already builds itself, see [compiler/README.md](compiler/README.md))
3. A documentation website built from `docs/`

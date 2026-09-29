# Burnlang

<p align="center">
    <img src="https://github.com/s42yt/assets/blob/master/assets/burnlang/burn-logo.png" alt="Burn Logo">
</p>

Burn is an easy-to-use, statically typed, general-purpose programming language with smart casts.
Burn is written in **Rust** and **x86-64 assembly**: programs run instantly on a bytecode VM
during development and compile to small **native executables** for shipping.

> [!WARNING]
> Burn is **not** ready for production. Syntax may still change. Please report bugs as issues.

```burn
def interface Shape {
    fun area(): float
}

def class Circle: Shape {
    float radius

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
    print(describe(Circle { radius: 2 }), describe(name), p)
}
```

## Features

- Static types with inference, so you rarely write them
- Smart casts: `is` checks, `!= null` checks, early returns and assignments narrow types automatically
- Null safety with `T?`
- Type-first declarations: `String name = "Burn"`, `[int] ids = []`
- One definition syntax for everything: `def type`, `def class`, `def interface`, `def enum`
- Classes with fields, methods, constructors, static functions and private members; interfaces with checked conformance
- Arrays, maps, records, enums, first-class functions and lambdas, string templates
- `async fun` / `await` running on real threads
- Modules with `pub` and `priv`
- Standard library for dates, times, HTTP, JSON, math and strings
- Three backends sharing one type checker and runtime: bytecode VM, native x86-64, JavaScript
- Precise error messages with line, column and suggestions
- Built-in REPL, formatter and language server, plus a VS Code extension

## Installation

Requirements: Rust 1.85 or newer. Building native executables needs a C toolchain (`cc`) for assembling and
linking; native output currently targets x86-64 Linux and macOS.

```sh
git clone https://github.com/burnlang/burn.git
cd burn
cargo build --release
./target/release/burn version
```

## Usage

```sh
burn app.bn                     # run instantly (bytecode VM)
burn run --native app.bn        # compile to machine code and run
burn build app.bn               # standalone executable ./app
burn build app.bn -o bin/app --emit-asm app.s
burn build app.bn --target js   # Node.js script app.js
burn check app.bn               # type-check only
burn fmt -w app.bn              # format in place
burn repl                       # interactive session
burn eval 'print(6 * 7)'        # run a snippet
burn lsp                        # language server for editors
```

The older flags `-r`, `-e`, `-exe` and `-d` still work.

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

def class Human: Greeter {
    string name

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

### Async

```burn
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

## Documentation

The full documentation is written in MDX in [`docs/`](docs/index.mdx): a syntax tour, types, smart casts,
classes and interfaces, modules, async, the standard library, the command line and how the compiler works.

## Examples

See [`examples/`](examples/) and the test programs in [`tests/cases/`](tests/cases/).

## Project structure

- `crates/burn/`: the compiler, VM and tools
  - `lexer.rs`, `parser.rs`, `ast.rs`: front end
  - `check/`: type checker with smart casts, lowers to the typed IR in `hir.rs`
  - `vm/`: bytecode compiler and virtual machine
  - `native/`: x86-64 code generator, hand-written entry assembly, linker driver
  - `js/`: JavaScript backend
  - `lsp/`, `fmt.rs`, `repl.rs`: tooling
- `crates/burn-runtime/`: runtime shared by the VM and native executables (GC, strings, collections, JSON, HTTP, tasks)
- `lib/std/`: the standard library, written in Burn
- `editors/vscode/`: VS Code extension
- `docs/`: documentation
- `tests/`: end-to-end tests run against every backend

## Development

```sh
cargo test            # runs every example on the VM, natively and as JavaScript
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

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Plans for Burn

1. Native backends for Windows and ARM64
2. Self-hosting the compiler
3. A documentation website built from `docs/`
4. A package manager

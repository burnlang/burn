# Burn keywords and built-in types

This is what your editor shows when you hover over a keyword or a built-in type of Burn, and what opens when you
go to its definition. The whole language is documented at https://s42yt.github.io/burn-docs/.

## `def`

*keyword*: starts a definition of a new type: `def type`, `def struct`, `def interface`, `def enum` or
`def annotation`. Everything Burn knows about a type comes from one `def`.

```burn
def type Point {
    int x
    int y
}

def enum Color { Red, Green, Blue }

print(Point { x: 1, y: 2 }, Color.Red)
```

More: https://s42yt.github.io/burn-docs/language/types

## `type`

*keyword*: `def type Name { ... }` defines a record type: named fields, created with a literal and compared by value.
`def type Name = Other` gives an existing type another name.

```burn
def type Person {
    string name
    int age = 0
}

def type UserId = int

var ada = Person { name: "Ada", age: 36 }
UserId id = 7
print(ada.name, ada.age, id)
```

More: https://s42yt.github.io/burn-docs/language/types

## `struct`

*keyword*: `def struct Name(params) { ... }` defines a struct: an object with a constructor, fields, methods and
static members, created with `new`. Structs are passed by reference. `: Parent(args)` extends another struct and
`:: Interface` implements an interface or an abstract struct.

```burn
def struct Counter(start: int) {
    int count = start

    fun add(n: int) {
        count += n
    }
}

var c = new Counter(10)
c.add(5)
print(c.count)
```

More: https://s42yt.github.io/burn-docs/language/structs-and-interfaces

## `interface`

*keyword*: `def interface Name { ... }` lists the methods a struct must have. A struct declares that it implements it
with `::`, the compiler checks that every method is there, and calls through the interface are dispatched to the
struct's method.

```burn
def interface Shape {
    fun area(): float
}

def struct Square(side: float) :: Shape {
    fun area(): float {
        return side * side
    }
}

Shape s = new Square(3.0)
print(s.area())
```

More: https://s42yt.github.io/burn-docs/language/structs-and-interfaces#interfaces

## `enum`

*keyword*: `def enum Name { ... }` defines a type with a fixed set of variants. Variants can carry data, and `match`
takes them apart.

```burn
def enum Shape {
    Circle(radius: float),
    Rect(w: float, h: float)
}

fun area(s: Shape): float {
    return match s {
        Shape.Circle(r) => 3.14159 * r * r
        Shape.Rect(w, h) => w * h
    }
}

print(area(Shape.Rect(2.0, 3.0)))
```

More: https://s42yt.github.io/burn-docs/language/enums

## `annotation`

*keyword*: `def annotation Name { ... }` defines an annotation, which is written as `@Name(...)` in front of a
definition and read at runtime with `annotationsOf`.

```burn
def annotation Route {
    string path
    string method = "GET"
}

@Route("/users", method: "POST")
def struct CreateUser(name: string) {
}

for a in annotationsOf(new CreateUser("ada")) {
    if (a is Route) {
        print(a.method, a.path)
    }
}
```

More: https://s42yt.github.io/burn-docs/language/annotations

## `abstract`

*keyword*: `def abstract struct` defines a struct that cannot be created with `new`, only extended. Its
`abstract fun` methods have no body; every struct that extends it with `::` must write them.

```burn
def abstract struct Animal(name: string) {
    abstract fun sound(): string

    fun describe(): string {
        return "${name} says ${sound()}"
    }
}

def struct Dog(name: string) :: Animal(name) {
    fun sound(): string {
        return "woof"
    }
}

print(new Dog("Rex").describe())
```

More: https://s42yt.github.io/burn-docs/language/structs-and-interfaces#abstract-structs

## `static`

*keyword*: a `static` field or method belongs to the struct itself, not to its objects, and is used as
`Name.member`. `def static struct` defines a struct that only has static members, like a namespace.

```burn
def struct Temperature(celsius: float) {
    static var created = 0

    static fun freezing(): Temperature {
        return new Temperature(0.0)
    }
}

var t = Temperature.freezing()
print(t.celsius, Temperature.created)
```

More: https://s42yt.github.io/burn-docs/language/structs-and-interfaces#static-members

## `new`

*keyword*: creates an object of a struct by calling its constructor: `new Name(args)`.

```burn
def struct Point(x: int, y: int) {
}

var p = new Point(3, 4)
print(p.x + p.y)
```

More: https://s42yt.github.io/burn-docs/language/structs-and-interfaces#creating-objects

## `destroy`

*keyword*: `destroy name(args)` destroys the object in a variable now instead of when it is no longer used. If the
struct defines `fun destroy(...)`, that function runs first with the arguments. Using the variable afterwards is a
compile error until a new object is assigned to it.

```burn
def struct File(path: string) {
    fun destroy(reason: string) {
        print("closing", path, "because", reason)
    }
}

var f = new File("data.txt")
destroy f("done")
```

More: https://s42yt.github.io/burn-docs/language/structs-and-interfaces#destroying-objects

## `fun`

*keyword*: declares a function, a method inside a struct, or an anonymous function used as a value. Parameters are
`name: Type`; the result type comes after `:` and can be left out when the body makes it clear.

```burn
fun add(a: int, b: int): int {
    return a + b
}

var twice = fun(x: int): int {
    return x * 2
}

print(add(2, 3), twice(21))
```

More: https://s42yt.github.io/burn-docs/language/functions

## `var`

*keyword*: declares a variable. Its type comes from the value (`var n = 3` is an `int`) or is written after `:`.
A variable can also be declared type first, without `var`: `int n = 3`.

```burn
var count = 3
var ratio: float = 2.5
count += 1
print(count, ratio)
```

More: https://s42yt.github.io/burn-docs/language/variables

## `const`

*keyword*: declares a constant: a name for a value that never changes.

```burn
const LIMIT = 10
const NAMES = ["ada", "alan"]
print(LIMIT, NAMES[0])
```

More: https://s42yt.github.io/burn-docs/language/variables#constants

## `if`

*keyword*: runs a block when a condition is true; `else` and `else if` add the other cases. The condition must be a
`bool`, and checks such as `x is T` or `x != null` narrow the type of `x` inside the block (smart casts).

```burn
var n = 7
if (n > 10) {
    print("big")
} else if (n > 5) {
    print("medium")
} else {
    print("small")
}
```

More: https://s42yt.github.io/burn-docs/language/control-flow

## `else`

*keyword*: the part of an `if` that runs when the condition is false, and the arm of a `match` that fits every
value that no other arm fits.

```burn
var n = 3
if (n % 2 == 0) {
    print("even")
} else {
    print("odd")
}
print(match n { 1 => "one", else => "more" })
```

More: https://s42yt.github.io/burn-docs/language/control-flow

## `while`

*keyword*: repeats a block as long as a condition is true.

```burn
var n = 3
while (n > 0) {
    print(n)
    n -= 1
}
```

More: https://s42yt.github.io/burn-docs/language/control-flow

## `for`

*keyword*: loops over a range (`0..3`, or `0..=3` with the end included), an array, a map or a string, with
`for x in ...` or `for i, x in ...` for the index too. `for (init; condition; step)` is the loop of C.

```burn
for i in 0..3 {
    print(i)
}
for i, name in ["ada", "alan"] {
    print(i, name)
}
```

More: https://s42yt.github.io/burn-docs/language/control-flow

## `in`

*keyword*: separates the variables of a `for` loop from what it loops over.

```burn
for c in "abc" {
    print(c)
}
```

More: https://s42yt.github.io/burn-docs/language/control-flow

## `return`

*keyword*: ends the function and gives its result, or just ends it in a function without one.

```burn
fun sign(n: int): int {
    if (n < 0) {
        return -1
    }
    return 1
}

print(sign(-5))
```

More: https://s42yt.github.io/burn-docs/language/control-flow

## `break`

*keyword*: leaves the innermost loop right away.

```burn
for i in 0..10 {
    if (i == 3) {
        break
    }
    print(i)
}
```

More: https://s42yt.github.io/burn-docs/language/control-flow

## `continue`

*keyword*: skips the rest of the loop's body and goes on with the next round.

```burn
for i in 0..5 {
    if (i % 2 == 0) {
        continue
    }
    print(i)
}
```

More: https://s42yt.github.io/burn-docs/language/control-flow

## `match`

*keyword*: compares a value with patterns, top to bottom, and gives the result of the first arm that fits. Patterns
are values, ranges, enum variants (with their data), types (`is string s`) and `else`; `if` after a pattern adds a
guard. A `match` on an enum must cover every variant.

```burn
fun size(n: int): string {
    return match n {
        0 => "zero"
        1..=9 => "small"
        else => "large"
    }
}

print(size(5))
```

More: https://s42yt.github.io/burn-docs/language/match

## `true`

*keyword*: the `bool` value true.

```burn
var done = true
print(done && !false)
```

More: https://s42yt.github.io/burn-docs/language/types

## `false`

*keyword*: the `bool` value false.

```burn
var found = false
print(found || true)
```

More: https://s42yt.github.io/burn-docs/language/types

## `null`

*keyword*: no value. Only nullable types, written `T?`, can hold it; `?.`, `??` and `!= null` checks work with them.

```burn
string? nickname = null
print(nickname ?? "none")
nickname = "Ada"
if (nickname != null) {
    print(len(nickname))
}
```

More: https://s42yt.github.io/burn-docs/language/errors

## `import`

*keyword*: loads a module: the standard library (`"std/strings"`), a file of the project (`"utils.bn"`, `"@/net/http"`),
a package or a bytecode library (`"lib.bvmc"`). `as Name` imports it under an alias; imports inside a function or
block are only visible there.

```burn
import "std/math" as M

print(M.sqrt(16.0))
```

More: https://s42yt.github.io/burn-docs/language/modules

## `as`

*keyword*: `import "module" as Name` names a module; `value as T` converts a value to a type, and `value as? T` gives
`null` instead of an error when it is not one.

```burn
fun show(v: any): string {
    var s = v as? string
    return s ?? "not a string"
}

any n = 41
print((n as int) + 1, show("burn"), show(3))
```

More: https://s42yt.github.io/burn-docs/language/smart-casts

## `is`

*keyword*: `value is T` checks whether a value is of type `T`, and inside the `if` it narrows the value to `T`
(smart cast).

```burn
fun describe(v: any): string {
    if (v is string) {
        return "a string of length ${len(v)}"
    }
    return "something else"
}

print(describe("burn"))
```

More: https://s42yt.github.io/burn-docs/language/smart-casts

## `pub`

*keyword*: makes a name of a module visible to the files that import it. Names are public by default; `pub` says so
explicitly.

```burn
pub fun greet(name: string): string {
    return "hi " + name
}

print(greet("ada"))
```

More: https://s42yt.github.io/burn-docs/language/modules

## `priv`

*keyword*: keeps a name private to its module: files that import the module cannot use it. Inside a struct,
`priv` members can only be used by the struct itself.

```burn
priv fun helper(): int {
    return 41
}

print(helper() + 1)
```

More: https://s42yt.github.io/burn-docs/language/modules

## `async`

*keyword*: `async fun` runs on its own thread when it is called and gives a `Future<T>` at once; `await` waits for
its result.

```burn
async fun double(n: int): int {
    return n * 2
}

var a = double(20)
print(await a + 2)
```

More: https://s42yt.github.io/burn-docs/language/async

## `await`

*keyword*: waits until the `Future<T>` of an `async fun` has its result, and gives it.

```burn
async fun answer(): int {
    return 42
}

print(await answer())
```

More: https://s42yt.github.io/burn-docs/language/async

## `unsafe`

*keyword*: `unsafe { ... }` marks code the compiler cannot check: raw memory (`std/memory`), `asm` blocks and calls to
C (`@Extern`). `unsafe fun` makes a function that can only be called in unsafe code.

```burn
import "std/memory"

unsafe {
    var p = Memory.alloc(8)
    Memory.store64(p, 42)
    print(Memory.load64(p))
    Memory.free(p)
}
```

More: https://s42yt.github.io/burn-docs/language/unsafe

## `asm`

*keyword*: an `asm { ... }` block in unsafe code holds assembly for each target in its own section (`x86_64`,
`arm64`, `bvm`, `js`), with an optional `else` section in Burn for every other target. `{name}` stands for a variable.

```burn
var x = 20
var r = 0
unsafe {
    asm {
        bvm {
            load {x}
            const 2
            iadd
            store {r}
        }
        else {
            r = x + 2
        }
    }
}
print(r)
```

More: https://s42yt.github.io/burn-docs/language/inline-assembly

## `int`

*type*: a 64-bit signed integer, from -9223372036854775808 to 9223372036854775807. Overflow is a runtime error.
Literals: `42`, `0x1F`, `0b1010`, `1_000_000`. Also written `Int`, `int64` or `long`.

```burn
int n = 0x10
print(n * 2, toString(255, 16))
```

More: https://s42yt.github.io/burn-docs/language/types

## `float`

*type*: a 64-bit floating point number (IEEE 754 double). Literals: `3.14`, `2.5e-3`. Also written `Float`,
`float64` or `double`.

```burn
float r = 2.0
print(3.14159 * r * r)
```

More: https://s42yt.github.io/burn-docs/language/types

## `bool`

*type*: `true` or `false`, the type of conditions. Also written `Bool` or `boolean`.

```burn
bool ready = 3 > 2
print(ready, !ready)
```

More: https://s42yt.github.io/burn-docs/language/types

## `string`

*type*: an immutable sequence of Unicode characters. `+` joins strings, `${...}` inserts a value into one, and
`len`, indexing and `for` work on characters. Also written `String` or `str`.

```burn
string name = "Burn"
print("hello ${name}", len(name), name[0])
```

More: https://s42yt.github.io/burn-docs/language/strings

## `any`

*type*: a value of any type. To use it as a specific type, check it with `is` (which narrows it) or convert it with
`as`. Also written `Any`.

```burn
any v = "text"
if (v is string) {
    print(len(v))
}
```

More: https://s42yt.github.io/burn-docs/language/smart-casts

## `void`

*type*: the result type of a function that returns nothing. Also written `Void`.

```burn
fun log(msg: string): void {
    print("[log]", msg)
}

log("started")
```

More: https://s42yt.github.io/burn-docs/language/functions

## `int8`

*type*: an 8-bit signed integer, from -128 to 127. Arrays of it (`[int8]`) use one byte per element.

```burn
int8 t = -100
print(t)
```

More: https://s42yt.github.io/burn-docs/language/types#sized-numbers

## `int16`

*type*: a 16-bit signed integer, from -32768 to 32767.

```burn
int16 sample = -1200
print(sample)
```

More: https://s42yt.github.io/burn-docs/language/types#sized-numbers

## `int32`

*type*: a 32-bit signed integer, from -2147483648 to 2147483647.

```burn
int32 n = 2000000000
print(n)
```

More: https://s42yt.github.io/burn-docs/language/types#sized-numbers

## `uint8`

*type*: an 8-bit unsigned integer, from 0 to 255, also called `byte`. Arrays of it (`[uint8]`) use one byte per
element.

```burn
[uint8] pixels = [255, 128, 0]
print(pixels)
```

More: https://s42yt.github.io/burn-docs/language/types#sized-numbers

## `uint16`

*type*: a 16-bit unsigned integer, from 0 to 65535.

```burn
uint16 port = 8080
print(port)
```

More: https://s42yt.github.io/burn-docs/language/types#sized-numbers

## `uint32`

*type*: a 32-bit unsigned integer, from 0 to 4294967295.

```burn
uint32 color = 4294967295
print(color)
```

More: https://s42yt.github.io/burn-docs/language/types#sized-numbers

## `uint64`

*type*: a 64-bit unsigned integer, from 0 to 18446744073709551615.

```burn
uint64 hash = 14695981039346656037
print(hash)
```

More: https://s42yt.github.io/burn-docs/language/types#sized-numbers

## `float32`

*type*: a 32-bit floating point number, with about 7 significant digits. Arrays of it use 4 bytes per element.

```burn
float32 ratio = 0.5
print(ratio)
```

More: https://s42yt.github.io/burn-docs/language/types#sized-numbers

## `Future`

*type*: `Future<T>` is the result of calling an `async fun`: the value `T` the function will give, which `await`
waits for.

```burn
async fun slow(): string {
    return "done"
}

Future<string> f = slow()
print(await f)
```

More: https://s42yt.github.io/burn-docs/language/async

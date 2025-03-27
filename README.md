# Burnlang

<p align="center">
    <img src="https://github.com/s42yt/assets/blob/master/assets/burnlang/burn-logo.png" alt="Burn Logo">
</p>

Burn is an easy-to-use general-purpose programming language designed with simplicity and expressiveness in mind.

> [!WARNING]
> Burn is **not** ready for Production! Syntax may still change and functions may not work. Please Report bugs as Issues


## Features

- Clean, readable syntax
- Strong and static typing
- First-class functions
- Struct-based type system
- Import system for code organization
- Built-in REPL for interactive development

## Installation

### Prerequisites

- Go 1.24 or higher

### Building from Source

1. Clone the repository:
```sh
git clone https://github.com/burnlang/burn.git
cd burn
```

2. Build the project:
```sh
go build
```

3. Run the executable:
```sh
# On Unix/Linux/macOS
./burn.exe

# On Windows
.\burn.exe
```

## Usage

Burn can be used in several ways:

### Execute a Burn file

```sh
burn path/to/file.bn
```

### Start the REPL (interactive mode)

```sh
burn -r
```

### Evaluate code directly

```sh
burn -e 'print("Hello, World!")'
```

### Debug mode

Add the `-d` flag to see tokens, AST, and execution details:

```sh
burn -d path/to/file.bn
```

## Language Syntax

### Variables

```bn
var name = "John"
var age = 30
const PI = 3.14159
```

### Functions

```bn
fun add(a: int, b: int): int {
    return a + b
}
```

### Types

```bn
def Person {
    name: string,
    age: int,
    active: bool
}

var person = {
    name: "John",
    age: 30,
    active: true
}

print(person.name)
```

### Classes

```bn
// Classes provide a way to organize related functions
class Human {
    fun create(name: string, age: int): Human {
        return {
            name: name,
            age: age
        }
    }
    
    fun greet(human: Human): string {
        return "Hello, " + human.name + "!"
    }
}

fun main() {
    var john = Human.create("John", 30)
    print(Human.greet(john))
}
``` 

### Control Flow

```bn
// Define variables before using them
var x = 3
var counter = 0

// If statements
if (x > 5) {
    print("x is greater than 5")
} else if (x == 5) {
    print("x equals 5")
} else {
    print("x is less than 5")
}

// While loops
while (counter < 3) {
    print("Counter: " + toString(counter))
    counter = counter + 1
}

// For loops
for (var i = 0; i < 3; i = i + 1) {
    print("Loop iteration: " + toString(i))
}
```

### Imports

```bn
import "test/utils.bn"

fun main() {
    var result = power(2, 3)  // Using imported function
    print("2^3 = " + toString(result))
}
```

### Built-in Functions

- `print(value)`: Display values to console
- `toString(value)`: Convert a value to string
- `input(prompt)`: Read user input with a prompt

## Examples

Check the [test](test/) directory for example programs:

- [Main example](test/main.bn)
- [Type definitions](test/type.bn)
- [Input handling](test/input.bn)
- [Utility functions](test/utils.bn)

## Project Structure

- `cmd/`: Command-line interface
- `pkg/`: Core packages
  - `ast/`: Abstract syntax tree definitions
  - `lexer/`: Tokenization of source code
  - `parser/`: Parsing tokens into AST
  - `typechecker/`: Type checking system
  - `interpreter/`: Runtime execution

## Contributing

Contributions are welcome! Here's how you can contribute:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Please make sure your code follows the existing style and includes appropriate tests.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Thanks to all contributors who have helped shape the Burn language
- Inspired by modern programming languages with clean syntax


## Plans For Burn

1. Until language is ready for Production only master branch will be used
2. Post Production Language will be self hosted
3. Documentaion of the entire language with its own website
4. After Selfhosting Package Manager will be next and use of other packages will be possible
5. ...
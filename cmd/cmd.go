package cmd

import (
    "fmt"
    "io"
    "os"
    "strings"

    "github.com/s42yt/burn/pkg/ast"
    "github.com/s42yt/burn/pkg/interpreter"
    "github.com/s42yt/burn/pkg/lexer"
    "github.com/s42yt/burn/pkg/parser"
    "github.com/s42yt/burn/pkg/typechecker"
)

func Execute(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
    if len(args) < 1 {
        printUsage(stdout)
        return 1
    }

    nonOptions, options := parseArgs(args)

    if options["help"] {
        printUsage(stdout)
        return 0
    }

    if options["version"] {
        fmt.Fprintf(stdout, "Burn Programming Language v%s\n", getVersion())
        return 0
    }

    if options["repl"] {
        return startREPL(stdin, stdout, stderr)
    }

    if options["eval"] {
        if len(nonOptions) == 0 {
            fmt.Fprintln(stderr, "Error: no code provided for evaluation")
            return 1
        }
        return executeCode(nonOptions[0], options["debug"], stdout, stderr)
    }

    if len(nonOptions) == 0 {
        printUsage(stdout)
        return 1
    }

    filename := nonOptions[0]
    debug := options["debug"]

    return executeFile(filename, debug, stdout, stderr)
}

func getVersion() string {
    return "0.1.0"
}

func parseArgs(args []string) ([]string, map[string]bool) {
    nonOptions := []string{}
    options := map[string]bool{
        "help":    false,
        "version": false,
        "repl":    false,
        "eval":    false,
        "debug":   false,
    }

    for i := 0; i < len(args); i++ {
        arg := args[i]
        if strings.HasPrefix(arg, "-") {
            switch arg {
            case "-h", "--help":
                options["help"] = true
            case "-v", "--version":
                options["version"] = true
            case "-r", "--repl":
                options["repl"] = true
            case "-e", "--eval":
                options["eval"] = true
                if i+1 < len(args) {
                    nonOptions = append(nonOptions, args[i+1])
                    i++
                }
            case "-d", "--debug":
                options["debug"] = true
            }
        } else {
            nonOptions = append(nonOptions, arg)
        }
    }

    return nonOptions, options
}

func printUsage(w io.Writer) {
    fmt.Fprintln(w, "Burn Programming Language")
    fmt.Fprintln(w, "Usage:")
    fmt.Fprintln(w, "  burn [options] [filename]")
    fmt.Fprintln(w, "")
    fmt.Fprintln(w, "Options:")
    fmt.Fprintln(w, "  -h, --help     Show this help message")
    fmt.Fprintln(w, "  -v, --version  Show version information")
    fmt.Fprintln(w, "  -r, --repl     Start interactive REPL (Read-Eval-Print Loop)")
    fmt.Fprintln(w, "  -e, --eval     Evaluate Burn code from command line")
    fmt.Fprintln(w, "  -d, --debug    Run in debug mode (show more information)")
    fmt.Fprintln(w, "")
    fmt.Fprintln(w, "Examples:")
    fmt.Fprintln(w, "  burn main.bn              Execute a Burn program")
    fmt.Fprintln(w, "  burn -r                   Start REPL")
    fmt.Fprintln(w, "  burn -e 'print(\"Hello\")' Evaluate a single expression")
}

func executeFile(filename string, debug bool, stdout, stderr io.Writer) int {
    if !strings.HasSuffix(filename, ".bn") {
        fmt.Fprintf(stderr, "Warning: File %s does not have the .bn extension\n", filename)
    }

    source, err := os.ReadFile(filename)
    if err != nil {
        fmt.Fprintf(stderr, "Error reading file: %v\n", err)
        return 1
    }

    return executeCode(string(source), debug, stdout, stderr)
}

func executeCode(source string, debug bool, stdout, stderr io.Writer) int {
    result, err := execute(source, debug, stdout)
    if err != nil {
        fmt.Fprintf(stderr, "Error: %v\n", err)
        return 1
    }

    if result != nil && debug {
        fmt.Fprintln(stdout, "Program result:", result)
    }

    return 0
}

func execute(source string, debug bool, stdout io.Writer) (interface{}, error) {
    lex := lexer.New(source)
    tokens, err := lex.Tokenize()
    if err != nil {
        return nil, formattedError("lexical error", err, source, lex.Position())
    }

    if debug {
        fmt.Fprintln(stdout, "--- Tokens ---")
        for _, token := range tokens {
            if token.Type != lexer.TokenEOF {
                fmt.Fprintf(stdout, "%s '%s' at line %d, col %d\n", 
                    tokenTypeToString(token.Type), token.Value, token.Line, token.Col)
            }
        }
        fmt.Fprintln(stdout)
    }

    p := parser.New(tokens)
    program, err := p.Parse()
    if err != nil {
        return nil, formattedError("syntax error", err, source, p.Position())
    }

    if debug {
        fmt.Fprintln(stdout, "--- AST ---")
        printAST(program, 0, stdout)
        fmt.Fprintln(stdout)
    }

    tc := typechecker.New()
    if err := tc.Check(program); err != nil {
        return nil, formattedError("type error", err, source, tc.Position())
    }

    if debug {
        fmt.Fprintln(stdout, "--- Type Check Passed ---")
        fmt.Fprintln(stdout)
    }

    interp := interpreter.New()
    result, err := interp.Interpret(program)
    if err != nil {
        return nil, formattedError("runtime error", err, source, interp.Position())
    }

    return result, nil
}

func formattedError(errType string, err error, source string, pos int) error {
    line, col := getLineAndCol(source, pos)
    return fmt.Errorf("%s at line %d, column %d: %v", errType, line, col, err)
}

func getLineAndCol(source string, pos int) (int, int) {
    line := 1
    col := 1

    for i := 0; i < pos && i < len(source); i++ {
        if source[i] == '\n' {
            line++
            col = 1
        } else {
            col++
        }
    }

    return line, col
}

func tokenTypeToString(tokenType lexer.TokenType) string {
    switch tokenType {
    case lexer.TokenIdentifier:
        return "IDENTIFIER"
    case lexer.TokenNumber:
        return "NUMBER"
    case lexer.TokenString:
        return "STRING"
    case lexer.TokenFun:
        return "FUN"
    case lexer.TokenVar:
        return "VAR"
    case lexer.TokenConst:
        return "CONST"
    case lexer.TokenDef:
        return "DEF"
    default:
        return fmt.Sprintf("TOKEN(%d)", int(tokenType))
    }
}

func printAST(node ast.Node, indent int, w io.Writer) {
    indentStr := strings.Repeat("  ", indent)

    switch n := node.(type) {
    case *ast.Program:
        fmt.Fprintln(w, indentStr+"Program")
        for _, decl := range n.Declarations {
            printAST(decl, indent+1, w)
        }
    case *ast.FunctionDeclaration:
        fmt.Fprintf(w, "%sFunctionDeclaration: %s\n", indentStr, n.Name)
        fmt.Fprintf(w, "%s  Parameters: ", indentStr)
        for i, param := range n.Parameters {
            if i > 0 {
                fmt.Fprint(w, ", ")
            }
            fmt.Fprintf(w, "%s: %s", param.Name, param.Type)
        }
        fmt.Fprintln(w)
        if n.ReturnType != "" {
            fmt.Fprintf(w, "%s  ReturnType: %s\n", indentStr, n.ReturnType)
        }
        fmt.Fprintf(w, "%s  Body:\n", indentStr)
        for _, stmt := range n.Body {
            printAST(stmt, indent+2, w)
        }
    default:
        fmt.Fprintf(w, "%sNode: %T\n", indentStr, node)
    }
}

func startREPL(stdin io.Reader, stdout, stderr io.Writer) int {
    fmt.Fprintf(stdout, "Burn Programming Language v%s\n", getVersion())
    fmt.Fprintln(stdout, "Type 'exit' to quit, 'help' for more information")

    
    buf := make([]byte, 1024)
    
    for {
        fmt.Fprint(stdout, "> ")
        n, err := stdin.Read(buf)
        if err != nil {
            if err == io.EOF {
                break
            }
            fmt.Fprintf(stderr, "Error reading input: %v\n", err)
            continue
        }
        
        line := strings.TrimSpace(string(buf[:n]))
        if line == "" {
            continue
        }
        
        if line == "exit" || line == "quit" {
            return 0
        }
        
        if line == "help" {
            fmt.Fprintln(stdout, "Burn REPL commands:")
            fmt.Fprintln(stdout, "  exit, quit  - Exit the REPL")
            fmt.Fprintln(stdout, "  help        - Show this help message")
            continue
        }
        
        result, err := execute(line, false, stdout)
        if err != nil {
            fmt.Fprintf(stderr, "Error: %v\n", err)
        } else if result != nil {
            fmt.Fprintf(stdout, "=> %v\n", result)
        }
    }
    
    return 0
}
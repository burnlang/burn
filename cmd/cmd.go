package cmd

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/interpreter"
	"github.com/burnlang/burn/pkg/lexer"
	"github.com/burnlang/burn/pkg/parser"
	"github.com/burnlang/burn/pkg/stdlib"
	"github.com/burnlang/burn/pkg/typechecker"
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

	if options["exe"] {
		if len(nonOptions) == 0 {
			fmt.Fprintln(stderr, "Error: no source file provided for compilation")
			return 1
		}
		return compileToExecutable(nonOptions[0], nonOptions[len(nonOptions)-1], stdout, stderr)
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
		"exe":     false,
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
			case "-exe", "--executable":
				options["exe"] = true
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
	fmt.Fprintln(w, "  -exe, --executable  Compile to a standalone executable")
	fmt.Fprintln(w, "")
	fmt.Fprintln(w, "Examples:")
	fmt.Fprintln(w, "  burn main.bn              Execute a Burn program")
	fmt.Fprintln(w, "  burn -r                   Start REPL")
	fmt.Fprintln(w, "  burn -e 'print(\"Hello\")' Evaluate a single expression")
	fmt.Fprintln(w, "  burn -exe test/main.bn    Compile to executable")
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
		return nil, formattedError("Lexical error", err, source, lex.Position())
	}

	if debug {
		fmt.Fprintln(stdout, "--- Tokens ---")
		for _, token := range tokens {
			if token.Type != lexer.TokenEOF {
				fmt.Fprintf(stdout, "%s '%s' at position %d\n",
					tokenTypeToString(token.Type), token.Value, token.Position)
			}
		}
		fmt.Fprintln(stdout)
	}

	p := parser.New(tokens)
	program, err := p.Parse()
	if err != nil {
		return nil, formattedError("Parse error", err, source, p.Position())
	}

	if debug {
		fmt.Fprintln(stdout, "--- AST ---")
		printAST(program, 0, stdout)
		fmt.Fprintln(stdout)
	}

	tc := typechecker.New()
	if err := tc.Check(program); err != nil {
		return nil, formattedError("Type error", err, source, tc.Position())
	}

	if debug {
		fmt.Fprintln(stdout, "--- Type Check Passed ---")
		fmt.Fprintln(stdout)
	}

	interpreter := interpreter.New()
	result, err := interpreter.Interpret(program)
	if err != nil {
		return nil, formattedError("Runtime error", err, source, interpreter.Position())
	}

	return result, nil
}

func formattedError(errType string, err error, source string, pos int) error {
	errMsg := err.Error()

	if strings.Contains(errMsg, "at line") {
		return fmt.Errorf("%s: %v", errType, err)
	}

	if pos < 0 {
		pos = 0
	}
	if pos >= len(source) {
		pos = len(source) - 1
		if pos < 0 {
			pos = 0
		}
	}

	line, col := getLineAndCol(source, pos)
	return fmt.Errorf("%s at line %d, column %d: %v", errType, line, col, err)
}

func getLineAndCol(source string, pos int) (int, int) {
	lineStart := 0
	line := 1

	for i := 0; i < pos && i < len(source); i++ {
		if source[i] == '\n' {
			lineStart = i + 1
			line++
		}
	}

	column := pos - lineStart + 1
	return line, column
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
	case *ast.ClassDeclaration:
		fmt.Fprintf(w, "%sClassDeclaration: %s\n", indentStr, n.Name)
		fmt.Fprintf(w, "%s  Methods:\n", indentStr)
		for _, method := range n.Methods {
			printAST(method, indent+2, w)
		}
	case *ast.VariableDeclaration:
		fmt.Fprintf(w, "%sVariableDeclaration: %s", indentStr, n.Name)
		if n.Type != "" {
			fmt.Fprintf(w, " : %s", n.Type)
		}
		fmt.Fprintln(w)
		if n.Value != nil {
			printAST(n.Value, indent+1, w)
		}
	case *ast.ExpressionStatement:
		fmt.Fprintf(w, "%sExpressionStatement:\n", indentStr)
		printAST(n.Expression, indent+1, w)
	case *ast.CallExpression:
		fmt.Fprintf(w, "%sCallExpression: ", indentStr)
		printAST(n.Callee, indent, w)
		fmt.Fprintf(w, "%s  Arguments:\n", indentStr)
		for _, arg := range n.Arguments {
			printAST(arg, indent+2, w)
		}
	case *ast.ClassMethodCallExpression:
		fmt.Fprintf(w, "%sClassMethodCallExpression: %s.%s\n", indentStr, n.ClassName, n.MethodName)
		fmt.Fprintf(w, "%s  Arguments:\n", indentStr)
		for _, arg := range n.Arguments {
			printAST(arg, indent+2, w)
		}
	case *ast.LiteralExpression:
		fmt.Fprintf(w, "%sLiteral: %v (%T)\n", indentStr, n.Value, n.Value)
	case *ast.VariableExpression:
		fmt.Fprintf(w, "%sVariable: %s\n", indentStr, n.Name)
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

func compileToExecutable(sourceFile, outputName string, stdout, stderr io.Writer) int {
	if !strings.HasSuffix(sourceFile, ".bn") {
		fmt.Fprintf(stderr, "Warning: File %s does not have the .bn extension\n", sourceFile)
	}

	if outputName == sourceFile || outputName == "" {
		outputName = strings.TrimSuffix(filepath.Base(sourceFile), ".bn")
	}

	if !strings.HasSuffix(outputName, ".exe") {
		outputName += ".exe"
	}

	fmt.Fprintf(stdout, "Compiling %s to executable %s...\n", sourceFile, outputName)

	source, err := os.ReadFile(sourceFile)
	if err != nil {
		fmt.Fprintf(stderr, "Error reading source file: %v\n", err)
		return 1
	}

	lex := lexer.New(string(source))
	tokens, err := lex.Tokenize()
	if err != nil {
		fmt.Fprintf(stderr, "Lexical error: %v\n", err)
		return 1
	}

	p := parser.New(tokens)
	program, err := p.Parse()
	if err != nil {
		fmt.Fprintf(stderr, "Parse error: %v\n", err)
		return 1
	}

	tc := typechecker.New()
	if err := tc.Check(program); err != nil {
		fmt.Fprintf(stderr, "Type error: %v\n", err)
		return 1
	}

	tempDir, err := os.MkdirTemp("", "burn-build-")
	if err != nil {
		fmt.Fprintf(stderr, "Error creating build directory: %v\n", err)
		return 1
	}
	defer os.RemoveAll(tempDir)

	goFilePath := filepath.Join(tempDir, "main.go")
	err = createExecutableWrapper(goFilePath, sourceFile, string(source))
	if err != nil {
		fmt.Fprintf(stderr, "Error creating executable wrapper: %v\n", err)
		return 1
	}

	cmd := exec.Command("go", "build", "-o", outputName, goFilePath)
	cmd.Stdout = stdout
	cmd.Stderr = stderr
	if err := cmd.Run(); err != nil {
		fmt.Fprintf(stderr, "Error building executable: %v\n", err)
		return 1
	}

	fmt.Fprintf(stdout, "Successfully compiled %s to %s\n", sourceFile, outputName)
	return 0
}

func createExecutableWrapper(goFilePath, burnFilePath, burnSource string) error {
	imports, err := collectImports(burnFilePath, burnSource)
	if err != nil {
		return err
	}

	for name, content := range stdlib.StdLibFiles {
		stdlibPath := "src/lib/std/" + name + ".bn"
		if _, exists := imports[stdlibPath]; !exists {
			imports[stdlibPath] = content
		}

		if _, exists := imports[name]; !exists {
			imports[name] = content
		}
	}

	wrapperTemplate := `package main

import (
    "fmt"
    "os"

    "github.com/burnlang/burn/pkg/interpreter"
    "github.com/burnlang/burn/pkg/lexer"
    "github.com/burnlang/burn/pkg/parser"
    "github.com/burnlang/burn/pkg/typechecker"
)


var mainSource = %q


var importSources = map[string]string{
%s
}

func main() {
    exitCode := runBurnProgram()
    os.Exit(exitCode)
}

func runBurnProgram() int {
    
    interp := interpreter.New()
    
    
    for path, source := range importSources {
        if err := registerImport(interp, path, source); err != nil {
            fmt.Fprintf(os.Stderr, "Import error: %%v\n", err)
            return 1
        }
    }

    
    lex := lexer.New(mainSource)
    tokens, err := lex.Tokenize()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Lexical error: %%v\n", err)
        return 1
    }

    p := parser.New(tokens)
    program, err := p.Parse()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Parse error: %%v\n", err)
        return 1
    }

    tc := typechecker.New()
    if err := tc.Check(program); err != nil {
        fmt.Fprintf(os.Stderr, "Type error: %%v\n", err)
        return 1
    }

    _, err = interp.Interpret(program)
    if err != nil {
        fmt.Fprintf(os.Stderr, "Runtime error: %%v\n", err)
        return 1
    }

    return 0
}

func registerImport(interp *interpreter.Interpreter, path, source string) error {
    lex := lexer.New(source)
    tokens, err := lex.Tokenize()
    if err != nil {
        return err
    }

    p := parser.New(tokens)
    program, err := p.Parse()
    if err != nil {
        return err
    }
    
    importInterp := interpreter.New()
    _, err = importInterp.Interpret(program)
    if err != nil {
        return err
    }
    
    for name, fn := range importInterp.GetFunctions() {
        if name != "main" {
            interp.AddFunction(name, fn)
        }
    }

    return nil
}
`

	var importSourcesContent strings.Builder
	for path, source := range imports {
		importSourcesContent.WriteString(fmt.Sprintf("\t%q: %q,\n", path, source))
	}

	wrapperCode := fmt.Sprintf(wrapperTemplate, burnSource, importSourcesContent.String())

	return os.WriteFile(goFilePath, []byte(wrapperCode), 0644)
}

func collectImports(mainFile, mainSource string) (map[string]string, error) {
	imports := make(map[string]string)

	workingDir, err := os.Getwd()
	if err != nil {
		return nil, fmt.Errorf("error getting current directory: %v", err)
	}

	for name, content := range stdlib.StdLibFiles {
		imports[name] = content
		fmt.Printf("Including standard library %s (built-in)\n", name)
	}

	stdLibDir := filepath.Join(filepath.Dir(mainFile), "src", "lib", "std")
	if _, err := os.Stat(stdLibDir); err == nil {
		err = stdlib.AutoRegisterLibrariesFromDir(stdLibDir)
		if err == nil {

			for name, content := range stdlib.StdLibFiles {
				if _, exists := imports[name]; !exists {
					imports[name] = content
					fmt.Printf("Auto-discovered standard library %s\n", name)
				}
			}
		}
	}

	lex := lexer.New(mainSource)
	tokens, err := lex.Tokenize()
	if err != nil {
		return nil, err
	}

	p := parser.New(tokens)
	program, err := p.Parse()
	if err != nil {
		return nil, err
	}

	baseDir := filepath.Dir(mainFile)

	processImport := func(imp *ast.ImportDeclaration) error {
		moduleName := imp.Path
		if strings.HasSuffix(moduleName, ".bn") {
			moduleName = strings.TrimSuffix(moduleName, ".bn")
		}

		baseName := filepath.Base(moduleName)
		if strings.HasSuffix(baseName, ".bn") {
			baseName = strings.TrimSuffix(baseName, ".bn")
		}

		if _, exists := stdlib.StdLibFiles[baseName]; exists {
			return nil
		}

		var fileContent []byte
		var readErr error

		fileContent, readErr = os.ReadFile(imp.Path)
		if readErr == nil {
			imports[imp.Path] = string(fileContent)
			fmt.Printf("Including imported file %s\n", imp.Path)
			return collectNestedImports(imp.Path, string(fileContent), imports, workingDir, baseDir)
		}

		possiblePaths := []string{
			filepath.Join(baseDir, imp.Path),
			imp.Path + ".bn",
			filepath.Join(baseDir, imp.Path+".bn"),
		}

		for _, path := range possiblePaths {
			fileContent, readErr = os.ReadFile(path)
			if readErr == nil {
				imports[imp.Path] = string(fileContent)
				fmt.Printf("Including imported file %s\n", path)
				return collectNestedImports(path, string(fileContent), imports, workingDir, baseDir)
			}
		}

		return fmt.Errorf("could not find import '%s'", imp.Path)
	}

	for _, decl := range program.Declarations {
		if imp, ok := decl.(*ast.ImportDeclaration); ok {
			if err := processImport(imp); err != nil {
				return nil, err
			}
		}
		if multiImp, ok := decl.(*ast.MultiImportDeclaration); ok {
			for _, imp := range multiImp.Imports {
				if err := processImport(imp); err != nil {
					return nil, err
				}
			}
		}
	}

	return imports, nil
}

func collectNestedImports(filePath, source string, imports map[string]string, workingDir, originBaseDir string) error {
	lex := lexer.New(source)
	tokens, err := lex.Tokenize()
	if err != nil {
		return err
	}

	p := parser.New(tokens)
	program, err := p.Parse()
	if err != nil {
		return err
	}

	baseDir := filepath.Dir(filePath)

	processNestedImport := func(imp *ast.ImportDeclaration) error {
		if _, exists := imports[imp.Path]; exists {
			return nil
		}

		baseName := filepath.Base(imp.Path)
		if strings.HasSuffix(baseName, ".bn") {
			baseName = strings.TrimSuffix(baseName, ".bn")
		}

		if stdLib, exists := stdlib.StdLibFiles[baseName]; exists {
			imports[imp.Path] = stdLib
			fmt.Printf("Including standard library %s (built-in)\n", baseName)
			return nil
		}

		possiblePaths := []string{
			imp.Path,
			filepath.Join(baseDir, imp.Path),
			filepath.Join(workingDir, imp.Path),
			imp.Path + ".bn",
			filepath.Join(baseDir, imp.Path+".bn"),
			filepath.Join(workingDir, imp.Path+".bn"),
		}

		for _, path := range possiblePaths {
			fileContent, readErr := os.ReadFile(path)
			if readErr == nil {
				imports[imp.Path] = string(fileContent)
				fmt.Printf("Including nested import %s\n", path)
				return collectNestedImports(path, string(fileContent), imports, workingDir, originBaseDir)
			}
		}

		return fmt.Errorf("could not find nested import '%s'", imp.Path)
	}

	for _, decl := range program.Declarations {
		if imp, ok := decl.(*ast.ImportDeclaration); ok {
			if err := processNestedImport(imp); err != nil {
				return err
			}
		}
		if multiImp, ok := decl.(*ast.MultiImportDeclaration); ok {
			for _, imp := range multiImp.Imports {
				if err := processNestedImport(imp); err != nil {
					return err
				}
			}
		}
	}

	return nil
}

package cmd

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/lexer"
	"github.com/burnlang/burn/pkg/parser"
	"github.com/burnlang/burn/pkg/stdlib"
	"github.com/burnlang/burn/pkg/typechecker"
)

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
	if err := tc.Check(program.Declarations); err != nil {
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
	if err := tc.Check(program.Declarations); err != nil {
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

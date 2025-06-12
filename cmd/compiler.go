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

	wrapperTemplate := `package main

import (
    "fmt"
    "os"
    "path/filepath"
    "strings"

    "github.com/burnlang/burn/pkg/interpreter"
    "github.com/burnlang/burn/pkg/lexer"
    "github.com/burnlang/burn/pkg/parser"
)

func main() {
    mainSource := %s

    imports := map[string]string{
%s
    }

    l := lexer.New(mainSource)
    tokens, err := l.Tokenize()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Lexical error: %%v\n", err)
        os.Exit(1)
    }

    p := parser.New(tokens)
    program, err := p.Parse()
    if err != nil {
        fmt.Fprintf(os.Stderr, "Parse error: %%v\n", err)
        os.Exit(1)
    }

    i := interpreter.New()

    
    for importPath, importSource := range imports {
        if err := processImport(i, importPath, importSource); err != nil {
            fmt.Fprintf(os.Stderr, "Import error for '%%s': %%v\n", importPath, err)
            os.Exit(1)
        }
    }

    _, err = i.Interpret(program)
    if err != nil {
        fmt.Fprintf(os.Stderr, "Runtime error: %%v\n", err)
        os.Exit(1)
    }
}

func processImport(i *interpreter.Interpreter, importPath, importSource string) error {
    
    if strings.HasPrefix(importPath, "std/") || 
       (!strings.Contains(importPath, "/") && !strings.Contains(importPath, "\\") && 
        (importPath == "date" || importPath == "http" || importPath == "time")) {
        return nil
    }

    l := lexer.New(importSource)
    tokens, err := l.Tokenize()
    if err != nil {
        return err
    }

    p := parser.New(tokens)
    program, err := p.Parse()
    if err != nil {
        return err
    }

    importInterpreter := interpreter.New()

    _, err = importInterpreter.Interpret(program)
    if err != nil {
        return err
    }

    
    for name, fn := range importInterpreter.GetFunctions() {
        if name != "main" {
            i.AddFunction(name, fn)
        }
    }

    for name, value := range importInterpreter.GetVariables() {
        i.AddVariable(name, value)
    }

    return nil
}
`

	var importStrings []string
	for name, content := range imports {

		if strings.HasPrefix(name, "std/") ||
			(!strings.Contains(name, "/") && !strings.Contains(name, "\\") &&
				(name == "date" || name == "http" || name == "time")) {
			continue
		}

		escapedContent := strings.ReplaceAll(content, "`", "` + \"`\" + `")
		importStrings = append(importStrings, fmt.Sprintf("        %q: `%s`,", name, escapedContent))
	}

	escapedSource := strings.ReplaceAll(burnSource, "`", "` + \"`\" + `")

	finalCode := fmt.Sprintf(wrapperTemplate, fmt.Sprintf("`%s`", escapedSource), strings.Join(importStrings, "\n"))

	return os.WriteFile(goFilePath, []byte(finalCode), 0644)
}

func collectImports(mainFile, mainSource string) (map[string]string, error) {
	imports := make(map[string]string)

	workingDir, err := os.Getwd()
	if err != nil {
		return nil, fmt.Errorf("error getting current directory: %v", err)
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

		if strings.HasPrefix(imp.Path, "std/") ||
			(!strings.Contains(imp.Path, "/") && !strings.Contains(imp.Path, "\\") &&
				(imp.Path == "date" || imp.Path == "http" || imp.Path == "time")) {
			return nil
		}

		var fileContent []byte
		var readErr error

		possiblePaths := []string{
			imp.Path,
			filepath.Join(baseDir, imp.Path),
			filepath.Join(workingDir, imp.Path),
			imp.Path + ".bn",
			filepath.Join(baseDir, imp.Path+".bn"),
			filepath.Join(workingDir, imp.Path+".bn"),
			filepath.Join(baseDir, "test", imp.Path),
			filepath.Join(baseDir, "test", imp.Path+".bn"),
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

		if strings.HasPrefix(imp.Path, "std/") ||
			(!strings.Contains(imp.Path, "/") && !strings.Contains(imp.Path, "\\") &&
				(imp.Path == "date" || imp.Path == "http" || imp.Path == "time")) {
			return nil
		}

		possiblePaths := []string{
			imp.Path,
			filepath.Join(baseDir, imp.Path),
			filepath.Join(workingDir, imp.Path),
			imp.Path + ".bn",
			filepath.Join(baseDir, imp.Path+".bn"),
			filepath.Join(workingDir, imp.Path+".bn"),
			filepath.Join(originBaseDir, "src", "lib", imp.Path),
			filepath.Join(originBaseDir, "src", "lib", imp.Path+".bn"),
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

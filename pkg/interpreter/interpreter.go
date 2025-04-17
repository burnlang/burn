package interpreter

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/lexer"
	"github.com/burnlang/burn/pkg/parser"
	"github.com/burnlang/burn/pkg/stdlib"
)

type Interpreter struct {
	environment map[string]Value
	functions   map[string]*ast.FunctionDeclaration
	types       map[string]*ast.TypeDefinition
	classes     map[string]*Class
	errorPos    int

	importedModules map[string]bool
}

type Environment struct {
	enclosing *Environment
	values    map[string]interface{}
}

func NewEnvironment(enclosing *Environment) *Environment {
	return &Environment{
		enclosing: enclosing,
		values:    make(map[string]interface{}),
	}
}

func New() *Interpreter {
	i := &Interpreter{
		environment:     make(map[string]Value),
		functions:       make(map[string]*ast.FunctionDeclaration),
		types:           make(map[string]*ast.TypeDefinition),
		classes:         make(map[string]*Class),
		errorPos:        0,
		importedModules: make(map[string]bool),
	}
	i.addBuiltins()
	return i
}

func (i *Interpreter) RegisterBuiltinStandardLibraries() {

	i.registerDateLibrary()
	i.registerHTTPLibrary()
	i.registerTimeLibrary()

	for name, lib := range stdlib.StdLibFiles {
		if name == "date" || name == "http" || name == "time" {

			continue
		}
		_ = i.interpretStdLib(name, lib)
	}
}

func (i *Interpreter) Interpret(program *ast.Program) (Value, error) {

	for _, decl := range program.Declarations {
		if typeDef, ok := decl.(*ast.TypeDefinition); ok {
			i.types[typeDef.Name] = typeDef
		} else if classDef, ok := decl.(*ast.ClassDeclaration); ok {
			class := NewClass(classDef.Name)
			for _, method := range classDef.Methods {
				class.AddMethod(method.Name, method)
			}
			for _, method := range classDef.StaticMethods {
				class.AddStatic(method.Name, method)
			}
			i.classes[classDef.Name] = class
		}
	}

	i.addBuiltins()

	i.RegisterBuiltinStandardLibraries()

	for _, decl := range program.Declarations {
		if fn, ok := decl.(*ast.FunctionDeclaration); ok {
			i.functions[fn.Name] = fn
		}
		if imp, ok := decl.(*ast.ImportDeclaration); ok {
			if err := i.handleImport(imp); err != nil {
				return nil, err
			}
		}
		if multiImp, ok := decl.(*ast.MultiImportDeclaration); ok {
			for _, imp := range multiImp.Imports {
				if err := i.handleImport(imp); err != nil {
					return nil, err
				}
			}
		}
	}

	if mainFn, exists := i.functions["main"]; exists {
		return i.executeFunction(mainFn, []Value{})
	}

	var result Value
	for _, decl := range program.Declarations {
		var err error
		result, err = i.executeDeclaration(decl)
		if err != nil {
			return nil, err
		}
	}

	return result, nil
}

func (i *Interpreter) handleImport(imp *ast.ImportDeclaration) error {
	libName := imp.Path

	if i.importedModules[libName] {
		return nil 
	}

	i.importedModules[libName] = true

	
	if strings.HasPrefix(libName, "std/") || (!strings.Contains(libName, "/") && !strings.Contains(libName, "\\")) {
		basename := strings.TrimPrefix(libName, "std/")
		basename = strings.TrimSuffix(basename, ".bn")

		switch basename {
		case "date":
			i.registerDateLibrary()
			return nil
		case "http":
			i.registerHTTPLibrary()
			return nil
		case "time":
			i.registerTimeLibrary()
			return nil
		}
	}

	
	if strings.HasSuffix(libName, ".bn") || !strings.Contains(libName, ".") {
		path := libName

		if !strings.HasSuffix(path, ".bn") {
			path = path + ".bn"
		}

		
		workingDir, err := os.Getwd()
		if err != nil {
			return fmt.Errorf("error getting current directory: %v", err)
		}

		
		searchPaths := []string{
			path,
			filepath.Join(workingDir, path),
			filepath.Join("src", "lib", "std", path),
			filepath.Join("src", "lib", path),
			filepath.Join("src", "lib", "std", strings.TrimSuffix(path, ".bn")+".bn"),
			filepath.Join("src", "lib", strings.TrimSuffix(path, ".bn")+".bn"),
			
			filepath.Join("test", strings.TrimPrefix(path, "test/")),
		}

		var source []byte
		var foundPath string

		for _, searchPath := range searchPaths {
			source, err = os.ReadFile(searchPath)
			if err == nil {
				foundPath = searchPath
				break
			}
		}

		if foundPath == "" {
			return fmt.Errorf("could not find import file: %s (tried paths: %v)", libName, searchPaths)
		}

		l := lexer.New(string(source))
		tokens, err := l.Tokenize()
		if err != nil {
			return fmt.Errorf("lexical error in import %s: %v", foundPath, err)
		}

		p := parser.New(tokens)
		program, err := p.Parse()
		if err != nil {
			return fmt.Errorf("parse error in import %s: %v", foundPath, err)
		}

		
		importInterpreter := New()
		importInterpreter.addBuiltins()
		importInterpreter.RegisterBuiltinStandardLibraries()

		
		for mod := range i.importedModules {
			importInterpreter.importedModules[mod] = true
		}

		_, err = importInterpreter.Interpret(program)
		if err != nil {
			return fmt.Errorf("error interpreting import %s: %v", foundPath, err)
		}

		
		for name, typeDef := range importInterpreter.types {
			i.types[name] = typeDef
		}

		for name, fn := range importInterpreter.functions {
			if name != "main" { 
				i.functions[name] = fn
			}
		}

		for name, class := range importInterpreter.classes {
			i.classes[name] = class
		}

		for name, value := range importInterpreter.environment {
			if _, exists := i.environment[name]; !exists {
				i.environment[name] = value
			}
		}

		return nil
	}

	
	basename := filepath.Base(libName)
	if lib, exists := stdlib.StdLibFiles[basename]; exists {
		switch basename {
		case "date":
			i.registerDateLibrary()
		case "http":
			i.registerHTTPLibrary()
		case "time":
			i.registerTimeLibrary()
		default:
			return i.interpretStdLib(basename, lib)
		}
		return nil
	}

	return fmt.Errorf("could not find import: %s", imp.Path)
}

func (i *Interpreter) interpretStdLib(name, source string) error {
	l := lexer.New(source)
	tokens, err := l.Tokenize()
	if err != nil {
		return err
	}

	p := parser.New(tokens)
	program, err := p.Parse()
	if err != nil {
		return err
	}

	importInterpreter := New()
	importInterpreter.addBuiltins()
	importInterpreter.RegisterBuiltinStandardLibraries()

	_, err = importInterpreter.Interpret(program)
	if err != nil {
		return err
	}

	for name, fn := range importInterpreter.functions {
		if name != "main" {
			i.functions[name] = fn
		}
	}

	for name, class := range importInterpreter.classes {
		i.classes[name] = class
	}

	for name, value := range importInterpreter.environment {
		if _, exists := i.environment[name]; !exists {
			i.environment[name] = value
		}
	}

	return nil
}

func (i *Interpreter) executeDeclaration(decl ast.Declaration) (Value, error) {
	if decl != nil {
		i.setErrorPos(decl.Pos())
	}

	switch d := decl.(type) {
	case *ast.ClassDeclaration:
		return nil, nil
	case *ast.TypeDefinition:
		return nil, nil
	case *ast.FunctionDeclaration:
		i.functions[d.Name] = d
		return nil, nil
	case *ast.VariableDeclaration:
		if d.Value != nil {
			value, err := i.evaluateExpression(d.Value)
			if err != nil {
				return nil, err
			}
			i.environment[d.Name] = value
		}
		return nil, nil
	case *ast.ExpressionStatement:
		return i.evaluateExpression(d.Expression)
	case *ast.ReturnStatement:
		if d.Value == nil {
			return nil, nil
		}
		return i.evaluateExpression(d.Value)
	case *ast.IfStatement:
		condition, err := i.evaluateExpression(d.Condition)
		if err != nil {
			return nil, err
		}

		if cond, ok := condition.(bool); ok {
			if cond {
				for _, stmt := range d.ThenBranch {
					result, err := i.executeDeclaration(stmt)
					if err != nil {
						return nil, err
					}
					if _, ok := stmt.(*ast.ReturnStatement); ok {
						return result, nil
					}
				}
			} else if d.ElseBranch != nil {
				for _, stmt := range d.ElseBranch {
					result, err := i.executeDeclaration(stmt)
					if err != nil {
						return nil, err
					}
					if _, ok := stmt.(*ast.ReturnStatement); ok {
						return result, nil
					}
				}
			}
		}
		return nil, nil
	case *ast.WhileStatement:
		for {
			condition, err := i.evaluateExpression(d.Condition)
			if err != nil {
				return nil, err
			}

			if cond, ok := condition.(bool); ok && cond {
				for _, stmt := range d.Body {
					result, err := i.executeDeclaration(stmt)
					if err != nil {
						return nil, err
					}
					if _, ok := stmt.(*ast.ReturnStatement); ok {
						return result, nil
					}
				}
			} else {
				break
			}
		}
		return nil, nil
	case *ast.ForStatement:
		if d.Initializer != nil {
			_, err := i.executeDeclaration(d.Initializer)
			if err != nil {
				return nil, err
			}
		}

		for {
			if d.Condition != nil {
				condition, err := i.evaluateExpression(d.Condition)
				if err != nil {
					return nil, err
				}
				if cond, ok := condition.(bool); !ok || !cond {
					break
				}
			}

			for _, stmt := range d.Body {
				result, err := i.executeDeclaration(stmt)
				if err != nil {
					return nil, err
				}
				if _, ok := stmt.(*ast.ReturnStatement); ok {
					return result, nil
				}
			}

			if d.Increment != nil {
				_, err := i.evaluateExpression(d.Increment)
				if err != nil {
					return nil, err
				}
			}
		}
		return nil, nil
	default:
		return nil, fmt.Errorf("unknown declaration type: %T", decl)
	}
}

func (i *Interpreter) executeBuiltin(name string, args []Value) (Value, error) {
	if builtinFunc, ok := i.environment[name]; ok {
		if bf, ok := builtinFunc.(*BuiltinFunction); ok {
			return bf.Call(args)
		}
	}
	return nil, fmt.Errorf("undefined builtin function: %s", name)
}

func (i *Interpreter) executeFunction(fn *ast.FunctionDeclaration, args []Value) (Value, error) {
	if fn.Body == nil {
		return i.executeBuiltin(fn.Name, args)
	}

	prevEnv := make(map[string]Value)
	for k, v := range i.environment {
		prevEnv[k] = v
	}

	newEnv := make(map[string]Value)

	for k, v := range i.environment {
		if _, ok := v.(*BuiltinFunction); ok {
			newEnv[k] = v
		}
	}

	i.environment = newEnv

	for j, param := range fn.Parameters {
		if j < len(args) {
			i.environment[param.Name] = args[j]
		}
	}

	var result Value
	for _, stmt := range fn.Body {
		var err error
		result, err = i.executeDeclaration(stmt)
		if err != nil {
			return nil, err
		}
	}

	i.environment = prevEnv

	return result, nil
}

func (i *Interpreter) GetVariables() map[string]interface{} {
	if i.environment == nil {
		return make(map[string]interface{})
	}
	result := make(map[string]interface{})
	for k, v := range i.environment {
		result[k] = v
	}
	return result
}

func (i *Interpreter) setErrorPos(pos int) {
	i.errorPos = pos
}

func (i *Interpreter) Position() int {
	return i.errorPos
}

func (i *Interpreter) AddFunction(name string, fn *ast.FunctionDeclaration) {
	i.functions[name] = fn
}

func (i *Interpreter) GetFunctions() map[string]*ast.FunctionDeclaration {
	return i.functions
}

func (i *Interpreter) AddVariable(name string, value interface{}) {
	if _, exists := i.environment[name]; !exists {
		i.environment[name] = value
	}
}

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
	classes     map[string]*Class
	errorPos    int
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
		environment: make(map[string]Value),
		functions:   make(map[string]*ast.FunctionDeclaration),
		classes:     make(map[string]*Class),
		errorPos:    0,
	}
	i.addBuiltins()
	return i
}

func (i *Interpreter) RegisterBuiltinStandardLibraries() {
	i.registerDateLibrary()
	i.registerHTTPLibrary()
	i.registerTimeLibrary()
}

func (i *Interpreter) Interpret(program *ast.Program) (Value, error) {

	for _, decl := range program.Declarations {
		if classDef, ok := decl.(*ast.ClassDeclaration); ok {
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
	
	if strings.HasSuffix(libName, ".bn") || !strings.Contains(libName, ".") {
		path := libName
		
		if !strings.HasSuffix(path, ".bn") {
			path = path + ".bn"
		}

		source, err := os.ReadFile(path)
		if err != nil {
			return fmt.Errorf("could not read imported file %s: %v", path, err)
		}

		l := lexer.New(string(source))
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

		
		defer func() {
			
			for name, fn := range importInterpreter.functions {
				if name != "main" {
					i.functions[name] = fn
				}
			}

			
			for name, class := range importInterpreter.classes {
				i.classes[name] = class
			}
		}()

		_, err = importInterpreter.Interpret(program)
		if err != nil {
			return err
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

	i.environment = make(map[string]Value)

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

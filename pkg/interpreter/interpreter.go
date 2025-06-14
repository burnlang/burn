package interpreter

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/lexer"
	"github.com/burnlang/burn/pkg/parser"
)

type Interpreter struct {
	environment map[string]Value
	functions   map[string]*ast.FunctionDeclaration
	types       map[string]*ast.TypeDefinition
	classes     map[string]*Class
	errorPos    int

	importedModules map[string]bool

	stdout io.Writer
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
	return &Interpreter{
		environment:     make(map[string]Value),
		functions:       make(map[string]*ast.FunctionDeclaration),
		types:           make(map[string]*ast.TypeDefinition),
		classes:         make(map[string]*Class),
		importedModules: make(map[string]bool),
		stdout:          os.Stdout,
	}
}

func (i *Interpreter) Interpret(program *ast.Program) (Value, error) {
	var result Value

	for _, decl := range program.Declarations {
		if imp, ok := decl.(*ast.ImportDeclaration); ok {
			if err := i.handleImport(imp); err != nil {
				return nil, err
			}
			continue
		}

		if multiImp, ok := decl.(*ast.MultiImportDeclaration); ok {
			for _, imp := range multiImp.Imports {
				if err := i.handleImport(imp); err != nil {
					return nil, err
				}
			}
			continue
		}

		val, err := i.executeDeclaration(decl)
		if err != nil {
			return nil, err
		}
		result = val
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

			return nil
		case "http":

			return nil
		case "time":

			return nil
		default:
			return fmt.Errorf("unknown standard library: %s", basename)
		}
	}

	if strings.HasSuffix(libName, ".bn") || !strings.Contains(libName, ".") {
		return i.handleFileImport(libName)
	}

	return fmt.Errorf("could not find import: %s", imp.Path)
}

func (i *Interpreter) handleFileImport(libName string) error {
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
		filepath.Join("test", path),
		filepath.Join("src", path),
		filepath.Join(".", path),
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

func (i *Interpreter) callBuiltinFunction(name string, args []interface{}) (interface{}, error) {
	switch name {
	case "print":

		if len(args) == 0 {
			fmt.Fprintln(i.stdout)
			return nil, nil
		}

		var output strings.Builder
		for j, arg := range args {
			if j > 0 {
				output.WriteString(" ")
			}

			switch v := arg.(type) {
			case string:
				output.WriteString(v)
			case int:
				output.WriteString(strconv.Itoa(v))
			case int64:
				output.WriteString(strconv.FormatInt(v, 10))
			case float64:
				output.WriteString(strconv.FormatFloat(v, 'f', -1, 64))
			case bool:
				output.WriteString(strconv.FormatBool(v))
			case nil:
				output.WriteString("null")
			default:
				if stringer, ok := v.(interface{ String() string }); ok {
					output.WriteString(stringer.String())
				} else {
					output.WriteString(fmt.Sprintf("%v", v))
				}
			}
		}

		fmt.Fprintln(i.stdout, output.String())

		return nil, nil

	}

	return nil, fmt.Errorf("unknown builtin function: %s", name)
}

func (i *Interpreter) SetStdout(w io.Writer) {
	i.stdout = w
}

package typechecker

import (
	"fmt"
	"io/ioutil"
	"path/filepath"

	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/lexer"
	"github.com/burnlang/burn/pkg/parser"
)

type FunctionType struct {
	Parameters []string
	ReturnType string
}

type TypeChecker struct {
	types      map[string]map[string]string
	functions  map[string]FunctionType
	variables  map[string]string
	classes    map[string]map[string]FunctionType
	arrayTypes map[string]string
	currentFn  string
	errorPos   int
}

func New() *TypeChecker {
	tc := &TypeChecker{
		types:      make(map[string]map[string]string),
		functions:  make(map[string]FunctionType),
		variables:  make(map[string]string),
		classes:    make(map[string]map[string]FunctionType),
		arrayTypes: make(map[string]string),
		currentFn:  "",
		errorPos:   0,
	}

	initStandardLibrary(tc)
	return tc
}

func (t *TypeChecker) Check(program []ast.Declaration) error {

	if err := t.registerTypes(program); err != nil {
		return err
	}

	if err := t.registerFunctions(program); err != nil {
		return err
	}

	for _, decl := range program {
		if err := t.checkDeclaration(decl); err != nil {
			return err
		}
	}

	return nil
}

func (t *TypeChecker) registerTypes(program []ast.Declaration) error {
	for _, decl := range program {
		if typeDef, ok := decl.(*ast.TypeDefinition); ok {
			if err := t.checkTypeDefinition(typeDef); err != nil {
				return err
			}
		}
	}
	return nil
}

func (t *TypeChecker) registerFunctions(program []ast.Declaration) error {
	for _, decl := range program {
		if fn, ok := decl.(*ast.FunctionDeclaration); ok {
			if err := t.registerFunction(fn); err != nil {
				return err
			}
		} else if class, ok := decl.(*ast.ClassDeclaration); ok {
			if err := t.registerClass(class); err != nil {
				return err
			}
		}
	}
	return nil
}

func (t *TypeChecker) registerFunction(fn *ast.FunctionDeclaration) error {
	if _, exists := t.functions[fn.Name]; exists {
		return fmt.Errorf("function %s is already defined", fn.Name)
	}

	paramTypes := make([]string, len(fn.Parameters))
	for i, param := range fn.Parameters {
		paramTypes[i] = param.Type
	}

	t.functions[fn.Name] = FunctionType{
		Parameters: paramTypes,
		ReturnType: fn.ReturnType,
	}

	return nil
}

func (t *TypeChecker) registerClass(class *ast.ClassDeclaration) error {
	if _, exists := t.classes[class.Name]; exists {
		return fmt.Errorf("class %s is already defined", class.Name)
	}

	classMethods := make(map[string]FunctionType)
	t.classes[class.Name] = classMethods

	t.types[class.Name] = make(map[string]string)

	for _, method := range class.Methods {
		if _, exists := classMethods[method.Name]; exists {
			return fmt.Errorf("method %s is already defined in class %s", method.Name, class.Name)
		}

		paramTypes := make([]string, len(method.Parameters))
		for i, param := range method.Parameters {
			paramTypes[i] = param.Type
		}

		classMethods[method.Name] = FunctionType{
			Parameters: paramTypes,
			ReturnType: method.ReturnType,
		}

		t.functions[class.Name+"."+method.Name] = FunctionType{
			Parameters: paramTypes,
			ReturnType: method.ReturnType,
		}
	}

	for _, method := range class.StaticMethods {
		methodKey := "static." + method.Name
		if _, exists := classMethods[methodKey]; exists {
			return fmt.Errorf("static method %s is already defined in class %s", method.Name, class.Name)
		}

		paramTypes := make([]string, len(method.Parameters))
		for i, param := range method.Parameters {
			paramTypes[i] = param.Type
		}

		classMethods[methodKey] = FunctionType{
			Parameters: paramTypes,
			ReturnType: method.ReturnType,
		}

		t.functions[class.Name+".static."+method.Name] = FunctionType{
			Parameters: paramTypes,
			ReturnType: method.ReturnType,
		}
	}

	return nil
}

func (t *TypeChecker) CheckFile(filename string) error {
	data, err := ioutil.ReadFile(filename)
	if err != nil {
		return err
	}

	l := lexer.New(string(data))
	tokens, err := l.Tokenize()
	if err != nil {
		return err
	}

	p := parser.New(tokens)
	program, err := p.Parse()
	if err != nil {
		return err
	}

	if err := t.processImports(program.Declarations, filepath.Dir(filename)); err != nil {
		return err
	}

	return t.Check(program.Declarations)
}

func (t *TypeChecker) processImports(program []ast.Declaration, baseDir string) error {
	for _, decl := range program {
		if imp, ok := decl.(*ast.ImportDeclaration); ok {
			if err := t.processImport(imp, baseDir); err != nil {
				return err
			}
		} else if multiImp, ok := decl.(*ast.MultiImportDeclaration); ok {
			for _, imp := range multiImp.Imports {
				if err := t.processImport(imp, baseDir); err != nil {
					return err
				}
			}
		}
	}
	return nil
}

func (t *TypeChecker) processImport(imp *ast.ImportDeclaration, baseDir string) error {
	importPath := filepath.Join(baseDir, imp.Path)
	data, err := ioutil.ReadFile(importPath)
	if err != nil {
		return fmt.Errorf("could not import %s: %v", imp.Path, err)
	}

	l := lexer.New(string(data))
	tokens, err := l.Tokenize()
	if err != nil {
		return fmt.Errorf("lexical error in import %s: %v", imp.Path, err)
	}

	p := parser.New(tokens)
	importProgram, err := p.Parse()
	if err != nil {
		return fmt.Errorf("parse error in import %s: %v", imp.Path, err)
	}

	return t.registerImportedDeclarations(importProgram.Declarations, imp)
}

func (t *TypeChecker) registerImportedDeclarations(declarations []ast.Declaration, imp *ast.ImportDeclaration) error {
	
	for _, decl := range declarations {
		if typeDef, ok := decl.(*ast.TypeDefinition); ok {
			
			if _, exists := t.types[typeDef.Name]; exists {
				continue
			}

			
			fields := make(map[string]string)
			for _, field := range typeDef.Fields {
				fields[field.Name] = field.Type
			}
			t.types[typeDef.Name] = fields

		} else if class, ok := decl.(*ast.ClassDeclaration); ok {
			
			if _, exists := t.classes[class.Name]; exists {
				continue
			}

			
			if _, exists := t.types[class.Name]; !exists {
				t.types[class.Name] = make(map[string]string)
			}
		}
	}

	
	for _, decl := range declarations {
		if fn, ok := decl.(*ast.FunctionDeclaration); ok {
			
			if _, exists := t.functions[fn.Name]; exists {
				continue
			}

			paramTypes := make([]string, len(fn.Parameters))
			for i, param := range fn.Parameters {
				paramTypes[i] = param.Type
			}

			t.functions[fn.Name] = FunctionType{
				Parameters: paramTypes,
				ReturnType: fn.ReturnType,
			}
		} else if class, ok := decl.(*ast.ClassDeclaration); ok {
			
			if _, exists := t.classes[class.Name]; !exists {
				classMethods := make(map[string]FunctionType)
				t.classes[class.Name] = classMethods

				
				for _, method := range class.Methods {
					paramTypes := make([]string, len(method.Parameters))
					for i, param := range method.Parameters {
						paramTypes[i] = param.Type
					}

					classMethods[method.Name] = FunctionType{
						Parameters: paramTypes,
						ReturnType: method.ReturnType,
					}

					t.functions[class.Name+"."+method.Name] = FunctionType{
						Parameters: paramTypes,
						ReturnType: method.ReturnType,
					}
				}

				
				for _, method := range class.StaticMethods {
					methodKey := "static." + method.Name
					paramTypes := make([]string, len(method.Parameters))
					for i, param := range method.Parameters {
						paramTypes[i] = param.Type
					}

					classMethods[methodKey] = FunctionType{
						Parameters: paramTypes,
						ReturnType: method.ReturnType,
					}

					t.functions[class.Name+".static."+method.Name] = FunctionType{
						Parameters: paramTypes,
						ReturnType: method.ReturnType,
					}
				}
			}
		}
	}

	return nil
}

func (t *TypeChecker) setErrorPos(pos int) {
	t.errorPos = pos
}

func (t *TypeChecker) Position() int {
	return t.errorPos
}

package typechecker

import (
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"

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
	BaseDir    string
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
		BaseDir:    ".",
	}

	initStandardLibrary(tc)
	return tc
}

func initStandardLibrary(tc *TypeChecker) {

	tc.functions["print"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "void",
	}

	tc.functions["println"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "void",
	}

	tc.functions["input"] = FunctionType{
		Parameters: []string{},
		ReturnType: "string",
	}

	tc.functions["toString"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "string",
	}

	tc.functions["toInt"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "int",
	}

	tc.functions["toFloat"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "float",
	}

	tc.functions["len"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "int",
	}
}

func (t *TypeChecker) Check(program []ast.Declaration) error {

	if err := t.processImports(program, t.BaseDir); err != nil {
		return err
	}

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

	t.BaseDir = filepath.Dir(filename)

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

	if err := t.processImports(program.Declarations, t.BaseDir); err != nil {
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

	if strings.HasPrefix(imp.Path, "std/") ||
		(!strings.Contains(imp.Path, "/") && !strings.Contains(imp.Path, "\\") &&
			(imp.Path == "date" || imp.Path == "http" || imp.Path == "time")) {

		basename := strings.TrimPrefix(imp.Path, "std/")
		basename = strings.TrimSuffix(basename, ".bn")

		className := basename
		if imp.Alias != "" {
			className = imp.Alias
		}

		switch basename {
		case "date":
			t.registerDateLibrary(className)
			return nil
		case "http":
			t.registerHTTPLibrary(className)
			return nil
		case "time":
			t.registerTimeLibrary(className)
			return nil
		default:
			return fmt.Errorf("standard library module '%s' not found", basename)
		}
	}

	path := imp.Path
	if !strings.HasSuffix(path, ".bn") {
		path = path + ".bn"
	}

	searchPaths := []string{
		path,
		filepath.Join(baseDir, path),
		filepath.Join("test", path),
		filepath.Join("src", path),
		filepath.Join(".", path),
	}

	if workingDir, err := os.Getwd(); err == nil {
		searchPaths = append(searchPaths,
			filepath.Join(workingDir, path),
			filepath.Join(workingDir, "test", path),
			filepath.Join(workingDir, "src", path),
		)
	}

	var data []byte
	var err error
	var foundPath string

	for _, searchPath := range searchPaths {
		data, err = ioutil.ReadFile(searchPath)
		if err == nil {
			foundPath = searchPath
			break
		}
	}

	if foundPath == "" {
		return fmt.Errorf("could not import %s: file not found in search paths %v", imp.Path, searchPaths)
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

func (t *TypeChecker) qualifyType(typeName string, aliasPrefix string, localTypes map[string]struct{}) string {
	if aliasPrefix == "" {
		return typeName
	}

	if strings.HasPrefix(typeName, "[]") {
		baseType := strings.TrimPrefix(typeName, "[]")
		if _, isLocalBase := localTypes[baseType]; isLocalBase {
			return "[]" + aliasPrefix + baseType
		}
	}

	if _, isLocal := localTypes[typeName]; isLocal {
		return aliasPrefix + typeName
	}

	return typeName
}

func (t *TypeChecker) registerImportedDeclarations(declarations []ast.Declaration, imp *ast.ImportDeclaration) error {

	aliasPrefix := ""
	if imp.Alias != "" {
		aliasPrefix = imp.Alias + "."
	}

	localTypesInImport := make(map[string]struct{})
	for _, decl := range declarations {
		if td, ok := decl.(*ast.TypeDefinition); ok {
			localTypesInImport[td.Name] = struct{}{}
		} else if cd, ok := decl.(*ast.ClassDeclaration); ok {
			localTypesInImport[cd.Name] = struct{}{}
		}
	}

	for _, decl := range declarations {
		switch d := decl.(type) {
		case *ast.FunctionDeclaration:
			fnName := aliasPrefix + d.Name
			if _, exists := t.functions[fnName]; exists {
				return fmt.Errorf("imported function %s is already defined", fnName)
			}

			paramTypes := make([]string, len(d.Parameters))
			for i, param := range d.Parameters {
				paramTypes[i] = t.qualifyType(param.Type, aliasPrefix, localTypesInImport)
			}
			returnType := t.qualifyType(d.ReturnType, aliasPrefix, localTypesInImport)

			t.functions[fnName] = FunctionType{
				Parameters: paramTypes,
				ReturnType: returnType,
			}

		case *ast.ClassDeclaration:
			className := aliasPrefix + d.Name
			if _, exists := t.classes[className]; exists {
				return fmt.Errorf("imported class %s is already defined", className)
			}

			classMethods := make(map[string]FunctionType)
			t.classes[className] = classMethods

			t.types[className] = make(map[string]string)

			for _, method := range d.Methods {
				methodName := method.Name
				if _, exists := classMethods[methodName]; exists {
					return fmt.Errorf("method %s is already defined in imported class %s", methodName, className)
				}

				paramTypes := make([]string, len(method.Parameters))
				for i, param := range method.Parameters {
					paramTypes[i] = t.qualifyType(param.Type, aliasPrefix, localTypesInImport)
				}
				returnType := t.qualifyType(method.ReturnType, aliasPrefix, localTypesInImport)

				classMethods[methodName] = FunctionType{
					Parameters: paramTypes,
					ReturnType: returnType,
				}

				t.functions[className+"."+methodName] = FunctionType{
					Parameters: paramTypes,
					ReturnType: returnType,
				}
			}

			for _, method := range d.StaticMethods {
				methodKey := "static." + method.Name
				if _, exists := classMethods[methodKey]; exists {
					return fmt.Errorf("static method %s is already defined in imported class %s", method.Name, className)
				}
				paramTypes := make([]string, len(method.Parameters))
				for i, param := range method.Parameters {
					paramTypes[i] = t.qualifyType(param.Type, aliasPrefix, localTypesInImport)
				}
				returnType := t.qualifyType(method.ReturnType, aliasPrefix, localTypesInImport)

				classMethods[methodKey] = FunctionType{
					Parameters: paramTypes,
					ReturnType: returnType,
				}

				t.functions[className+".static."+method.Name] = FunctionType{
					Parameters: paramTypes,
					ReturnType: returnType,
				}

				t.functions[className+"."+method.Name] = FunctionType{
					Parameters: paramTypes,
					ReturnType: returnType,
				}
			}

		case *ast.TypeDefinition:
			typeName := aliasPrefix + d.Name
			if _, exists := t.types[typeName]; exists {
				return fmt.Errorf("imported type %s is already defined", typeName)
			}
			fields := make(map[string]string)

			for _, field := range d.Fields {
				if _, exists := fields[field.Name]; exists {
					return fmt.Errorf("field %s is already defined in imported type %s", field.Name, typeName)
				}
				fields[field.Name] = t.qualifyType(field.Type, aliasPrefix, localTypesInImport)
			}
			t.types[typeName] = fields
		}
	}
	return nil
}

func (t *TypeChecker) registerHTTPLibrary(className string) {

	t.classes[className] = make(map[string]FunctionType)
	t.types[className] = make(map[string]string)

	httpMethods := []struct {
		name       string
		params     []string
		returnType string
	}{
		{"get", []string{"string"}, "HTTPResponse"},
		{"post", []string{"string", "string"}, "HTTPResponse"},
		{"put", []string{"string", "string"}, "HTTPResponse"},
		{"delete", []string{"string"}, "HTTPResponse"},
		{"setHeaders", []string{"array"}, "void"},
		{"getHeader", []string{"HTTPResponse", "string"}, "string"},
		{"parseJSON", []string{"string"}, "any"},
	}

	classMethods := t.classes[className]

	for _, method := range httpMethods {

		staticKey := "static." + method.name
		methodType := FunctionType{
			Parameters: method.params,
			ReturnType: method.returnType,
		}

		classMethods[staticKey] = methodType
		t.functions[className+".static."+method.name] = methodType

		t.functions[className+"."+method.name] = methodType

		classMethods[method.name] = methodType
	}

	t.types["HTTPResponse"] = map[string]string{
		"statusCode": "int",
		"body":       "string",
		"headers":    "array",
	}
}

func (t *TypeChecker) registerDateLibrary(className string) {

	t.classes[className] = make(map[string]FunctionType)
	t.types[className] = make(map[string]string)

	t.types["Date"] = map[string]string{
		"year":  "int",
		"month": "int",
		"day":   "int",
	}

	t.functions[className+".now"] = FunctionType{
		Parameters: []string{},
		ReturnType: "Date",
	}

	t.functions[className+".formatDate"] = FunctionType{
		Parameters: []string{"Date"},
		ReturnType: "string",
	}

	t.functions[className+".parse"] = FunctionType{
		Parameters: []string{"string"},
		ReturnType: "int",
	}

	t.functions[className+".currentYear"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	t.functions[className+".currentMonth"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	t.functions[className+".currentDay"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	t.functions[className+".isLeapYear"] = FunctionType{
		Parameters: []string{"int"},
		ReturnType: "bool",
	}

	t.functions[className+".daysInMonth"] = FunctionType{
		Parameters: []string{"int", "int"},
		ReturnType: "int",
	}

	t.functions[className+".createDate"] = FunctionType{
		Parameters: []string{"int", "int", "int"},
		ReturnType: "Date",
	}

	t.functions[className+".dayOfWeek"] = FunctionType{
		Parameters: []string{"Date"},
		ReturnType: "int",
	}

	t.functions[className+".addDays"] = FunctionType{
		Parameters: []string{"Date", "int"},
		ReturnType: "Date",
	}

	t.functions[className+".subtractDays"] = FunctionType{
		Parameters: []string{"Date", "int"},
		ReturnType: "Date",
	}

	t.functions[className+".today"] = FunctionType{
		Parameters: []string{},
		ReturnType: "string",
	}

	t.classes[className]["static.now"] = FunctionType{
		Parameters: []string{},
		ReturnType: "Date",
	}

	t.classes[className]["static.formatDate"] = FunctionType{
		Parameters: []string{"Date"},
		ReturnType: "string",
	}

	t.classes[className]["static.parse"] = FunctionType{
		Parameters: []string{"string"},
		ReturnType: "int",
	}

	t.classes[className]["static.currentYear"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	t.classes[className]["static.currentMonth"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	t.classes[className]["static.currentDay"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	t.classes[className]["static.isLeapYear"] = FunctionType{
		Parameters: []string{"int"},
		ReturnType: "bool",
	}

	t.classes[className]["static.daysInMonth"] = FunctionType{
		Parameters: []string{"int", "int"},
		ReturnType: "int",
	}

	t.classes[className]["static.createDate"] = FunctionType{
		Parameters: []string{"int", "int", "int"},
		ReturnType: "Date",
	}

	t.classes[className]["static.dayOfWeek"] = FunctionType{
		Parameters: []string{"Date"},
		ReturnType: "int",
	}

	t.classes[className]["static.addDays"] = FunctionType{
		Parameters: []string{"Date", "int"},
		ReturnType: "Date",
	}

	t.classes[className]["static.subtractDays"] = FunctionType{
		Parameters: []string{"Date", "int"},
		ReturnType: "Date",
	}

	t.classes[className]["static.today"] = FunctionType{
		Parameters: []string{},
		ReturnType: "string",
	}
}

func (t *TypeChecker) registerTimeLibrary(className string) {

	t.classes[className] = make(map[string]FunctionType)
	t.types[className] = make(map[string]string)

	t.functions[className+".sleep"] = FunctionType{
		Parameters: []string{"int"},
		ReturnType: "void",
	}

	t.functions[className+".now"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	timeMethods := t.classes[className]
	timeMethods["static.sleep"] = FunctionType{
		Parameters: []string{"int"},
		ReturnType: "void",
	}
	timeMethods["static.now"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}
}

func (t *TypeChecker) setErrorPos(pos int) {
	t.errorPos = pos
}

func (t *TypeChecker) Position() int {
	return t.errorPos
}

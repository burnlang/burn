package typechecker

import (
	"fmt"
	"os"
	"time"

	"github.com/s42yt/burn/pkg/ast"
	"github.com/s42yt/burn/pkg/lexer"
	"github.com/s42yt/burn/pkg/parser"
)

type TypeChecker struct {
	variables  map[string]string
	functions  map[string]FunctionType
	types      map[string]map[string]string
	currentFn  string
	errorPos   int
	arrayTypes map[string]string
}

type FunctionType struct {
	Parameters []string
	ReturnType string
}

func New() *TypeChecker {
	tc := &TypeChecker{
		variables:  make(map[string]string),
		functions:  make(map[string]FunctionType),
		types:      make(map[string]map[string]string),
		arrayTypes: make(map[string]string),
	}

	tc.functions["print"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "",
	}

	tc.functions["toString"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "string",
	}

	tc.functions["input"] = FunctionType{
		Parameters: []string{"string"},
		ReturnType: "string",
	}

	tc.functions["now"] = FunctionType{
		Parameters: []string{},
		ReturnType: "Date",
	}

	tc.functions["formatDate"] = FunctionType{
		Parameters: []string{"Date"},
		ReturnType: "string",
	}

	tc.functions["createDate"] = FunctionType{
		Parameters: []string{"int", "int", "int"},
		ReturnType: "Date",
	}

	tc.functions["currentYear"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	tc.functions["currentMonth"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	tc.functions["currentDay"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	tc.functions["isLeapYear"] = FunctionType{
		Parameters: []string{"int"},
		ReturnType: "bool",
	}

	tc.functions["daysInMonth"] = FunctionType{
		Parameters: []string{"int", "int"},
		ReturnType: "int",
	}

	tc.functions["dayOfWeek"] = FunctionType{
		Parameters: []string{"Date"},
		ReturnType: "int",
	}

	tc.functions["today"] = FunctionType{
		Parameters: []string{},
		ReturnType: "string",
	}

	tc.functions["addDays"] = FunctionType{
		Parameters: []string{"Date", "int"},
		ReturnType: "Date",
	}

	tc.functions["subtractDays"] = FunctionType{
		Parameters: []string{"Date", "int"},
		ReturnType: "Date",
	}

	tc.types["Date"] = map[string]string{
		"year":  "int",
		"month": "int",
		"day":   "int",
	}

	tc.types["array"] = map[string]string{}

	tc.types["any"] = map[string]string{}

	return tc
}

func (t *TypeChecker) Check(program *ast.Program) error {
	for _, decl := range program.Declarations {
		switch d := decl.(type) {
		case *ast.FunctionDeclaration:
			paramTypes := make([]string, len(d.Parameters))
			for i, param := range d.Parameters {
				paramTypes[i] = param.Type
			}

			t.functions[d.Name] = FunctionType{
				Parameters: paramTypes,
				ReturnType: d.ReturnType,
			}
		case *ast.TypeDefinition:
			if err := t.checkTypeDefinition(d); err != nil {
				return err
			}
		}
	}

	for _, decl := range program.Declarations {
		if err := t.checkDeclaration(decl); err != nil {
			return err
		}
	}

	return nil
}

func (t *TypeChecker) checkTypeDefinition(typeDef *ast.TypeDefinition) error {
	if _, exists := t.types[typeDef.Name]; exists {
		return fmt.Errorf("type %s is already defined", typeDef.Name)
	}

	fields := make(map[string]string)
	for _, field := range typeDef.Fields {
		if _, exists := fields[field.Name]; exists {
			return fmt.Errorf("duplicate field %s in type %s", field.Name, typeDef.Name)
		}
		fields[field.Name] = field.Type
	}

	t.types[typeDef.Name] = fields
	return nil
}

func (t *TypeChecker) checkDeclaration(decl ast.Declaration) error {
	switch d := decl.(type) {
	case *ast.ImportDeclaration:
		return t.checkImport(d)
	case *ast.TypeDefinition:
		return nil
	case *ast.FunctionDeclaration:
		return t.checkFunction(d)
	case *ast.VariableDeclaration:
		return t.checkVariableDeclaration(d)
	case *ast.ExpressionStatement:
		_, err := t.checkExpression(d.Expression)
		return err
	case *ast.ReturnStatement:
		if t.currentFn == "" {
			return fmt.Errorf("return statement outside of function")
		}
		if d.Value == nil {
			if t.functions[t.currentFn].ReturnType != "" {
				return fmt.Errorf("function %s must return a value of type %s",
					t.currentFn, t.functions[t.currentFn].ReturnType)
			}
			return nil
		}
		valueType, err := t.checkExpression(d.Value)
		if err != nil {
			return err
		}
		if valueType != t.functions[t.currentFn].ReturnType {
			return fmt.Errorf("function %s must return %s but got %s",
				t.currentFn, t.functions[t.currentFn].ReturnType, valueType)
		}
		return nil
	case *ast.IfStatement:
		condType, err := t.checkExpression(d.Condition)
		if err != nil {
			return err
		}
		if condType != "bool" {
			return fmt.Errorf("condition must be boolean, got %s", condType)
		}

		for _, stmt := range d.ThenBranch {
			if err := t.checkDeclaration(stmt); err != nil {
				return err
			}
		}

		if d.ElseBranch != nil {
			for _, stmt := range d.ElseBranch {
				if err := t.checkDeclaration(stmt); err != nil {
					return err
				}
			}
		}
		return nil
	case *ast.WhileStatement:
		condType, err := t.checkExpression(d.Condition)
		if err != nil {
			return err
		}
		if condType != "bool" {
			return fmt.Errorf("condition must be boolean, got %s", condType)
		}

		for _, stmt := range d.Body {
			if err := t.checkDeclaration(stmt); err != nil {
				return err
			}
		}
		return nil
	case *ast.ForStatement:
		if d.Initializer != nil {
			if err := t.checkDeclaration(d.Initializer); err != nil {
				return err
			}
		}

		if d.Condition != nil {
			condType, err := t.checkExpression(d.Condition)
			if err != nil {
				return err
			}
			if condType != "bool" {
				return fmt.Errorf("for condition must be boolean, got %s", condType)
			}
		}

		if d.Increment != nil {
			if _, err := t.checkExpression(d.Increment); err != nil {
				return err
			}
		}

		for _, stmt := range d.Body {
			if err := t.checkDeclaration(stmt); err != nil {
				return err
			}
		}
		return nil
	default:
		return fmt.Errorf("unknown declaration type: %T", decl)
	}
}

func (t *TypeChecker) checkImport(imp *ast.ImportDeclaration) error {
	path := imp.Path

	source, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("could not read imported file %s: %v", path, err)
	}

	lexer := lexer.New(string(source))
	tokens, err := lexer.Tokenize()
	if err != nil {
		return err
	}

	parser := parser.New(tokens)
	program, err := parser.Parse()
	if err != nil {
		return err
	}

	importedChecker := New()
	if err := importedChecker.Check(program); err != nil {
		return err
	}

	for name, fn := range importedChecker.functions {
		if name != "main" {
			t.functions[name] = fn
		}
	}

	for typeName, typeFields := range importedChecker.types {
		t.types[typeName] = typeFields
	}

	return nil
}

func (t *TypeChecker) checkFunction(fn *ast.FunctionDeclaration) error {
	prevVariables := make(map[string]string)
	for k, v := range t.variables {
		prevVariables[k] = v
	}
	prevFn := t.currentFn

	t.currentFn = fn.Name
	t.variables = make(map[string]string)

	for _, param := range fn.Parameters {
		t.variables[param.Name] = param.Type
	}

	for _, stmt := range fn.Body {
		if err := t.checkDeclaration(stmt); err != nil {
			return err
		}
	}

	t.variables = prevVariables
	t.currentFn = prevFn

	return nil
}

func (t *TypeChecker) checkVariableDeclaration(decl *ast.VariableDeclaration) error {
	if decl.Value != nil {
		valueType, err := t.checkExpression(decl.Value)
		if err != nil {
			return err
		}

		if valueType == "array" {
			if arrayLiteral, ok := decl.Value.(*ast.ArrayLiteralExpression); ok && len(arrayLiteral.Elements) > 0 {
				elemType, err := t.checkExpression(arrayLiteral.Elements[0])
				if err != nil {
					return err
				}
				t.arrayTypes[decl.Name] = elemType
			}
		}

		if decl.Type != "" && decl.Type != valueType {
			return fmt.Errorf("type mismatch: variable %s is declared as %s but assigned %s",
				decl.Name, decl.Type, valueType)
		}

		if decl.Type == "" {
			decl.Type = valueType
		}
	} else if decl.Type == "" {
		return fmt.Errorf("variable %s must have a type or initializer", decl.Name)
	}

	if _, isBuiltin := t.types[decl.Type]; !isBuiltin &&
		decl.Type != "int" && decl.Type != "float" &&
		decl.Type != "string" && decl.Type != "bool" &&
		decl.Type != "array" && decl.Type != "any" {
		return fmt.Errorf("unknown type: %s", decl.Type)
	}

	t.variables[decl.Name] = decl.Type
	return nil
}

func (t *TypeChecker) checkExpression(expr ast.Expression) (string, error) {
	switch e := expr.(type) {
	case *ast.BinaryExpression:
		return t.checkBinaryExpression(e)
	case *ast.UnaryExpression:
		return t.checkUnary(e)
	case *ast.VariableExpression:
		return t.checkVariable(e)
	case *ast.AssignmentExpression:
		return t.checkAssignment(e)
	case *ast.CallExpression:
		return t.checkCall(e)
	case *ast.StructLiteralExpression:
		return t.checkStructLiteral(e)
	case *ast.GetExpression:
		return t.checkGetExpression(e)
	case *ast.SetExpression:
		objectType, err := t.checkExpression(e.Object)
		if err != nil {
			return "", err
		}

		typeDef, exists := t.types[objectType]
		if !exists {
			return "", fmt.Errorf("cannot set field on non-struct type: %s", objectType)
		}

		fieldType, exists := typeDef[e.Name]
		if !exists {
			return "", fmt.Errorf("unknown field %s in type %s", e.Name, objectType)
		}

		valueType, err := t.checkExpression(e.Value)
		if err != nil {
			return "", err
		}

		if valueType != fieldType {
			return "", fmt.Errorf("cannot assign %s to field %s of type %s",
				valueType, e.Name, fieldType)
		}

		return fieldType, nil
	case *ast.LiteralExpression:
		if e.Type == "number" {
			return "int", nil
		}
		return e.Type, nil
	case *ast.ArrayLiteralExpression:
		if len(e.Elements) == 0 {
			return "array", nil
		}

		firstType, err := t.checkExpression(e.Elements[0])
		if err != nil {
			return "", err
		}

		for i := 1; i < len(e.Elements); i++ {
			elemType, err := t.checkExpression(e.Elements[i])
			if err != nil {
				return "", err
			}

			if elemType != firstType {
				return "", fmt.Errorf("array elements must be of the same type, got %s and %s", firstType, elemType)
			}
		}

		return "array", nil

	case *ast.IndexExpression:
		arrayType, err := t.checkExpression(e.Array)
		if err != nil {
			return "", err
		}

		if arrayType != "array" {
			return "", fmt.Errorf("cannot index into non-array type: %s", arrayType)
		}

		indexType, err := t.checkExpression(e.Index)
		if err != nil {
			return "", err
		}

		if indexType != "int" {
			return "", fmt.Errorf("array index must be an integer, got %s", indexType)
		}

		if varExpr, ok := e.Array.(*ast.VariableExpression); ok {
			if elemType, exists := t.arrayTypes[varExpr.Name]; exists {
				return elemType, nil
			}
		}

		return "int", nil
	default:
		return "", fmt.Errorf("unknown expression type: %T", expr)
	}
}

func (t *TypeChecker) checkBinaryExpression(expr *ast.BinaryExpression) (string, error) {
	leftType, err := t.checkExpression(expr.Left)
	if err != nil {
		return "", err
	}

	rightType, err := t.checkExpression(expr.Right)
	if err != nil {
		return "", err
	}

	switch expr.Operator {
	case "+", "-", "*", "/", "%":
		if leftType == "number" {
			leftType = "int"
		}
		if rightType == "number" {
			rightType = "int"
		}

		if (leftType == "int" || leftType == "float") && (rightType == "int" || rightType == "float") {
			if leftType == "float" || rightType == "float" {
				return "float", nil
			}
			return "int", nil
		}

		if expr.Operator == "+" && leftType == "string" && rightType == "string" {
			return "string", nil
		}
		return "", fmt.Errorf("incompatible types for operator %s: %s and %s",
			expr.Operator, leftType, rightType)
	case "&&", "||":
		if leftType != "bool" || rightType != "bool" {
			return "", fmt.Errorf("operator %s requires boolean operands, got %s and %s",
				expr.Operator, leftType, rightType)
		}
		return "bool", nil
	case "==", "!=", "<", ">", "<=", ">=":
		if (leftType == "int" || leftType == "float") && (rightType == "int" || rightType == "float") {
			return "bool", nil
		}

		if leftType != rightType {
			return "", fmt.Errorf("incompatible types for comparison: %s and %s",
				leftType, rightType)
		}
		return "bool", nil
	default:
		return "", fmt.Errorf("unknown operator: %s", expr.Operator)
	}
}

func (t *TypeChecker) checkUnary(expr *ast.UnaryExpression) (string, error) {
	rightType, err := t.checkExpression(expr.Right)
	if err != nil {
		return "", err
	}

	switch expr.Operator {
	case "-":
		if rightType == "int" || rightType == "float" {
			return rightType, nil
		}
		return "", fmt.Errorf("cannot apply unary - to type %s", rightType)
	case "!":
		if rightType == "bool" {
			return "bool", nil
		}
		return "", fmt.Errorf("cannot apply unary ! to type %s", rightType)
	default:
		return "", fmt.Errorf("unknown unary operator: %s", expr.Operator)
	}
}

func (t *TypeChecker) checkVariable(expr *ast.VariableExpression) (string, error) {
	if varType, exists := t.variables[expr.Name]; exists {
		return varType, nil
	}
	return "", fmt.Errorf("undefined variable: %s", expr.Name)
}

func (t *TypeChecker) checkAssignment(expr *ast.AssignmentExpression) (string, error) {
	valueType, err := t.checkExpression(expr.Value)
	if err != nil {
		return "", err
	}

	if varType, exists := t.variables[expr.Name]; exists {
		if varType != valueType {
			return "", fmt.Errorf("cannot assign %s to variable %s of type %s",
				valueType, expr.Name, varType)
		}
		return varType, nil
	}

	t.variables[expr.Name] = valueType
	return valueType, nil
}

func (t *TypeChecker) checkCall(expr *ast.CallExpression) (string, error) {
	callee, ok := expr.Callee.(*ast.VariableExpression)
	if !ok {
		return "", fmt.Errorf("callee is not a function name")
	}

	fn, exists := t.functions[callee.Name]
	if !exists {
		return "", fmt.Errorf("undefined function: %s", callee.Name)
	}

	if callee.Name == "print" {
		if len(expr.Arguments) < 1 {
			return "", fmt.Errorf("print expects at least 1 argument")
		}
		for _, arg := range expr.Arguments {
			_, err := t.checkExpression(arg)
			if err != nil {
				return "", err
			}
		}
		return "", nil
	}

	if callee.Name == "toString" {
		if len(expr.Arguments) != 1 {
			return "", fmt.Errorf("toString expects 1 argument but got %d", len(expr.Arguments))
		}
		_, err := t.checkExpression(expr.Arguments[0])
		if err != nil {
			return "", err
		}
		return "string", nil
	}

	if len(expr.Arguments) != len(fn.Parameters) {
		return "", fmt.Errorf("function %s expects %d arguments but got %d",
			callee.Name, len(fn.Parameters), len(expr.Arguments))
	}

	for i, arg := range expr.Arguments {
		argType, err := t.checkExpression(arg)
		if err != nil {
			return "", err
		}

		if fn.Parameters[i] != "any" && argType != fn.Parameters[i] {
			return "", fmt.Errorf("argument %d of function %s expects %s but got %s",
				i+1, callee.Name, fn.Parameters[i], argType)
		}
	}

	return fn.ReturnType, nil
}

func (t *TypeChecker) checkStructLiteral(expr *ast.StructLiteralExpression) (string, error) {
	typeDef, exists := t.types[expr.Type]
	if !exists {
		return "", fmt.Errorf("unknown type: %s", expr.Type)
	}

	for fieldName, fieldExpr := range expr.Fields {
		fieldType, exists := typeDef[fieldName]
		if !exists {
			return "", fmt.Errorf("unknown field %s in type %s", fieldName, expr.Type)
		}

		valueType, err := t.checkExpression(fieldExpr)
		if err != nil {
			return "", err
		}

		if valueType != fieldType {
			return "", fmt.Errorf("type mismatch for field %s: expected %s but got %s",
				fieldName, fieldType, valueType)
		}
	}

	return expr.Type, nil
}

func (t *TypeChecker) checkGetExpression(expr *ast.GetExpression) (string, error) {
	objectType, err := t.checkExpression(expr.Object)
	if err != nil {
		return "", err
	}

	typeDef, exists := t.types[objectType]
	if !exists {
		return "", fmt.Errorf("cannot access field on non-struct type: %s", objectType)
	}

	fieldType, exists := typeDef[expr.Name]
	if !exists {
		return "", fmt.Errorf("unknown field %s in type %s", expr.Name, objectType)
	}

	return fieldType, nil
}

func (t *TypeChecker) setErrorPos(pos int) {
	t.errorPos = pos
}

func (t *TypeChecker) Position() int {
	return t.errorPos
}

type DateStruct struct {
	Year  int
	Month int
	Day   int
}

type Struct struct {
	TypeName string
	Fields   map[string]interface{}
}

func timeToDateStruct(t time.Time) *Struct {
	return &Struct{
		TypeName: "Date",
		Fields: map[string]interface{}{
			"year":  t.Year(),
			"month": int(t.Month()),
			"day":   t.Day(),
		},
	}
}

func dateStructToTime(dateStruct *Struct) (time.Time, error) {
	if dateStruct.TypeName != "Date" {
		return time.Time{}, fmt.Errorf("expected Date struct")
	}

	year, ok1 := dateStruct.Fields["year"].(int)
	month, ok2 := dateStruct.Fields["month"].(int)
	day, ok3 := dateStruct.Fields["day"].(int)

	if !ok1 || !ok2 || !ok3 {
		return time.Time{}, fmt.Errorf("invalid Date struct fields")
	}

	return time.Date(year, time.Month(month), day, 0, 0, 0, 0, time.UTC), nil
}

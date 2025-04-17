package typechecker

import (
	"fmt"
	"strings"

	"github.com/burnlang/burn/pkg/ast"
)

func (t *TypeChecker) checkDeclaration(decl ast.Declaration) error {
	switch d := decl.(type) {
	case *ast.VariableDeclaration:
		if d.IsConst {
			return t.checkConstDeclaration(d)
		}
		return t.checkVarDeclaration(d)
	case *ast.FunctionDeclaration:
		return t.checkFunctionDeclaration(d)
	case *ast.ExpressionStatement:
		_, err := t.checkExpression(d.Expression)
		return err
	case *ast.TypeDefinition:
		return t.checkTypeDefinition(d)
	case *ast.ImportDeclaration:

		return nil
	case *ast.MultiImportDeclaration:

		return nil
	case *ast.ClassDeclaration:
		return t.checkClassDeclaration(d)
	case *ast.ReturnStatement:
		return t.checkReturnStatement(d)
	case *ast.IfStatement:
		return t.checkIfStatement(d)
	case *ast.WhileStatement:
		return t.checkWhileStatement(d)
	case *ast.ForStatement:
		return t.checkForStatement(d)
	case *ast.BlockStatement:
		return t.checkBlockStatement(d)
	default:
		return fmt.Errorf("unknown declaration type: %T", decl)
	}
}

func (t *TypeChecker) checkVarDeclaration(decl *ast.VariableDeclaration) error {
	t.setErrorPos(decl.Pos())

	if decl.Value != nil {
		valueType, err := t.checkExpression(decl.Value)
		if err != nil {
			return err
		}

		if decl.Type != "" && valueType != decl.Type {
			return fmt.Errorf("variable type %s does not match initializer type %s", decl.Type, valueType)
		}

		if decl.Type == "" {
			decl.Type = valueType
		}

		if arrayLiteral, ok := decl.Value.(*ast.ArrayLiteralExpression); ok && len(arrayLiteral.Elements) > 0 {
			elemType, err := t.checkExpression(arrayLiteral.Elements[0])
			if err != nil {
				return err
			}
			t.arrayTypes[decl.Name] = elemType
		}
	}

	if decl.Type == "" {
		return fmt.Errorf("variable %s must have a type or an initializer", decl.Name)
	}

	if _, exists := t.variables[decl.Name]; exists {
		return fmt.Errorf("variable %s is already defined", decl.Name)
	}

	t.variables[decl.Name] = decl.Type
	return nil
}

func (t *TypeChecker) checkConstDeclaration(decl *ast.VariableDeclaration) error {
	t.setErrorPos(decl.Pos())

	if decl.Value == nil {
		return fmt.Errorf("constant %s must have an initializer", decl.Name)
	}

	valueType, err := t.checkExpression(decl.Value)
	if err != nil {
		return err
	}

	if decl.Type != "" && valueType != decl.Type {
		return fmt.Errorf("constant type %s does not match initializer type %s", decl.Type, valueType)
	}

	if decl.Type == "" {
		decl.Type = valueType
	}

	if _, exists := t.variables[decl.Name]; exists {
		return fmt.Errorf("constant %s is already defined", decl.Name)
	}

	t.variables[decl.Name] = decl.Type
	return nil
}

func (t *TypeChecker) checkFunctionDeclaration(decl *ast.FunctionDeclaration) error {
	t.setErrorPos(decl.Pos())

	prevVars := make(map[string]string)
	for k, v := range t.variables {
		prevVars[k] = v
	}
	prevFn := t.currentFn

	t.currentFn = decl.Name
	t.variables = make(map[string]string)

	for _, param := range decl.Parameters {
		t.variables[param.Name] = param.Type
	}

	for _, stmt := range decl.Body {
		if err := t.checkDeclaration(stmt); err != nil {
			return fmt.Errorf("in function %s: %w", decl.Name, err)
		}
	}

	if decl.ReturnType != "" && decl.ReturnType != "void" {
		if !t.functionHasValidReturn(decl.Body, decl.ReturnType) {
			return fmt.Errorf("function %s must return a value of type %s", decl.Name, decl.ReturnType)
		}
	}

	t.variables = prevVars
	t.currentFn = prevFn

	return nil
}

func (t *TypeChecker) functionHasValidReturn(body []ast.Declaration, expectedType string) bool {
	for _, stmt := range body {
		if ret, ok := stmt.(*ast.ReturnStatement); ok {
			if ret.Value == nil {
				return false
			}

			valueType, err := t.checkExpression(ret.Value)
			if err != nil || valueType != expectedType {
				return false
			}

			return true
		}

		if block, ok := stmt.(*ast.BlockStatement); ok {
			if t.functionHasValidReturn(block.Statements, expectedType) {
				return true
			}
		}

		if ifStmt, ok := stmt.(*ast.IfStatement); ok {
			if t.functionHasValidReturn(ifStmt.ThenBranch, expectedType) {
				if len(ifStmt.ElseBranch) > 0 {
					return t.functionHasValidReturn(ifStmt.ElseBranch, expectedType)
				}
			}
		}
	}

	return false
}

func (t *TypeChecker) checkTypeDefinition(decl *ast.TypeDefinition) error {
	t.setErrorPos(decl.Pos())

	fields := make(map[string]string)
	for _, field := range decl.Fields {
		if !isBuiltinType(field.Type) && field.Type != decl.Name {
			if _, exists := t.types[field.Type]; !exists {
				return fmt.Errorf("unknown type %s for field %s", field.Type, field.Name)
			}
		}
		fields[field.Name] = field.Type
	}
	t.types[decl.Name] = fields

	return nil
}

func isBuiltinType(typeName string) bool {
	switch typeName {
	case "int", "float", "string", "bool", "void", "any":
		return true
	default:
		return false
	}
}

func (t *TypeChecker) checkClassDeclaration(decl *ast.ClassDeclaration) error {
	t.setErrorPos(decl.Pos())

	if _, exists := t.types[decl.Name]; !exists {
		t.types[decl.Name] = make(map[string]string)
	}

	for _, method := range decl.Methods {
		prevVars := make(map[string]string)
		for k, v := range t.variables {
			prevVars[k] = v
		}
		prevFn := t.currentFn

		t.currentFn = decl.Name + "." + method.Name
		t.variables = make(map[string]string)

		t.variables["this"] = decl.Name

		for _, param := range method.Parameters {
			t.variables[param.Name] = param.Type
		}

		for _, stmt := range method.Body {
			if err := t.checkDeclaration(stmt); err != nil {
				return fmt.Errorf("in method %s.%s: %w", decl.Name, method.Name, err)
			}
		}

		if method.ReturnType != "" && method.ReturnType != "void" {
			if !t.functionHasValidReturn(method.Body, method.ReturnType) {
				return fmt.Errorf("method %s.%s must return a value of type %s",
					decl.Name, method.Name, method.ReturnType)
			}
		}

		t.variables = prevVars
		t.currentFn = prevFn
	}

	for _, method := range decl.StaticMethods {
		prevVars := make(map[string]string)
		for k, v := range t.variables {
			prevVars[k] = v
		}
		prevFn := t.currentFn

		t.currentFn = decl.Name + ".static." + method.Name
		t.variables = make(map[string]string)

		for _, param := range method.Parameters {
			t.variables[param.Name] = param.Type
		}

		for _, stmt := range method.Body {
			if err := t.checkDeclaration(stmt); err != nil {
				return fmt.Errorf("in static method %s.%s: %w", decl.Name, method.Name, err)
			}
		}

		if method.ReturnType != "" && method.ReturnType != "void" {
			if !t.functionHasValidReturn(method.Body, method.ReturnType) {
				return fmt.Errorf("static method %s.%s must return a value of type %s",
					decl.Name, method.Name, method.ReturnType)
			}
		}

		t.variables = prevVars
		t.currentFn = prevFn
	}

	return nil
}

func (t *TypeChecker) checkReturnStatement(stmt *ast.ReturnStatement) error {
	t.setErrorPos(stmt.Pos())

	if t.currentFn == "" {
		return fmt.Errorf("return statement outside of function")
	}

	var expectedType string
	if strings.Contains(t.currentFn, ".") {
		parts := strings.Split(t.currentFn, ".")

		if len(parts) == 3 && parts[1] == "static" {
			className, methodName := parts[0], parts[2]
			if classMethods, exists := t.classes[className]; exists {
				if fn, exists := classMethods["static."+methodName]; exists {
					expectedType = fn.ReturnType
				}
			}
		} else if len(parts) == 2 {

			className, methodName := parts[0], parts[1]
			if classMethods, exists := t.classes[className]; exists {
				if fn, exists := classMethods[methodName]; exists {
					expectedType = fn.ReturnType
				}
			}
		}
	} else {

		if fn, exists := t.functions[t.currentFn]; exists {
			expectedType = fn.ReturnType
		}
	}

	if expectedType == "" {
		return fmt.Errorf("could not determine return type for function %s", t.currentFn)
	}

	if expectedType == "void" {
		if stmt.Value != nil {
			return fmt.Errorf("void function cannot return a value")
		}
		return nil
	}

	if stmt.Value == nil {
		return fmt.Errorf("non-void function must return a value")
	}

	actualType, err := t.checkExpression(stmt.Value)
	if err != nil {
		return err
	}

	if actualType != expectedType {
		return fmt.Errorf("return type %s does not match expected type %s",
			actualType, expectedType)
	}

	return nil
}

func (t *TypeChecker) checkIfStatement(stmt *ast.IfStatement) error {

	condType, err := t.checkExpression(stmt.Condition)
	if err != nil {
		return err
	}

	if condType != "bool" {
		return fmt.Errorf("if condition must be a boolean expression, got %s", condType)
	}

	for _, thenStmt := range stmt.ThenBranch {
		if err := t.checkDeclaration(thenStmt); err != nil {
			return err
		}
	}

	if len(stmt.ElseBranch) > 0 {
		for _, elseStmt := range stmt.ElseBranch {
			if err := t.checkDeclaration(elseStmt); err != nil {
				return err
			}
		}
	}

	return nil
}

func (t *TypeChecker) checkWhileStatement(stmt *ast.WhileStatement) error {

	condType, err := t.checkExpression(stmt.Condition)
	if err != nil {
		return err
	}

	if condType != "bool" {
		return fmt.Errorf("while condition must be a boolean expression, got %s", condType)
	}

	for _, bodyStmt := range stmt.Body {
		if err := t.checkDeclaration(bodyStmt); err != nil {
			return err
		}
	}

	return nil
}

func (t *TypeChecker) checkForStatement(stmt *ast.ForStatement) error {

	prevVars := make(map[string]string)
	for k, v := range t.variables {
		prevVars[k] = v
	}

	if stmt.Initializer != nil {
		if err := t.checkDeclaration(stmt.Initializer); err != nil {
			return err
		}
	}

	if stmt.Condition != nil {
		condType, err := t.checkExpression(stmt.Condition)
		if err != nil {
			return err
		}

		if condType != "bool" {
			return fmt.Errorf("for condition must be a boolean expression, got %s", condType)
		}
	}

	if stmt.Increment != nil {
		_, err := t.checkExpression(stmt.Increment)
		if err != nil {
			return err
		}
	}

	for _, bodyStmt := range stmt.Body {
		if err := t.checkDeclaration(bodyStmt); err != nil {
			return err
		}
	}

	t.variables = prevVars

	return nil
}

func (t *TypeChecker) checkBlockStatement(stmt *ast.BlockStatement) error {

	prevVars := make(map[string]string)
	for k, v := range t.variables {
		prevVars[k] = v
	}

	for _, blockStmt := range stmt.Statements {
		if err := t.checkDeclaration(blockStmt); err != nil {
			return err
		}
	}

	t.variables = prevVars

	return nil
}

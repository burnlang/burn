package typechecker

import (
	"fmt"

	"github.com/burnlang/burn/pkg/ast"
)

func (t *TypeChecker) checkExpression(expr ast.Expression) (string, error) {
	if expr != nil {
		t.setErrorPos(expr.Pos())
	}

	switch e := expr.(type) {
	case *ast.BinaryExpression:
		return t.checkBinaryExpression(e)
	case *ast.UnaryExpression:
		return t.checkUnaryExpression(e)
	case *ast.VariableExpression:
		return t.checkVariableExpression(e)
	case *ast.AssignmentExpression:
		return t.checkAssignmentExpression(e)
	case *ast.CallExpression:
		return t.checkCallExpression(e)
	case *ast.StructLiteralExpression:
		return t.checkStructLiteralExpression(e)
	case *ast.GetExpression:
		return t.checkGetExpression(e)
	case *ast.SetExpression:
		return t.checkSetExpression(e)
	case *ast.LiteralExpression:
		return t.checkLiteralExpression(e)
	case *ast.ArrayLiteralExpression:
		return t.checkArrayLiteralExpression(e)
	case *ast.IndexExpression:
		return t.checkIndexExpression(e)
	case *ast.ClassMethodCallExpression:
		return t.checkClassMethodCallExpression(e)
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
		return t.checkArithmeticOperation(expr.Operator, leftType, rightType)
	case "&&", "||":
		return t.checkLogicalOperation(expr.Operator, leftType, rightType)
	case "==", "!=", "<", ">", "<=", ">=":
		return t.checkComparisonOperation(expr.Operator, leftType, rightType)
	default:
		return "", fmt.Errorf("unknown operator: %s", expr.Operator)
	}
}

func (t *TypeChecker) checkArithmeticOperation(operator string, leftType, rightType string) (string, error) {

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

	if operator == "+" && leftType == "string" && rightType == "string" {
		return "string", nil
	}

	return "", fmt.Errorf("incompatible types for operator %s: %s and %s",
		operator, leftType, rightType)
}

func (t *TypeChecker) checkLogicalOperation(operator string, leftType, rightType string) (string, error) {
	if leftType != "bool" || rightType != "bool" {
		return "", fmt.Errorf("operator %s requires boolean operands, got %s and %s",
			operator, leftType, rightType)
	}
	return "bool", nil
}

func (t *TypeChecker) checkComparisonOperation(operator string, leftType, rightType string) (string, error) {

	if (leftType == "int" || leftType == "float") && (rightType == "int" || rightType == "float") {
		return "bool", nil
	}

	if leftType != rightType {
		return "", fmt.Errorf("incompatible types for comparison: %s and %s",
			leftType, rightType)
	}
	return "bool", nil
}

func (t *TypeChecker) checkUnaryExpression(expr *ast.UnaryExpression) (string, error) {
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

func (t *TypeChecker) checkVariableExpression(expr *ast.VariableExpression) (string, error) {
	t.setErrorPos(expr.Pos())

	if varType, exists := t.variables[expr.Name]; exists {
		return varType, nil
	}
	return "", fmt.Errorf("undefined variable: %s", expr.Name)
}

func (t *TypeChecker) checkAssignmentExpression(expr *ast.AssignmentExpression) (string, error) {
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

func (t *TypeChecker) checkCallExpression(expr *ast.CallExpression) (string, error) {

	if getExpr, ok := expr.Callee.(*ast.GetExpression); ok {
		if classNameExpr, ok := getExpr.Object.(*ast.VariableExpression); ok {
			className := classNameExpr.Name
			methodName := getExpr.Name

			classMethodCall := &ast.ClassMethodCallExpression{
				ClassName:  className,
				MethodName: methodName,
				Arguments:  expr.Arguments,
				IsStatic:   false,
				Position:   expr.Position,
			}

			return t.checkClassMethodCallExpression(classMethodCall)
		}
	}

	if getExpr, ok := expr.Callee.(*ast.GetExpression); ok {

		if classExpr, ok := getExpr.Object.(*ast.VariableExpression); ok {
			className := classExpr.Name
			methodName := getExpr.Name

			if _, exists := t.classes[className]; exists {

				classMethodCall := &ast.ClassMethodCallExpression{
					ClassName:  className,
					MethodName: methodName,
					Arguments:  expr.Arguments,
					IsStatic:   true,
					Position:   expr.Position,
				}

				return t.checkClassMethodCallExpression(classMethodCall)
			}
		}
	}

	callee, ok := expr.Callee.(*ast.VariableExpression)
	if !ok {
		return "", fmt.Errorf("callee is not a function name")
	}

	fn, exists := t.functions[callee.Name]
	if !exists {
		return "", fmt.Errorf("undefined function: %s", callee.Name)
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

		expectedType := fn.Parameters[i]
		if expectedType != "any" && argType != expectedType {
			return "", fmt.Errorf("argument %d of function %s expects %s but got %s",
				i+1, callee.Name, expectedType, argType)
		}
	}

	return fn.ReturnType, nil
}

func (t *TypeChecker) checkStructLiteralExpression(expr *ast.StructLiteralExpression) (string, error) {
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

func (t *TypeChecker) checkSetExpression(expr *ast.SetExpression) (string, error) {
	objectType, err := t.checkExpression(expr.Object)
	if err != nil {
		return "", err
	}

	typeDef, exists := t.types[objectType]
	if !exists {
		return "", fmt.Errorf("cannot set field on non-struct type: %s", objectType)
	}

	fieldType, exists := typeDef[expr.Name]
	if !exists {
		return "", fmt.Errorf("unknown field %s in type %s", expr.Name, objectType)
	}

	valueType, err := t.checkExpression(expr.Value)
	if err != nil {
		return "", err
	}

	if valueType != fieldType {
		return "", fmt.Errorf("cannot assign %s to field %s of type %s",
			valueType, expr.Name, fieldType)
	}

	return fieldType, nil
}

func (t *TypeChecker) checkLiteralExpression(expr *ast.LiteralExpression) (string, error) {

	if expr.Type == "number" {
		return "int", nil
	}
	return expr.Type, nil
}

func (t *TypeChecker) checkArrayLiteralExpression(expr *ast.ArrayLiteralExpression) (string, error) {
	if len(expr.Elements) == 0 {
		return "array", nil
	}

	firstType, err := t.checkExpression(expr.Elements[0])
	if err != nil {
		return "", err
	}

	for i := 1; i < len(expr.Elements); i++ {
		elemType, err := t.checkExpression(expr.Elements[i])
		if err != nil {
			return "", err
		}

		if elemType != firstType {
			return "", fmt.Errorf("array elements must be of the same type, got %s and %s",
				firstType, elemType)
		}
	}

	return "array", nil
}

func (t *TypeChecker) checkIndexExpression(expr *ast.IndexExpression) (string, error) {
	arrayType, err := t.checkExpression(expr.Array)
	if err != nil {
		return "", err
	}

	if arrayType != "array" {
		return "", fmt.Errorf("cannot index into non-array type: %s", arrayType)
	}

	indexType, err := t.checkExpression(expr.Index)
	if err != nil {
		return "", err
	}

	if indexType != "int" {
		return "", fmt.Errorf("array index must be an integer, got %s", indexType)
	}

	if varExpr, ok := expr.Array.(*ast.VariableExpression); ok {
		if elemType, exists := t.arrayTypes[varExpr.Name]; exists {
			return elemType, nil
		}
	}

	return "int", nil
}

func (t *TypeChecker) checkClassMethodCallExpression(expr *ast.ClassMethodCallExpression) (string, error) {
	className := expr.ClassName
	methodName := expr.MethodName
	isStatic := expr.IsStatic

	classMethods, exists := t.classes[className]
	if !exists {
		return "", fmt.Errorf("undefined class: %s", className)
	}

	methodKey := methodName
	if isStatic {
		methodKey = "static." + methodName
	}

	method, exists := classMethods[methodKey]
	if !exists {
		if isStatic {
			return "", fmt.Errorf("undefined static method %s.%s", className, methodName)
		} else {

			methodKey = "static." + methodName
			method, exists = classMethods[methodKey]
			if !exists {
				return "", fmt.Errorf("undefined method %s.%s", className, methodName)
			}

			return "", fmt.Errorf("static method %s.%s cannot be called on instance", className, methodName)
		}
	}

	if len(expr.Arguments) != len(method.Parameters) {
		return "", fmt.Errorf("method %s.%s expects %d arguments but got %d",
			className, methodName, len(method.Parameters), len(expr.Arguments))
	}

	for i, arg := range expr.Arguments {
		argType, err := t.checkExpression(arg)
		if err != nil {
			return "", err
		}

		expectedType := method.Parameters[i]
		if expectedType != "any" && argType != expectedType {
			return "", fmt.Errorf("argument %d of method %s.%s expects %s but got %s",
				i+1, className, methodName, expectedType, argType)
		}
	}

	return method.ReturnType, nil
}

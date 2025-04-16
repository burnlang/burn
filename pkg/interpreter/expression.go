package interpreter

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/burnlang/burn/pkg/ast"
)

func (i *Interpreter) evaluateExpression(expr ast.Expression) (Value, error) {
	if expr != nil {
		i.setErrorPos(expr.Pos())
	}

	switch e := expr.(type) {
	case *ast.BinaryExpression:
		return i.evaluateBinary(e)
	case *ast.UnaryExpression:
		return i.evaluateUnary(e)
	case *ast.VariableExpression:
		if value, exists := i.environment[e.Name]; exists {
			return value, nil
		}
		return nil, fmt.Errorf("undefined variable: %s", e.Name)
	case *ast.AssignmentExpression:
		value, err := i.evaluateExpression(e.Value)
		if err != nil {
			return nil, err
		}
		i.environment[e.Name] = value
		return value, nil
	case *ast.CallExpression:
		return i.evaluateCall(e)
	case *ast.GetExpression:
		object, err := i.evaluateExpression(e.Object)
		if err != nil {
			return nil, err
		}

		if structObj, ok := object.(*Struct); ok {
			if value, exists := structObj.Fields[e.Name]; exists {
				if intVal, ok := value.(int); ok {
					return float64(intVal), nil
				}
				return value, nil
			}
			return nil, fmt.Errorf("undefined field '%s' on struct of type '%s'",
				e.Name, structObj.TypeName)
		}

		if obj, ok := object.(map[string]interface{}); ok {
			if value, exists := obj[e.Name]; exists {
				if intVal, ok := value.(int); ok {
					return float64(intVal), nil
				}
				return value, nil
			}
			return nil, fmt.Errorf("undefined field: %s", e.Name)
		}

		return nil, fmt.Errorf("cannot access field on non-struct value")
	case *ast.SetExpression:
		object, err := i.evaluateExpression(e.Object)
		if err != nil {
			return nil, err
		}
		value, err := i.evaluateExpression(e.Value)
		if err != nil {
			return nil, err
		}
		if structObj, ok := object.(*Struct); ok {
			structObj.Fields[e.Name] = value
			return value, nil
		}
		if obj, ok := object.(map[string]interface{}); ok {
			obj[e.Name] = value
			return value, nil
		}
		return nil, fmt.Errorf("cannot set field on non-struct value")
	case *ast.LiteralExpression:
		return i.evaluateLiteral(e)
	case *ast.StructLiteralExpression:
		fields := make(map[string]interface{})
		for name, value := range e.Fields {
			evaluated, err := i.evaluateExpression(value)
			if err != nil {
				return nil, err
			}
			fields[name] = evaluated
		}
		return &Struct{
			TypeName: e.Type,
			Fields:   fields,
		}, nil
	case *ast.ArrayLiteralExpression:
		elements := make([]Value, 0, len(e.Elements))
		for _, element := range e.Elements {
			value, err := i.evaluateExpression(element)
			if err != nil {
				return nil, err
			}
			elements = append(elements, value)
		}
		return elements, nil
	case *ast.IndexExpression:
		array, err := i.evaluateExpression(e.Array)
		if err != nil {
			return nil, err
		}

		index, err := i.evaluateExpression(e.Index)
		if err != nil {
			return nil, err
		}

		indexInt, ok := index.(float64)
		if !ok {
			return nil, fmt.Errorf("array index must be a number")
		}

		arrayValue, ok := array.([]Value)
		if !ok {
			return nil, fmt.Errorf("cannot index into non-array value")
		}

		idx := int(indexInt)
		if idx < 0 || idx >= len(arrayValue) {
			return nil, fmt.Errorf("array index out of bounds: %d", idx)
		}

		return arrayValue[idx], nil
	default:
		return nil, fmt.Errorf("unknown expression type: %T", expr)
	}
}

func (i *Interpreter) evaluateBinary(expr *ast.BinaryExpression) (Value, error) {
	left, err := i.evaluateExpression(expr.Left)
	if err != nil {
		return nil, err
	}

	right, err := i.evaluateExpression(expr.Right)
	if err != nil {
		return nil, err
	}

	switch expr.Operator {
	case "+", "-", "*", "/", "<", ">", "<=", ">=", "==", "!=":
		if lInt, lok := left.(int); lok {
			left = float64(lInt)
		}
		if rInt, rok := right.(int); rok {
			right = float64(rInt)
		}
	}

	switch expr.Operator {
	case "&&":
		if lBool, lok := left.(bool); lok {
			if rBool, rok := right.(bool); rok {
				return lBool && rBool, nil
			}
		}
		return nil, fmt.Errorf("cannot perform logical AND on non-boolean values")
	case "||":
		if lBool, lok := left.(bool); lok {
			if rBool, rok := right.(bool); rok {
				return lBool || rBool, nil
			}
		}
		return nil, fmt.Errorf("cannot perform logical OR on non-boolean values")
	case "+":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum + rNum, nil
			}
		}
		if lStr, lOk := left.(string); lOk {
			if rStr, rOk := right.(string); rOk {
				return lStr + rStr, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case "-":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum - rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case "*":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum * rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case "/":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				if rNum == 0 {
					return nil, fmt.Errorf("division by zero")
				}
				return lNum / rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case "%":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				if rNum == 0 {
					return nil, fmt.Errorf("modulo by zero")
				}
				return float64(int(lNum) % int(rNum)), nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case "==":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum == rNum, nil
			}
		}
		if lStr, lOk := left.(string); lOk {
			if rStr, rOk := right.(string); rOk {
				return lStr == rStr, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case "!=":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum != rNum, nil
			}
		}
		if lStr, lOk := left.(string); lOk {
			if rStr, rOk := right.(string); rOk {
				return lStr != rStr, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case "<":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum < rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case ">":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum > rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case "<=":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum <= rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	case ">=":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum >= rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
	}

	return nil, fmt.Errorf("invalid operator %s for types %T and %T", expr.Operator, left, right)
}

func (i *Interpreter) evaluateUnary(expr *ast.UnaryExpression) (Value, error) {
	right, err := i.evaluateExpression(expr.Right)
	if err != nil {
		return nil, err
	}

	switch expr.Operator {
	case "-":
		if num, ok := right.(float64); ok {
			return -num, nil
		}
	case "!":
		if b, ok := right.(bool); ok {
			return !b, nil
		}
	}

	return nil, fmt.Errorf("invalid unary operator %s for type", expr.Operator)
}

func (i *Interpreter) evaluateCall(expr *ast.CallExpression) (Value, error) {
	if getExpr, ok := expr.Callee.(*ast.GetExpression); ok {
		if classNameExpr, ok := getExpr.Object.(*ast.VariableExpression); ok {
			className := classNameExpr.Name
			methodName := getExpr.Name

			class, exists := i.classes[className]
			if !exists {
				return nil, fmt.Errorf("undefined class: %s", className)
			}

			args := make([]Value, 0, len(expr.Arguments))
			for _, arg := range expr.Arguments {
				value, err := i.evaluateExpression(arg)
				if err != nil {
					return nil, err
				}
				args = append(args, value)
			}

			if static, exists := class.Statics[methodName]; exists {
				result, err := i.executeFunction(static, args)
				if err != nil {
					return nil, err
				}

				
				if methodName == "create" {
					if mapResult, ok := result.(map[string]interface{}); ok {
						
						return &Struct{
							TypeName: className,
							Fields:   mapResult,
						}, nil
					}
				}
				return result, nil
			}

			if instanceMethod, exists := class.Methods[methodName]; exists {
				result, err := i.executeFunction(instanceMethod, args)
				if err != nil {
					return nil, err
				}

				
				if methodName == "create" {
					if mapResult, ok := result.(map[string]interface{}); ok {
						
						return &Struct{
							TypeName: className,
							Fields:   mapResult,
						}, nil
					}
				}
				return result, nil
			}

			builtinFuncName := fmt.Sprintf("%s.%s", className, methodName)
			if builtinFunc, exists := i.environment[builtinFuncName]; exists {
				if bf, ok := builtinFunc.(*BuiltinFunction); ok {
					result, err := bf.Call(args)
					if err != nil {
						return nil, err
					}

					
					if methodName == "create" {
						if mapResult, ok := result.(map[string]interface{}); ok {
							
							return &Struct{
								TypeName: className,
								Fields:   mapResult,
							}, nil
						}
					}
					return result, nil
				}
			}

			return nil, fmt.Errorf("undefined static method '%s' in class '%s'", methodName, className)
		}

		object, err := i.evaluateExpression(getExpr.Object)
		if err != nil {
			return nil, err
		}

		if structObj, ok := object.(*Struct); ok {
			methodName := getExpr.Name

			args := make([]Value, len(expr.Arguments))
			for j, arg := range expr.Arguments {
				val, err := i.evaluateExpression(arg)
				if err != nil {
					return nil, err
				}
				args[j] = val
			}

			if class, exists := i.classes[structObj.TypeName]; exists {
				allArgs := make([]Value, len(args)+1)
				allArgs[0] = structObj
				copy(allArgs[1:], args)

				if method, exists := class.Methods[methodName]; exists {
					return i.executeFunction(method, allArgs)
				}
			}

			return nil, fmt.Errorf("undefined method '%s' on type '%s'", methodName, structObj.TypeName)
		}

		return nil, fmt.Errorf("cannot call method on expression of type %T", object)
	}

	callee, ok := expr.Callee.(*ast.VariableExpression)
	if !ok {
		return nil, fmt.Errorf("callee is not a function name")
	}

	args := make([]Value, 0, len(expr.Arguments))
	for _, arg := range expr.Arguments {
		value, err := i.evaluateExpression(arg)
		if err != nil {
			return nil, err
		}
		args = append(args, value)
	}

	if builtinFunc, exists := i.environment[callee.Name]; exists {
		if bf, ok := builtinFunc.(*BuiltinFunction); ok {
			return bf.Call(args)
		}
	}

	fn, exists := i.functions[callee.Name]
	if !exists {
		return nil, fmt.Errorf("undefined function: %s", callee.Name)
	}

	return i.executeFunction(fn, args)
}

func (i *Interpreter) evaluateLiteral(expr *ast.LiteralExpression) (Value, error) {
	switch expr.Type {
	case "number":
		if strings.Contains(expr.Value.(string), ".") {
			if val, err := strconv.ParseFloat(expr.Value.(string), 64); err == nil {
				return val, nil
			} else {
				return nil, fmt.Errorf("invalid float: %s", expr.Value)
			}
		} else {
			if val, err := strconv.ParseFloat(expr.Value.(string), 64); err == nil {
				return val, nil
			} else {
				return nil, fmt.Errorf("invalid number: %s", expr.Value)
			}
		}
	case "string":
		return expr.Value, nil
	case "bool":
		if expr.Value == "true" {
			return true, nil
		} else if expr.Value == "false" {
			return false, nil
		}
		return nil, fmt.Errorf("invalid boolean: %s", expr.Value)
	default:
		return nil, fmt.Errorf("unknown literal type: %s", expr.Type)
	}
}

func (i *Interpreter) evaluateClassMethodCall(expr *ast.ClassMethodCallExpression) (Value, error) {
	className := expr.ClassName
	methodName := expr.MethodName

	class, exists := i.classes[className]
	if !exists {
		return nil, fmt.Errorf("undefined class: %s", className)
	}

	args := make([]Value, len(expr.Arguments))
	for j, arg := range expr.Arguments {
		val, err := i.evaluateExpression(arg)
		if err != nil {
			return nil, err
		}
		args[j] = val
	}

	if method, exists := class.Methods[methodName]; exists {
		return i.executeFunction(method, args)
	}

	if static, exists := class.Statics[methodName]; exists {
		return i.executeFunction(static, args)
	}

	return class.Call(methodName, i, args)
}

func (i *Interpreter) evalBinaryExpression(expr *ast.BinaryExpression) (interface{}, error) {
	left, err := i.evaluateExpression(expr.Left)
	if err != nil {
		return nil, err
	}

	right, err := i.evaluateExpression(expr.Right)
	if err != nil {
		return nil, err
	}

	switch expr.Operator {
	case "+":
		if lInt, lok := left.(int); lok {
			if rInt, rok := right.(int); rok {
				return lInt + rInt, nil
			}
		}
		if lFloat, lok := left.(float64); lok {
			if rFloat, rok := right.(float64); rok {
				return lFloat + rFloat, nil
			}
		}
		if lStr, lok := left.(string); lok {
			if rStr, rok := right.(string); rok {
				return lStr + rStr, nil
			}
		}
		return nil, fmt.Errorf("cannot add values of types %T and %T", left, right)
	}

	return nil, fmt.Errorf("unsupported operator: %s", expr.Operator)
}

package interpreter

import (
	"fmt"
	"strconv"

	"github.com/s42yt/burn/pkg/ast"
)

type Value interface{}

type BuiltinFunction struct {
	Name string
	Fn   func(args []Value) (Value, error)
}

func (bf *BuiltinFunction) Call(args []Value) (Value, error) {
	return bf.Fn(args)
}

type Interpreter struct {
	environment map[string]Value
	functions   map[string]*ast.FunctionDeclaration
	errorPos    int
}

func New() *Interpreter {
	i := &Interpreter{
		environment: make(map[string]Value),
		functions:   make(map[string]*ast.FunctionDeclaration),
		errorPos:    0,
	}
	i.addBuiltins()
	return i
}

func (i *Interpreter) Interpret(program *ast.Program) (Value, error) {
	for _, decl := range program.Declarations {
		if fn, ok := decl.(*ast.FunctionDeclaration); ok {
			i.functions[fn.Name] = fn
		}
	}

	i.addBuiltins()

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

func (i *Interpreter) addBuiltins() {
	i.functions["print"] = &ast.FunctionDeclaration{
		Name: "print",
		Parameters: []ast.Parameter{
			{Name: "value", Type: "any"},
		},
	}

	i.environment["print"] = &BuiltinFunction{
		Name: "print",
		Fn: func(args []Value) (Value, error) {
			if len(args) < 1 {
				return nil, fmt.Errorf("print requires at least one argument")
			}
			for _, arg := range args {
				fmt.Print(arg)
			}
			fmt.Println()
			return nil, nil
		},
	}

	i.functions["toString"] = &ast.FunctionDeclaration{
		Name: "toString",
		Parameters: []ast.Parameter{
			{Name: "value", Type: "any"},
		},
		ReturnType: "string",
	}

	i.environment["toString"] = &BuiltinFunction{
		Name: "toString",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("toString expects exactly one argument")
			}
			return fmt.Sprintf("%v", args[0]), nil
		},
	}

	i.functions["input"] = &ast.FunctionDeclaration{
		Name: "input",
		Parameters: []ast.Parameter{
			{Name: "prompt", Type: "string"},
		},
		ReturnType: "string",
	}

	i.environment["input"] = &BuiltinFunction{
		Name: "input",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("input requires exactly one string argument")
			}

			fmt.Print(args[0])

			var input string
			_, err := fmt.Scanln(&input)
			if err != nil {
				return "", fmt.Errorf("error reading input: %v", err)
			}

			return input, nil
		},
	}
}

func (i *Interpreter) executeDeclaration(decl ast.Declaration) (Value, error) {
	switch d := decl.(type) {
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

func (i *Interpreter) executeBuiltin(name string, args []Value) (Value, error) {
	switch name {
	case "print":
		if len(args) > 0 {
			fmt.Println(args[0])
		}
		return nil, nil

	case "toString":
		if len(args) != 1 {
			return nil, fmt.Errorf("toString expects exactly one argument")
		}
		return fmt.Sprintf("%v", args[0]), nil

	case "input":
		if len(args) != 1 {
			return nil, fmt.Errorf("input requires exactly one string argument")
		}
		fmt.Print(args[0])

		var input string
		_, err := fmt.Scanln(&input)
		if err != nil {
			return "", fmt.Errorf("error reading input: %v", err)
		}
		return input, nil

	default:
		return nil, fmt.Errorf("unknown built-in function: %s", name)
	}
}

func (i *Interpreter) evaluateExpression(expr ast.Expression) (Value, error) {
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
		if obj, ok := object.(map[string]Value); ok {
			if value, exists := obj[e.Name]; exists {
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
		if obj, ok := object.(map[string]Value); ok {
			obj[e.Name] = value
			return value, nil
		}
		return nil, fmt.Errorf("cannot set field on non-struct value")
	case *ast.LiteralExpression:
		return i.evaluateLiteral(e)
	case *ast.StructLiteralExpression:
		fields := make(map[string]Value)
		for name, value := range e.Fields {
			evaluated, err := i.evaluateExpression(value)
			if err != nil {
				return nil, err
			}
			fields[name] = evaluated
		}
		return fields, nil
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
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
	case "-":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum - rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
	case "*":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum * rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
	case "/":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum / rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
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
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
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
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
	case "<":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum < rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
	case ">":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum > rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
	case "<=":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum <= rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
	case ">=":
		if lNum, lOk := left.(float64); lOk {
			if rNum, rOk := right.(float64); rOk {
				return lNum >= rNum, nil
			}
		}
		return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
	}

	return nil, fmt.Errorf("invalid operator %s for types", expr.Operator)
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

	switch callee.Name {
	case "print", "toString", "input":
		return i.executeBuiltin(callee.Name, args)
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
		if val, err := strconv.ParseFloat(expr.Value.(string), 64); err == nil {
			return val, nil
		} else {
			return nil, fmt.Errorf("invalid number: %s", expr.Value)
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

func (i *Interpreter) setErrorPos(pos int) {
	i.errorPos = pos
}

func (i *Interpreter) Position() int {
	return i.errorPos
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

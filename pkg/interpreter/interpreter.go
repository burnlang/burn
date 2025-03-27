package interpreter

import (
	"fmt"
	"strconv"
	"strings"
	"time"

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
		if imp, ok := decl.(*ast.ImportDeclaration); ok {
			if err := i.handleImport(imp); err != nil {
				return nil, err
			}
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

func (i *Interpreter) handleImport(imp *ast.ImportDeclaration) error {
	if strings.Contains(imp.Path, "src/std/date.bn") || imp.Path == "date" {
		i.registerDateLibrary()
		return nil
	}
	
	
	return nil
}

func (i *Interpreter) registerDateLibrary() {
	i.functions["now"] = &ast.FunctionDeclaration{
		Name:       "now",
		Parameters: []ast.Parameter{},
		ReturnType: "Date",
	}

	i.environment["now"] = &BuiltinFunction{
		Name: "now",
		Fn: func(args []Value) (Value, error) {
			currentTime := time.Now()

			dateStruct := &Struct{
				TypeName: "Date",
				Fields: map[string]interface{}{
					"year":  currentTime.Year(),
					"month": int(currentTime.Month()),
					"day":   currentTime.Day(),
				},
			}

			return dateStruct, nil
		},
	}

	i.functions["formatDate"] = &ast.FunctionDeclaration{
		Name: "formatDate",
		Parameters: []ast.Parameter{
			{Name: "date", Type: "Date"},
		},
		ReturnType: "string",
	}

	i.environment["formatDate"] = &BuiltinFunction{
		Name: "formatDate",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("formatDate expects exactly one Date argument")
			}

			dateStruct, ok := args[0].(*Struct)
			if !ok || dateStruct.TypeName != "Date" {
				return nil, fmt.Errorf("formatDate expects a Date struct")
			}

			year, _ := dateStruct.Fields["year"].(int)
			month, _ := dateStruct.Fields["month"].(int)
			day, _ := dateStruct.Fields["day"].(int)

			monthStr := fmt.Sprintf("%02d", month)
			dayStr := fmt.Sprintf("%02d", day)

			return fmt.Sprintf("%d-%s-%s", year, monthStr, dayStr), nil
		},
	}

	i.functions["currentYear"] = &ast.FunctionDeclaration{
		Name:       "currentYear",
		Parameters: []ast.Parameter{},
		ReturnType: "int",
	}

	i.environment["currentYear"] = &BuiltinFunction{
		Name: "currentYear",
		Fn: func(args []Value) (Value, error) {
			return float64(time.Now().Year()), nil
		},
	}

	i.functions["currentMonth"] = &ast.FunctionDeclaration{
		Name:       "currentMonth",
		Parameters: []ast.Parameter{},
		ReturnType: "int",
	}

	i.environment["currentMonth"] = &BuiltinFunction{
		Name: "currentMonth",
		Fn: func(args []Value) (Value, error) {
			return float64(int(time.Now().Month())), nil
		},
	}

	i.functions["currentDay"] = &ast.FunctionDeclaration{
		Name:       "currentDay",
		Parameters: []ast.Parameter{},
		ReturnType: "int",
	}

	i.environment["currentDay"] = &BuiltinFunction{
		Name: "currentDay",
		Fn: func(args []Value) (Value, error) {
			return float64(time.Now().Day()), nil
		},
	}

	i.functions["isLeapYear"] = &ast.FunctionDeclaration{
		Name: "isLeapYear",
		Parameters: []ast.Parameter{
			{Name: "year", Type: "int"},
		},
		ReturnType: "bool",
	}

	i.environment["isLeapYear"] = &BuiltinFunction{
		Name: "isLeapYear",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("isLeapYear expects exactly one integer argument")
			}

			yearFloat, ok := args[0].(float64)
			if !ok {
				return nil, fmt.Errorf("isLeapYear expects an integer")
			}

			year := int(yearFloat)

			isLeap := false
			if year%400 == 0 {
				isLeap = true
			} else if year%100 == 0 {
				isLeap = false
			} else if year%4 == 0 {
				isLeap = true
			}

			return isLeap, nil
		},
	}

	i.functions["daysInMonth"] = &ast.FunctionDeclaration{
		Name: "daysInMonth",
		Parameters: []ast.Parameter{
			{Name: "year", Type: "int"},
			{Name: "month", Type: "int"},
		},
		ReturnType: "int",
	}

	i.environment["daysInMonth"] = &BuiltinFunction{
		Name: "daysInMonth",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 2 {
				return nil, fmt.Errorf("daysInMonth expects exactly two integer arguments")
			}

			yearFloat, ok := args[0].(float64)
			if !ok {
				return nil, fmt.Errorf("daysInMonth expects year as an integer")
			}

			monthFloat, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("daysInMonth expects month as an integer")
			}

			year := int(yearFloat)
			month := int(monthFloat)

			daysInMonth := 31
			if month == 4 || month == 6 || month == 9 || month == 11 {
				daysInMonth = 30
			} else if month == 2 {
				isLeap := false
				if year%400 == 0 {
					isLeap = true
				} else if year%100 == 0 {
					isLeap = false
				} else if year%4 == 0 {
					isLeap = true
				}

				if isLeap {
					daysInMonth = 29
				} else {
					daysInMonth = 28
				}
			}

			return float64(daysInMonth), nil
		},
	}

	i.functions["createDate"] = &ast.FunctionDeclaration{
		Name: "createDate",
		Parameters: []ast.Parameter{
			{Name: "year", Type: "int"},
			{Name: "month", Type: "int"},
			{Name: "day", Type: "int"},
		},
		ReturnType: "Date",
	}

	i.environment["createDate"] = &BuiltinFunction{
		Name: "createDate",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 3 {
				return nil, fmt.Errorf("createDate expects exactly three integer arguments")
			}

			yearFloat, ok := args[0].(float64)
			if !ok {
				return nil, fmt.Errorf("createDate expects year as an integer")
			}

			monthFloat, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("createDate expects month as an integer")
			}

			dayFloat, ok := args[2].(float64)
			if !ok {
				return nil, fmt.Errorf("createDate expects day as an integer")
			}

			dateStruct := &Struct{
				TypeName: "Date",
				Fields: map[string]interface{}{
					"year":  int(yearFloat),
					"month": int(monthFloat),
					"day":   int(dayFloat),
				},
			}

			return dateStruct, nil
		},
	}

	i.functions["dayOfWeek"] = &ast.FunctionDeclaration{
		Name: "dayOfWeek",
		Parameters: []ast.Parameter{
			{Name: "date", Type: "Date"},
		},
		ReturnType: "int",
	}

	i.environment["dayOfWeek"] = &BuiltinFunction{
		Name: "dayOfWeek",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("dayOfWeek expects exactly one Date argument")
			}

			dateStruct, ok := args[0].(*Struct)
			if !ok || dateStruct.TypeName != "Date" {
				return nil, fmt.Errorf("dayOfWeek expects a Date struct")
			}

			year, _ := dateStruct.Fields["year"].(int)
			month, _ := dateStruct.Fields["month"].(int)
			day, _ := dateStruct.Fields["day"].(int)

			if month < 3 {
				month += 12
				year--
			}

			k := year % 100
			j := year / 100

			h := (day + ((13 * (month + 1)) / 5) + k + (k / 4) + (j / 4) - (2 * j)) % 7

			if h < 0 {
				h += 7
			}

			return float64(h), nil
		},
	}

	i.functions["addDays"] = &ast.FunctionDeclaration{
		Name: "addDays",
		Parameters: []ast.Parameter{
			{Name: "date", Type: "Date"},
			{Name: "days", Type: "int"},
		},
		ReturnType: "Date",
	}

	i.environment["addDays"] = &BuiltinFunction{
		Name: "addDays",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 2 {
				return nil, fmt.Errorf("addDays expects exactly two arguments: a Date and an integer")
			}

			dateStruct, ok := args[0].(*Struct)
			if !ok || dateStruct.TypeName != "Date" {
				return nil, fmt.Errorf("addDays expects a Date struct as first argument")
			}

			daysFloat, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("addDays expects an integer as second argument")
			}

			year, _ := dateStruct.Fields["year"].(int)
			month, _ := dateStruct.Fields["month"].(int)
			day, _ := dateStruct.Fields["day"].(int)

			t := time.Date(year, time.Month(month), day, 0, 0, 0, 0, time.UTC)
			newTime := t.AddDate(0, 0, int(daysFloat))

			newDateStruct := &Struct{
				TypeName: "Date",
				Fields: map[string]interface{}{
					"year":  newTime.Year(),
					"month": int(newTime.Month()),
					"day":   newTime.Day(),
				},
			}

			return newDateStruct, nil
		},
	}

	i.functions["subtractDays"] = &ast.FunctionDeclaration{
		Name: "subtractDays",
		Parameters: []ast.Parameter{
			{Name: "date", Type: "Date"},
			{Name: "days", Type: "int"},
		},
		ReturnType: "Date",
	}

	i.environment["subtractDays"] = &BuiltinFunction{
		Name: "subtractDays",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 2 {
				return nil, fmt.Errorf("subtractDays expects exactly two arguments: a Date and an integer")
			}

			dateStruct, ok := args[0].(*Struct)
			if !ok || dateStruct.TypeName != "Date" {
				return nil, fmt.Errorf("subtractDays expects a Date struct as first argument")
			}

			daysFloat, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("subtractDays expects an integer as second argument")
			}

			year, _ := dateStruct.Fields["year"].(int)
			month, _ := dateStruct.Fields["month"].(int)
			day, _ := dateStruct.Fields["day"].(int)

			t := time.Date(year, time.Month(month), day, 0, 0, 0, 0, time.UTC)
			newTime := t.AddDate(0, 0, -int(daysFloat))

			newDateStruct := &Struct{
				TypeName: "Date",
				Fields: map[string]interface{}{
					"year":  newTime.Year(),
					"month": int(newTime.Month()),
					"day":   newTime.Day(),
				},
			}

			return newDateStruct, nil
		},
	}

	i.functions["today"] = &ast.FunctionDeclaration{
		Name:       "today",
		Parameters: []ast.Parameter{},
		ReturnType: "string",
	}

	i.environment["today"] = &BuiltinFunction{
		Name: "today",
		Fn: func(args []Value) (Value, error) {
			currentTime := time.Now()

			year := currentTime.Year()
			month := int(currentTime.Month())
			day := currentTime.Day()

			monthStr := fmt.Sprintf("%02d", month)
			dayStr := fmt.Sprintf("%02d", day)

			return fmt.Sprintf("%d-%s-%s", year, monthStr, dayStr), nil
		},
	}
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
	if builtinFunc, exists := i.environment[name]; exists {
		if bf, ok := builtinFunc.(*BuiltinFunction); ok {
			return bf.Call(args)
		}
	}

	switch name {
	case "print":
		if len(args) > 0 {
			fmt.Println(args[0])
		}
		return nil, nil

	case "now":
		currentTime := time.Now()
		dateStruct := &Struct{
			TypeName: "Date",
			Fields: map[string]interface{}{
				"year":  currentTime.Year(),
				"month": int(currentTime.Month()),
				"day":   currentTime.Day(),
			},
		}
		return dateStruct, nil

	case "formatDate":
		if len(args) != 1 {
			return nil, fmt.Errorf("formatDate expects exactly one Date argument")
		}

		dateStruct, ok := args[0].(*Struct)
		if !ok || dateStruct.TypeName != "Date" {
			return nil, fmt.Errorf("formatDate expects a Date struct")
		}

		year, _ := dateStruct.Fields["year"].(int)
		month, _ := dateStruct.Fields["month"].(int)
		day, _ := dateStruct.Fields["day"].(int)

		monthStr := fmt.Sprintf("%02d", month)
		dayStr := fmt.Sprintf("%02d", day)

		return fmt.Sprintf("%d-%s-%s", year, monthStr, dayStr), nil

	case "currentYear":
		return float64(time.Now().Year()), nil

	case "currentMonth":
		return float64(int(time.Now().Month())), nil

	case "currentDay":
		return float64(time.Now().Day()), nil

	case "createDate":
		if len(args) != 3 {
			return nil, fmt.Errorf("createDate expects exactly three integer arguments")
		}

		yearFloat, ok := args[0].(float64)
		if !ok {
			return nil, fmt.Errorf("createDate expects year as an integer")
		}

		monthFloat, ok := args[1].(float64)
		if !ok {
			return nil, fmt.Errorf("createDate expects month as an integer")
		}

		dayFloat, ok := args[2].(float64)
		if !ok {
			return nil, fmt.Errorf("createDate expects day as an integer")
		}

		dateStruct := &Struct{
			TypeName: "Date",
			Fields: map[string]interface{}{
				"year":  int(yearFloat),
				"month": int(monthFloat),
				"day":   int(dayFloat),
			},
		}

		return dateStruct, nil

	case "isLeapYear":
		if len(args) != 1 {
			return nil, fmt.Errorf("isLeapYear expects exactly one integer argument")
		}

		yearFloat, ok := args[0].(float64)
		if !ok {
			return nil, fmt.Errorf("isLeapYear expects an integer")
		}

		year := int(yearFloat)

		isLeap := false
		if year%400 == 0 {
			isLeap = true
		} else if year%100 == 0 {
			isLeap = false
		} else if year%4 == 0 {
			isLeap = true
		}

		return isLeap, nil

	case "daysInMonth":
		if len(args) != 2 {
			return nil, fmt.Errorf("daysInMonth expects exactly two integer arguments")
		}

		yearFloat, ok := args[0].(float64)
		if !ok {
			return nil, fmt.Errorf("daysInMonth expects year as an integer")
		}

		monthFloat, ok := args[1].(float64)
		if !ok {
			return nil, fmt.Errorf("daysInMonth expects month as an integer")
		}

		year := int(yearFloat)
		month := int(monthFloat)

		daysInMonth := 31
		if month == 4 || month == 6 || month == 9 || month == 11 {
			daysInMonth = 30
		} else if month == 2 {
			isLeap := false
			if year%400 == 0 {
				isLeap = true
			} else if year%100 == 0 {
				isLeap = false
			} else if year%4 == 0 {
				isLeap = true
			}

			if isLeap {
				daysInMonth = 29
			} else {
				daysInMonth = 28
			}
		}

		return float64(daysInMonth), nil

	case "dayOfWeek":
		if len(args) != 1 {
			return nil, fmt.Errorf("dayOfWeek expects exactly one Date argument")
		}

		dateStruct, ok := args[0].(*Struct)
		if !ok || dateStruct.TypeName != "Date" {
			return nil, fmt.Errorf("dayOfWeek expects a Date struct")
		}

		year, _ := dateStruct.Fields["year"].(int)
		month, _ := dateStruct.Fields["month"].(int)
		day, _ := dateStruct.Fields["day"].(int)

		if month < 3 {
			month += 12
			year--
		}

		k := year % 100
		j := year / 100

		h := (day + ((13 * (month + 1)) / 5) + k + (k / 4) + (j / 4) - (2 * j)) % 7

		if h < 0 {
			h += 7
		}

		return float64(h), nil

	case "addDays":
		if len(args) != 2 {
			return nil, fmt.Errorf("addDays expects exactly two arguments: a Date and an integer")
		}

		dateStruct, ok := args[0].(*Struct)
		if !ok || dateStruct.TypeName != "Date" {
			return nil, fmt.Errorf("addDays expects a Date struct as first argument")
		}

		daysFloat, ok := args[1].(float64)
		if !ok {
			return nil, fmt.Errorf("addDays expects an integer as second argument")
		}

		year, _ := dateStruct.Fields["year"].(int)
		month, _ := dateStruct.Fields["month"].(int)
		day, _ := dateStruct.Fields["day"].(int)

		t := time.Date(year, time.Month(month), day, 0, 0, 0, 0, time.UTC)
		newTime := t.AddDate(0, 0, int(daysFloat))

		newDateStruct := &Struct{
			TypeName: "Date",
			Fields: map[string]interface{}{
				"year":  newTime.Year(),
				"month": int(newTime.Month()),
				"day":   newTime.Day(),
			},
		}

		return newDateStruct, nil

	case "subtractDays":
		if len(args) != 2 {
			return nil, fmt.Errorf("subtractDays expects exactly two arguments: a Date and an integer")
		}

		dateStruct, ok := args[0].(*Struct)
		if !ok || dateStruct.TypeName != "Date" {
			return nil, fmt.Errorf("subtractDays expects a Date struct as first argument")
		}

		daysFloat, ok := args[1].(float64)
		if !ok {
			return nil, fmt.Errorf("subtractDays expects an integer as second argument")
		}

		year, _ := dateStruct.Fields["year"].(int)
		month, _ := dateStruct.Fields["month"].(int)
		day, _ := dateStruct.Fields["day"].(int)

		t := time.Date(year, time.Month(month), day, 0, 0, 0, 0, time.UTC)
		newTime := t.AddDate(0, 0, -int(daysFloat))

		newDateStruct := &Struct{
			TypeName: "Date",
			Fields: map[string]interface{}{
				"year":  newTime.Year(),
				"month": int(newTime.Month()),
				"day":   newTime.Day(),
			},
		}

		return newDateStruct, nil

	case "today":
		currentTime := time.Now()
		year := currentTime.Year()
		month := int(currentTime.Month())
		day := currentTime.Day()

		monthStr := fmt.Sprintf("%02d", month)
		dayStr := fmt.Sprintf("%02d", day)

		return fmt.Sprintf("%d-%s-%s", year, monthStr, dayStr), nil

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

		if obj, ok := object.(map[string]Value); ok {
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

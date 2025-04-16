package interpreter

import (
	"fmt"
	"time"

	"github.com/burnlang/burn/pkg/ast"
)

func (i *Interpreter) registerDateLibrary() {
	i.functions["now"] = &ast.FunctionDeclaration{
		Name:       "now",
		Parameters: []ast.Parameter{},
		ReturnType: "Date",
	}

	i.environment["now"] = &BuiltinFunction{
		Name: "now",
		Fn: func(args []Value) (Value, error) {
			return getNow(), nil
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

func getNow() *Struct {
	currentTime := time.Now()

	dateStruct := &Struct{
		TypeName: "Date",
		Fields: map[string]interface{}{
			"year":  currentTime.Year(),
			"month": int(currentTime.Month()),
			"day":   currentTime.Day(),
		},
	}

	return dateStruct
}
